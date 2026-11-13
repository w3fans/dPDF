mod physical_smoke;
mod search_queue;
mod ui_smoke;
mod viewport_smoke;

use crate::{
    DocumentTab, MarkTile, PageTile, ReviewTile, SearchMarker, SidebarItem, Workspace,
    assembly::{OperationKind, PageOperation},
    document::*,
    pdf::{self, Command, Event, RenderKey},
    platform,
    preferences::{self, Preferences},
};
use slint::{ComponentHandle, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    rc::Rc,
    sync::{atomic::Ordering, mpsc},
    time::{Duration, Instant},
};

#[derive(Default)]
struct RecoveryState {
    observed: u64,
    due: Option<Instant>,
    original: Option<std::sync::Arc<Vec<u8>>>,
    snapshot_pending: bool,
    supersedes: Option<PathBuf>,
    saved_generation: Option<u64>,
}
struct ReviewState {
    model: crate::review::Review,
    baseline: crate::review::Review,
    present: bool,
    dirty: bool,
    generation: u64,
    /// Remark edits (content snapshots); separate from signature/stamp history.
    undo: Vec<crate::review::Review>,
    redo: Vec<crate::review::Review>,
}
const REVIEW_HISTORY: usize = 64;
impl ReviewState {
    fn new(
        model: crate::review::Review,
        baseline: crate::review::Review,
        present: bool,
        dirty: bool,
    ) -> Self {
        Self {
            model,
            baseline,
            present,
            dirty,
            generation: u64::from(dirty),
            undo: vec![],
            redo: vec![],
        }
    }
    fn record(&mut self, before: crate::review::Review) {
        if !before.content_differs(&self.model) {
            return;
        }
        self.undo.push(before);
        if self.undo.len() > REVIEW_HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    fn step(&mut self, undo: bool) -> bool {
        let Some(target) = (if undo {
            self.undo.pop()
        } else {
            self.redo.pop()
        }) else {
            return false;
        };
        let present = self.model.clone();
        if undo {
            self.redo.push(present.clone());
        } else {
            self.undo.push(present.clone());
        }
        self.model = crate::review::Review::restored_from(&target, &present);
        self.dirty = self.model.content_differs(&self.baseline);
        self.generation += 1;
        true
    }
    fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}
struct Tab {
    review: Option<ReviewState>,
    review_requested: bool,
    review_generation: u64,
    review_ready: bool,
    review_error: Option<String>,
    recovery: RecoveryState,
    marks: crate::placement_ui::Drafts,
    info: DocumentInfo,
    raw_sizes: Vec<PageSize>,
    units_ready: bool,
    units_error: Option<String>,
    view: ViewState,
    query: String,
    hits: Vec<SearchHit>,
    search_truncated: bool,
    hit: usize,
}

impl Tab {
    fn unsaved(&self) -> bool {
        !self.marks.items.is_empty() || self.review.as_ref().is_some_and(|r| r.dirty)
    }
    fn draft_generation(&self) -> u64 {
        self.marks
            .generation
            .saturating_add(self.review.as_ref().map_or(0, |r| r.generation))
    }
}

struct TextSelection {
    document: DocumentId,
    page: usize,
    bounds: NormalizedRect,
}

struct PageDrag {
    document: DocumentId,
    page: usize,
}
struct Arrangement {
    document: DocumentId,
    plan: crate::pages::PagePlan,
}

pub struct Application {
    worker: pdf::Worker,
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    opening: HashMap<u64, PathBuf>,
    engine_error: Option<String>,
    generation: u64,
    layout: Option<Layout>,
    viewport: (i32, i32),
    resize_anchor: Option<ViewportAnchor>,
    tab_strip_width: i32,
    reveal_active_tab: bool,
    pending: HashSet<RenderKey>,
    search_queue: search_queue::SearchQueue,
    failed: HashSet<RenderKey>,
    pages: Rc<VecModel<PageTile>>,
    mark_tiles: Rc<VecModel<MarkTile>>,
    sidebar: Rc<VecModel<SidebarItem>>,
    frames: HashMap<usize, RenderKey>,
    thumb_frames: HashSet<usize>,
    marker_offset: f32,
    render_window: Vec<usize>,
    preferences: Preferences,
    preferences_path: Option<PathBuf>,
    preferences_warning: Option<String>,
    preferences_changed: Option<Instant>,
    printer_results: mpsc::Receiver<Result<Vec<String>, String>>,
    printer_sender: mpsc::SyncSender<Result<Vec<String>, String>>,
    printer_names: Vec<String>,
    printer_loading: bool,
    print_pending: bool,
    export_pending: bool,
    selection: Option<TextSelection>,
    selection_token: u64,
    selected_text: String,
    selection_ready: bool,
    copy_when_ready: bool,
    clipboard: Option<arboard::Clipboard>,
    asset_usage: Option<crate::marks::UsageWorker>,
    arrangement: Option<Arrangement>,
    mark_studio: Option<crate::mark_studio::StudioSession>,
    about: Option<crate::about::Session>,
    pending_mark: Option<crate::mark_studio::StudioMark>,
    mark_export: Option<(u64, u64, ViewState, bool)>,
    reopen_view: Option<(PathBuf, ViewState)>,
    recovery: Option<crate::recovery::Worker>,
    /// False for diagnostics that run without the user's recovery store.
    recovery_allowed: bool,
    recovery_candidates: Vec<crate::recovery::Candidate>,
    recovering: Option<(DocumentId, PathBuf, crate::recovery::Restored)>,
    passwords: VecDeque<(DocumentId, PathBuf, bool)>,
    review_auto: bool,
}

impl Application {
    fn review_controls(&self, ui: &Workspace) {
        let review = self.tabs.get(self.active).and_then(|t| t.review.as_ref());
        ui.set_can_undo_remark(review.is_some_and(|r| !r.undo.is_empty()));
        ui.set_can_redo_remark(review.is_some_and(|r| !r.redo.is_empty()));
        ui.set_review_ready(
            review.is_some() && self.tabs.get(self.active).is_some_and(|t| t.review_ready),
        );
        ui.set_review_failed(
            self.tabs
                .get(self.active)
                .is_some_and(|t| t.review_error.is_some()),
        );
        ui.set_review_failure(
            self.tabs
                .get(self.active)
                .and_then(|t| t.review_error.as_deref())
                .unwrap_or("")
                .into(),
        );
        ui.set_review_loaded(review.is_some());
        if let Some(review) = review {
            ui.set_remarks_on(review.model.visible);
            ui.set_revision_labels(ModelRc::new(VecModel::from(
                review
                    .model
                    .revisions
                    .iter()
                    .map(|r| r.label.as_str().into())
                    .collect::<Vec<slint::SharedString>>(),
            )));
            ui.set_revision_index(
                review
                    .model
                    .revisions
                    .iter()
                    .position(|r| r.id == review.model.current)
                    .unwrap_or(0) as i32,
            );
            ui.set_review_filter(match &review.model.filter {
                crate::review::Filter::All => 0,
                crate::review::Filter::Current => 1,
                crate::review::Filter::Selected(_) => 2,
            });
            if !matches!(review.model.filter, crate::review::Filter::Selected(_)) {
                ui.set_revision_filter_ids(
                    (review
                        .model
                        .revisions
                        .iter()
                        .position(|r| r.id == review.model.current)
                        .unwrap_or(0)
                        + 1)
                    .to_string()
                    .into(),
                );
            }
            if let crate::review::Filter::Selected(ids) = &review.model.filter {
                ui.set_revision_filter_ids(
                    ids.iter()
                        .filter_map(|id| {
                            review
                                .model
                                .revisions
                                .iter()
                                .position(|r| r.id == *id)
                                .map(|i| (i + 1).to_string())
                        })
                        .collect::<Vec<_>>()
                        .join(",")
                        .into(),
                );
            }
            ui.set_review_count(format!("{} remarks", review.model.remarks.len()).into());
            let revision = review
                .model
                .revisions
                .iter()
                .find(|r| r.id == review.model.current)
                .map_or("", |r| r.label.as_str());
            ui.set_review_status(
                format!(
                    "Remarks {}{}{}",
                    if review.model.visible { "on" } else { "off" },
                    if revision.is_empty() { "" } else { " · " },
                    revision
                )
                .into(),
            );
        } else {
            ui.set_review_status("".into());
            ui.set_review_count(
                if self.tabs.get(self.active).is_some_and(|t| t.review_ready) {
                    "Review unavailable; qpdf is required"
                } else {
                    "Loading review…"
                }
                .into(),
            );
        }
    }
    fn refresh_reviews(&self, ui: &Workspace) {
        let mut tiles = vec![];
        if let (Some(tab), Some(layout)) = (self.tabs.get(self.active), &self.layout)
            && tab.review_ready
            && let Some(review) = &tab.review
        {
            for remark in review
                .model
                .remarks
                .iter()
                .filter(|r| review.model.is_visible(r))
            {
                if let Some(page) = layout.pages.iter().find(|p| p.page == remark.page)
                    && page.near_viewport(ui.get_scroll_y(), ui.get_viewport_height())
                {
                    let b = remark.bounds.rotated(tab.view.rotation);
                    tiles.push(ReviewTile {
                        px: page.x + b.x * page.width,
                        py: page.y + b.y * page.height,
                        pw: b.width * page.width,
                        ph: b.height * page.height,
                        turn: tab.view.rotation as i32,
                        kind: match remark.kind {
                            crate::review::Kind::Note => 0,
                            crate::review::Kind::Highlight => 1,
                            crate::review::Kind::Underline => 2,
                        },
                    });
                }
            }
        }
        ui.set_review_tiles(ModelRc::new(VecModel::from(tiles)));
    }
    fn review_changed(&mut self, ui: &Workspace, dirty: bool) {
        ui.set_review_selected(-1);
        if dirty
            && let Some(review) = self
                .tabs
                .get_mut(self.active)
                .and_then(|t| t.review.as_mut())
        {
            review.dirty = true;
            review.generation += 1;
        }
        self.review_controls(ui);
        self.refresh_reviews(ui);
        self.tabs_model(ui);
        if ui.get_sidebar_tab() == 2 {
            self.rebuild_sidebar(ui);
        }
    }
    fn step_review(&mut self, ui: &Workspace, undo: bool) {
        let stepped = self
            .tabs
            .get_mut(self.active)
            .and_then(|t| t.review.as_mut())
            .is_some_and(|review| review.step(undo));
        if stepped {
            // step() already maintains dirty/generation against the saved baseline.
            self.review_changed(ui, false);
            ui.set_status(
                if undo {
                    "Undid remark edit"
                } else {
                    "Redid remark edit"
                }
                .into(),
            );
        }
    }
    fn choose_revision(&mut self, ui: &Workspace, index: usize) {
        if let Some(review) = self
            .tabs
            .get_mut(self.active)
            .and_then(|t| t.review.as_mut())
            && let Some(revision) = review.model.revisions.get(index)
        {
            review.model.current = revision.id;
        }
        self.review_changed(ui, false);
    }
    fn new_revision(&mut self, ui: &Workspace) {
        if let Some(review) = self
            .tabs
            .get_mut(self.active)
            .and_then(|t| t.review.as_mut())
        {
            let label = if ui.get_revision_name().trim().is_empty() {
                format!("Revision {}", review.model.revisions.len() + 1)
            } else {
                ui.get_revision_name().to_string()
            };
            let before = review.model.clone();
            match review.model.new_revision(&label) {
                Ok(_) => {
                    review.record(before);
                    ui.set_revision_name("".into());
                }
                Err(e) => {
                    ui.set_status(e.to_string().into());
                    return;
                }
            }
        }
        self.review_changed(ui, true);
    }
    fn retry_review(&mut self, ui: &Workspace) {
        let Some(id) = self.tabs.get(self.active).map(|t| t.info.id) else {
            return;
        };
        self.request_review(ui, id);
    }
    fn request_review(&mut self, ui: &Workspace, id: DocumentId) {
        let Some(index) = self.tabs.iter().position(|tab| tab.info.id == id) else {
            return;
        };
        let Some(generation) = self.tabs[index].review_generation.checked_add(1) else {
            ui.set_status("Review request identity exhausted".into());
            return;
        };
        if self.command(ui, Command::LoadReview(id, generation)) {
            let tab = &mut self.tabs[index];
            tab.review_generation = generation;
            tab.review_requested = true;
            tab.review_ready = false;
            tab.review_error = None;
            self.review_controls(ui);
        }
    }
    fn filter_reviews(&mut self, ui: &Workspace, index: i32) {
        let result = (|| -> anyhow::Result<()> {
            let Some(review) = self
                .tabs
                .get_mut(self.active)
                .and_then(|t| t.review.as_mut())
            else {
                return Ok(());
            };
            let filter = match index {
                0 => crate::review::Filter::All,
                1 => crate::review::Filter::Current,
                _ => review
                    .model
                    .filter_for_positions(ui.get_revision_filter_ids().as_str())?,
            };
            if matches!(filter, crate::review::Filter::Current) {
                review.model.schema_version = 2;
            }
            review.model.filter = filter;
            review.model.visible = ui.get_remarks_on();
            Ok(())
        })();
        if let Err(error) = result {
            self.review_controls(ui);
            ui.set_status(error.to_string().into());
            return;
        }
        self.review_changed(ui, false);
    }
    fn add_remark(&mut self, ui: &Workspace, page: usize, start: (f32, f32), end: (f32, f32)) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        let Some(review) = &mut tab.review else {
            ui.set_status("Wait for review loading".into());
            return;
        };
        let kind = match ui.get_review_tool() {
            1 => crate::review::Kind::Note,
            2 => crate::review::Kind::Highlight,
            _ => crate::review::Kind::Underline,
        };
        let b = if kind == crate::review::Kind::Note {
            NormalizedRect {
                x: end.0.clamp(0.0, 0.96),
                y: end.1.clamp(0.0, 0.97),
                width: 0.04,
                height: 0.03,
            }
        } else {
            NormalizedRect {
                x: start.0.min(end.0),
                y: start.1.min(end.1),
                width: (end.0 - start.0).abs(),
                height: (end.1 - start.1).abs(),
            }
        };
        let b = b.rotated((4 - tab.view.rotation % 4) % 4);
        let before = review.model.clone();
        if let Err(error) = review.model.add(
            page,
            b,
            kind,
            ui.get_review_text().as_str(),
            tab.info.pages.len(),
        ) {
            ui.set_status(error.to_string().into());
            return;
        }
        review.record(before);
        review.model.visible = true;
        ui.set_review_tool(0);
        ui.set_review_text("".into());
        self.review_changed(ui, true);
        ui.set_sidebar_tab(2);
        self.rebuild_sidebar(ui);
        ui.set_status("Remark added to the active revision".into());
    }
    fn select_remark(&mut self, ui: &Workspace, index: usize, remove: bool) {
        let Some(review) = self
            .tabs
            .get_mut(self.active)
            .and_then(|t| t.review.as_mut())
        else {
            return;
        };
        let ids = review
            .model
            .remarks
            .iter()
            .filter(|r| review.model.is_visible(r))
            .map(|r| r.id)
            .collect::<Vec<_>>();
        let Some(id) = ids.get(index) else {
            return;
        };
        let Some(position) = review.model.remarks.iter().position(|r| r.id == *id) else {
            return;
        };
        if remove {
            let before = review.model.clone();
            review.model.remarks.remove(position);
            review.record(before);
            self.review_changed(ui, true);
        } else {
            let page = review.model.remarks[position].page;
            ui.set_review_selected(index as i32);
            ui.set_review_text(review.model.remarks[position].text.as_str().into());
            self.go_to(ui, page);
        }
    }
    fn update_remark(&mut self, ui: &Workspace) {
        let Some(review) = self
            .tabs
            .get_mut(self.active)
            .and_then(|t| t.review.as_mut())
        else {
            return;
        };
        let index = ui.get_review_selected();
        if index < 0 {
            return;
        }
        let Some(id) = review
            .model
            .remarks
            .iter()
            .filter(|r| review.model.is_visible(r))
            .nth(index as usize)
            .map(|r| r.id)
        else {
            return;
        };
        if ui.get_review_text().len() > 4096 {
            ui.set_status("Remark text is limited to 4096 bytes".into());
            return;
        }
        let before = review.model.clone();
        if let Some(remark) = review.model.remarks.iter_mut().find(|r| r.id == id) {
            remark.text = ui.get_review_text().to_string();
        }
        review.record(before);
        self.review_changed(ui, true);
    }
    fn export_review(&mut self, ui: &Workspace) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        if tab.info.password_protected && !confirm_unencrypted_copy() {
            return;
        }
        if tab.info.signatures>0 && !rfd::MessageDialog::new().set_title("Existing digital signatures").set_description("Saving review changes invalidates cryptographic signatures in this copy. The source is preserved.").set_buttons(rfd::MessageButtons::OkCancel).show().eq(&rfd::MessageDialogResult::Ok){return;}
        let name = format!(
            "{}-review.pdf",
            tab.info
                .path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
        );
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF", &["pdf"])
            .set_file_name(name)
            .save_file()
        {
            let mode = match ui.get_review_export() {
                0 => crate::review::ExportMode::Visible,
                1 => crate::review::ExportMode::Current,
                _ => crate::review::ExportMode::Clean,
            };
            self.save_workspace_to(ui, path, mode);
        }
    }
    fn password_controls(&self, ui: &Workspace) {
        ui.set_password_text("".into());
        if let Some((_, path, wrong)) = self.passwords.front() {
            ui.set_password_file(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .as_ref()
                    .into(),
            );
            ui.set_password_error(
                if *wrong {
                    "Incorrect password. Try again."
                } else {
                    "Enter this PDF's password."
                }
                .into(),
            );
            ui.set_password_required(true);
        } else {
            ui.set_password_required(false);
        }
    }
    fn password_action(&mut self, ui: &Workspace, cancel: bool) {
        let Some((id, _, _)) = self.passwords.front() else {
            return;
        };
        let id = *id;
        if cancel {
            if !self.command(ui, Command::Close(id)) {
                return;
            }
            self.opening.remove(&id);
            if self
                .recovering
                .as_ref()
                .is_some_and(|(pending, _, _)| *pending == id)
            {
                self.recovering = None;
                ui.set_recovery_busy(false);
            }
        } else {
            let password = ui.get_password_text().to_string();
            if let Err(error) = crate::encrypted::validate_password(&password) {
                ui.set_password_error(error.to_string().into());
                return;
            }
            if !self.command(ui, Command::Unlock(id, password)) {
                return;
            }
        }
        self.passwords.pop_front();
        self.password_controls(ui);
        ui.set_busy(!self.opening.is_empty());
        if cancel {
            ui.set_status("Password-protected PDF opening cancelled".into());
        }
    }
    fn recovery_controls(&self, ui: &Workspace) {
        ui.set_has_recovery(!self.recovery_candidates.is_empty());
        if let Some(candidate) = self.recovery_candidates.first() {
            ui.set_recovery_label(
                format!(
                    "Unsaved draft: {} ({} available)",
                    candidate
                        .source
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy(),
                    self.recovery_candidates.len()
                )
                .into(),
            );
        }
    }
    fn recover_draft(&mut self, ui: &Workspace, discard: bool) {
        if ui.get_recovery_busy() {
            return;
        }
        if let (Some(worker), Some(candidate)) = (&self.recovery, self.recovery_candidates.first())
        {
            let request = if discard {
                crate::recovery::Request::Discard(candidate.path.clone())
            } else {
                crate::recovery::Request::Load(candidate.path.clone())
            };
            if worker.requests.try_send(request).is_ok() {
                ui.set_recovery_busy(true);
            }
        }
    }
    /// Tabs keep their unsaved drafts; only their on-disk checkpoint state is reset.
    fn forget_recovery_checkpoints(&mut self, ui: &Workspace) {
        self.recovery_candidates.clear();
        for tab in &mut self.tabs {
            tab.recovery.saved_generation = None;
            tab.recovery.supersedes = None;
            tab.recovery.due = None;
            tab.recovery.observed = tab.draft_generation();
        }
        self.recovery_controls(ui);
        self.privacy_controls(ui);
    }
    fn privacy_controls(&self, ui: &Workspace) {
        ui.set_recovery_enabled(self.recovery.is_some());
        ui.set_recovery_allowed(self.recovery_allowed && crate::recovery::default_root().is_some());
        let location = crate::recovery::default_root()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "unavailable".into());
        ui.set_recovery_summary(
            if !self.recovery_allowed {
                "Recovery is disabled for this diagnostic session.".to_string()
            } else if self.recovery.is_some() {
                format!(
                    "{} saved draft(s) · unsaved marks and remarks are checkpointed unencrypted in {location}",
                    self.recovery_candidates.len()
                )
            } else {
                format!("Off · no new checkpoints; earlier drafts stay in {location} until deleted")
            }
            .into(),
        );
    }
    fn set_recovery_enabled(&mut self, ui: &Workspace, enabled: bool) {
        if enabled == self.recovery.is_some() || (enabled && !self.recovery_allowed) {
            self.privacy_controls(ui);
            return;
        }
        if enabled {
            self.recovery = crate::recovery::default_root().map(crate::recovery::Worker::start);
            for tab in &mut self.tabs {
                // Checkpoint unsaved work that was edited while recovery was off.
                if tab.unsaved() {
                    tab.recovery.observed = tab.draft_generation().wrapping_add(1);
                }
            }
        } else {
            // Dropping the worker finishes queued writes and releases the store lock.
            self.recovery = None;
            self.forget_recovery_checkpoints(ui);
            ui.set_recovery_busy(false);
        }
        self.preferences.recovery_enabled = self.recovery.is_some();
        self.preferences_changed = Some(Instant::now());
        self.remember_preferences(ui, true);
        ui.set_status(
            if self.recovery.is_some() {
                "Local draft recovery is on"
            } else {
                "Local draft recovery is off; existing drafts were kept"
            }
            .into(),
        );
        self.privacy_controls(ui);
    }
    fn clear_recovery(&mut self, ui: &Workspace) {
        if ui.get_recovery_busy() {
            return;
        }
        if let Some(worker) = &self.recovery {
            if worker
                .requests
                .try_send(crate::recovery::Request::ClearAll)
                .is_ok()
            {
                ui.set_recovery_busy(true);
            }
            return;
        }
        let Some(root) = crate::recovery::default_root() else {
            ui.set_status("Local recovery storage is unavailable".into());
            return;
        };
        match crate::recovery::clear_store(&root) {
            Ok(count) => {
                self.forget_recovery_checkpoints(ui);
                ui.set_status(cleared_message(count).into());
            }
            Err(error) => {
                ui.set_status(format!("Recovery drafts were not deleted: {error:#}").into())
            }
        }
    }
    fn remove_recovery(&mut self, index: usize) {
        if let (Some(worker), Some(tab)) = (&self.recovery, self.tabs.get_mut(index)) {
            let _ = worker.requests.send(crate::recovery::Request::Remove {
                id: tab.info.id,
                supersedes: tab.recovery.supersedes.take(),
            });
            tab.recovery.due = None;
            tab.recovery.observed = tab.draft_generation();
        }
    }
    fn recovery_tick(&mut self, ui: &Workspace) {
        use crate::recovery::{Reply, Request};
        while let Some(reply) = self
            .recovery
            .as_ref()
            .and_then(|w| w.replies.try_recv().ok())
        {
            match reply {
                Reply::Available(candidates) => {
                    self.recovery_candidates = candidates;
                    self.recovery_controls(ui);
                }
                Reply::Saved(id, generation, old) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.info.id == id) {
                        tab.recovery.saved_generation = Some(generation);
                        if tab.recovery.supersedes == old {
                            tab.recovery.supersedes = None;
                        }
                    }
                    if let Some(old) = old {
                        self.recovery_candidates.retain(|c| c.path != old);
                        self.recovery_controls(ui);
                    }
                }
                Reply::Removed(id, old) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.info.id == id) {
                        tab.recovery.saved_generation = None;
                    }
                    if let Some(old) = old {
                        self.recovery_candidates.retain(|c| c.path != old);
                        self.recovery_controls(ui);
                    }
                }
                Reply::Discarded(path) => {
                    self.recovery_candidates.retain(|c| c.path != path);
                    ui.set_recovery_busy(false);
                    self.recovery_controls(ui);
                }
                Reply::Cleared(count) => {
                    self.forget_recovery_checkpoints(ui);
                    ui.set_recovery_busy(false);
                    ui.set_status(cleared_message(count).into());
                }
                Reply::Loaded(path, draft) => {
                    let id = self.next_id;
                    self.next_id += 1;
                    if self.command(
                        ui,
                        Command::OpenSnapshot(id, draft.source.clone(), draft.original.clone()),
                    ) {
                        self.opening.insert(id, draft.source.clone());
                        ui.set_busy(true);
                        self.recovering = Some((id, path, draft));
                    } else {
                        ui.set_recovery_busy(false);
                    }
                }
                Reply::Unavailable(message) => {
                    self.recovery = None;
                    ui.set_recovery_busy(false);
                    ui.set_status(message.into());
                }
                Reply::Failed(message) => {
                    ui.set_recovery_busy(false);
                    ui.set_status(message.into());
                }
            }
        }
        ui.set_recovery_note(
            if self.recovery.is_none() {
                "Recovery off"
            } else if self
                .tabs
                .get(self.active)
                .is_some_and(|t| t.recovery.saved_generation == Some(t.draft_generation()))
            {
                "Recovery saved"
            } else {
                "Recovery pending"
            }
            .into(),
        );
        let Some(worker) = &self.recovery else {
            return;
        };
        for tab in &mut self.tabs {
            if tab.recovery.observed != tab.draft_generation() {
                tab.recovery.observed = tab.draft_generation();
                tab.recovery.due = Some(Instant::now());
            }
            let Some(since) = tab.recovery.due else {
                continue;
            };
            if !tab.unsaved() {
                if worker
                    .requests
                    .try_send(Request::Remove {
                        id: tab.info.id,
                        supersedes: tab.recovery.supersedes.clone(),
                    })
                    .is_ok()
                {
                    tab.recovery.due = None;
                    tab.recovery.original = None;
                }
                continue;
            }
            if tab.marks.is_dragging() || since.elapsed() < Duration::from_secs(1) {
                continue;
            }
            let Some(original) = &tab.recovery.original else {
                if !tab.recovery.snapshot_pending
                    && self
                        .worker
                        .commands
                        .try_send(Command::Snapshot(tab.info.id))
                        .is_ok()
                {
                    tab.recovery.snapshot_pending = true;
                }
                continue;
            };
            let draft = crate::recovery::Draft {
                review: tab
                    .review
                    .as_ref()
                    .filter(|r| r.present || r.dirty || !r.model.remarks.is_empty())
                    .map(|r| r.model.clone()),
                source: tab.info.path.clone(),
                original: original.clone(),
                selected: tab.marks.selected,
                marks: tab
                    .marks
                    .items
                    .iter()
                    .map(|m| crate::recovery::DraftMark {
                        placement: m.placement.clone(),
                        default_size: m.default_size,
                        aspect_locked: m.aspect_locked,
                    })
                    .collect(),
            };
            if worker
                .requests
                .try_send(Request::Save {
                    id: tab.info.id,
                    generation: tab.draft_generation(),
                    draft,
                    supersedes: tab.recovery.supersedes.clone(),
                })
                .is_ok()
            {
                tab.recovery.due = None;
            }
        }
    }
    fn request_native_print(&mut self, ui: &Workspace, use_range: bool) {
        if !ui.get_native_printing() {
            ui.set_print_visible(!self.tabs.is_empty());
            return;
        }
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            if self.guard_marks(ui) {
                return;
            }
            if self.print_pending {
                ui.set_status("A print job is already open or being submitted".into());
                return;
            }
            let Some(tab) = self.tabs.get(self.active) else {
                return;
            };
            let range = if use_range {
                match platform::validate_range(&ui.get_print_range(), tab.info.pages.len()) {
                    Ok(range) => range,
                    Err(e) => {
                        ui.set_status(e.to_string().into());
                        return;
                    }
                }
            } else {
                String::new()
            };
            let command = if let Some(review) = &tab.review
                && !review.model.remarks.is_empty()
            {
                Command::PrintReviewed {
                    id: tab.info.id,
                    review: review.model.clone(),
                    current: tab.view.page,
                    range,
                    printer: ui.get_printer_name().to_string(),
                    fit: ui.get_print_scaling() == 0,
                    native: true,
                }
            } else {
                Command::PrintNative {
                    id: tab.info.id,
                    current: tab.view.page,
                    range,
                    printer: ui.get_printer_name().to_string(),
                    fit: ui.get_print_scaling() == 0,
                }
            };
            if self.command(ui, command) {
                self.print_pending = true;
                ui.set_print_busy(true);
                ui.set_print_visible(false);
                ui.set_status("Native print dialog open — the reader remains available".into());
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        let _ = use_range;
    }
    fn new(smoke: bool, enable_recovery: bool, review_auto: bool) -> Self {
        let mut preferences_path = if smoke {
            None
        } else {
            preferences::preferences_path()
        };
        let (preferences, preferences_warning) = match preferences_path
            .as_deref()
            .map(preferences::load)
            .transpose()
        {
            Ok(preferences) => (preferences.unwrap_or_default(), None),
            Err(error) => {
                preferences_path = None;
                (
                    Preferences::default(),
                    Some(format!(
                        "Preferences could not be loaded; using defaults: {error:#}"
                    )),
                )
            }
        };
        let (printer_sender, printer_results) = mpsc::sync_channel(1);
        let recovery_on = enable_recovery && preferences.recovery_enabled;
        Self {
            worker: pdf::start(),
            tabs: vec![],
            active: 0,
            next_id: 1,
            opening: HashMap::new(),
            engine_error: None,
            generation: 0,
            layout: None,
            viewport: (0, 0),
            resize_anchor: None,
            tab_strip_width: 0,
            reveal_active_tab: true,
            pending: HashSet::new(),
            search_queue: search_queue::SearchQueue::default(),
            failed: HashSet::new(),
            pages: Rc::new(VecModel::default()),
            mark_tiles: Rc::new(VecModel::default()),
            sidebar: Rc::new(VecModel::default()),
            frames: HashMap::new(),
            thumb_frames: HashSet::new(),
            marker_offset: f32::NAN,
            render_window: vec![],
            preferences,
            preferences_path,
            preferences_warning,
            preferences_changed: None,
            printer_results,
            printer_sender,
            printer_names: vec![],
            printer_loading: false,
            print_pending: false,
            export_pending: false,
            selection: None,
            selection_token: 0,
            selected_text: String::new(),
            selection_ready: false,
            copy_when_ready: false,
            clipboard: None,
            asset_usage: crate::marks::UsageWorker::start(),
            arrangement: None,
            mark_studio: None,
            about: None,
            pending_mark: None,
            mark_export: None,
            reopen_view: None,
            recovery: if recovery_on {
                crate::recovery::default_root().map(crate::recovery::Worker::start)
            } else {
                None
            },
            recovery_allowed: enable_recovery,
            recovery_candidates: vec![],
            recovering: None,
            passwords: VecDeque::new(),
            review_auto,
        }
    }

    fn remember_preferences(&mut self, ui: &Workspace, force: bool) {
        let current = Preferences {
            dark_theme: ui.get_dark(),
            sidebar_visible: ui.get_sidebar_visible(),
            view_mode: ui.get_view_mode().clamp(0, 2) as u8,
            printer_name: ui.get_printer_name().to_string(),
            print_fit: ui.get_print_scaling() == 0,
            recovery_enabled: self.preferences.recovery_enabled,
            ..Default::default()
        };
        if self.preferences != current {
            self.preferences = current;
            self.preferences_changed = Some(Instant::now());
        }
        if self
            .preferences_changed
            .is_some_and(|when| force || when.elapsed() >= Duration::from_millis(500))
        {
            self.preferences_changed = None;
            if let Some(path) = &self.preferences_path
                && let Err(error) = preferences::save(path, &self.preferences)
            {
                ui.set_status(format!("Preferences could not be saved: {error:#}").into());
            }
        }
    }

    fn discover_printers(&mut self, ui: &Workspace) {
        if self.printer_loading {
            return;
        }
        self.printer_loading = true;
        ui.set_printer_loading(true);
        let sender = self.printer_sender.clone();
        std::thread::spawn(move || {
            let _ =
                sender.send(platform::discover_printers().map_err(|error| format!("{error:#}")));
        });
    }

    fn printer_choices(&self, ui: &Workspace) {
        let mut choices: Vec<slint::SharedString> = vec!["System default".into()];
        choices.extend(self.printer_names.iter().map(|name| name.as_str().into()));
        choices.push("Other printer…".into());
        let index = if ui.get_printer_name().is_empty() {
            0
        } else {
            self.printer_names
                .iter()
                .position(|name| name == ui.get_printer_name().as_str())
                .map_or(choices.len() - 1, |index| index + 1)
        };
        ui.set_printer_choices(ModelRc::new(VecModel::from(choices)));
        ui.set_printer_choice(index as i32);
    }

    fn choose_printer(&self, ui: &Workspace, index: i32) {
        if index == 0 {
            ui.set_printer_name("".into());
        } else if let Some(name) = self.printer_names.get(index as usize - 1) {
            ui.set_printer_name(name.as_str().into());
        }
    }

    fn command(&self, ui: &Workspace, command: Command) -> bool {
        match self.worker.commands.try_send(command) {
            Ok(()) => true,
            Err(error) => {
                ui.set_status(format!("PDF worker busy or unavailable: {error}").into());
                false
            }
        }
    }

    fn open(&mut self, ui: &Workspace, path: PathBuf) {
        if let Some(error) = &self.engine_error {
            ui.set_status(error.into());
            return;
        }
        let path = path.canonicalize().unwrap_or(path);
        if let Some(index) = self.tabs.iter().position(|tab| tab.info.path == path) {
            self.activate(ui, index);
            return;
        }
        if self.opening.values().any(|pending| pending == &path) {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        if self.command(ui, Command::Open(id, path.clone())) {
            self.opening.insert(id, path.clone());
            ui.set_busy(true);
            ui.set_status(format!("Opening {}", path.display()).into());
        }
    }

    fn tabs_model(&self, ui: &Workspace) {
        self.share_controls(ui);
        ui.set_tabs(ModelRc::new(VecModel::from(
            self.tabs
                .iter()
                .enumerate()
                .map(|(i, tab)| DocumentTab {
                    name: format!(
                        "{}{}",
                        tab.info
                            .path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .as_ref(),
                        if tab.unsaved() { " *" } else { "" }
                    )
                    .into(),
                    selected: self.active == i,
                })
                .collect::<Vec<_>>(),
        )));
        ui.set_has_document(!self.tabs.is_empty());
    }

    fn activate(&mut self, ui: &Workspace, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        if let Some(old) = self.tabs.get_mut(self.active) {
            old.marks.finish_drag();
            old.view.scroll_y = ui.get_scroll_y();
            old.view.scroll_x = ui.get_scroll_x();
        }
        self.show_tab(ui, index);
    }

    fn show_tab(&mut self, ui: &Workspace, index: usize) {
        ui.invoke_focus_reader();
        self.reveal_active_tab = true;
        ui.set_review_selected(-1);
        ui.set_review_tool(0);
        ui.set_review_text("".into());
        ui.set_revision_name("".into());
        self.pending_mark = None;
        ui.set_mark_pending(false);
        self.clear_selection(ui);
        self.active = index;
        let tab = &self.tabs[index];
        ui.set_document_title(tab.info.path.to_string_lossy().as_ref().into());
        ui.set_query(tab.query.as_str().into());
        ui.set_search_label(
            if tab.hits.is_empty() {
                String::new()
            } else {
                format!(
                    "{} / {} matches{}",
                    tab.hit + 1,
                    tab.hits.len(),
                    if tab.search_truncated { "+" } else { "" }
                )
            }
            .into(),
        );
        ui.set_view_mode(match tab.view.mode {
            ViewMode::Single => 0,
            ViewMode::Continuous => 1,
            ViewMode::Facing => 2,
        });
        let scroll = tab.view.scroll_y;
        let scroll_x = tab.view.scroll_x;
        self.worker.search_token.fetch_add(1, Ordering::Relaxed);
        self.tabs_model(ui);
        self.rebuild_sidebar(ui);
        self.relayout(ui, false);
        ui.set_scroll_y(scroll);
        ui.set_scroll_x(scroll_x);
        self.mark_controls(ui);
        self.review_controls(ui);
        self.refresh_reviews(ui);
    }

    fn close(&mut self, ui: &Workspace, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        if self.tabs[index].unsaved() && !confirm_discard() {
            return;
        }
        if !self.command(ui, Command::Close(self.tabs[index].info.id)) {
            return;
        }
        if let Some(current) = self.tabs.get_mut(self.active) {
            current.view.scroll_y = ui.get_scroll_y();
            current.view.scroll_x = ui.get_scroll_x();
        }
        self.remove_recovery(index);
        self.tabs.remove(index);
        self.active = self
            .active
            .saturating_sub(usize::from(index < self.active))
            .min(self.tabs.len().saturating_sub(1));
        if self.tabs.is_empty() {
            ui.set_tab_strip_x(0.0);
            self.clear_selection(ui);
            self.pending_mark = None;
            ui.set_mark_pending(false);
            ui.set_has_marks(false);
            ui.set_can_undo_mark(false);
            ui.set_can_redo_mark(false);
            ui.set_mark_selected(false);
            self.mark_tiles.set_vec(vec![]);
            ui.set_review_ready(false);
            ui.set_review_failed(false);
            ui.set_review_failure("".into());
            ui.set_review_selected(-1);
            ui.set_review_tool(0);
            ui.set_review_text("".into());
            ui.set_review_tiles(ModelRc::default());
            self.generation += 1;
            self.worker
                .generation
                .store(self.generation, Ordering::Relaxed);
            self.worker.search_token.fetch_add(1, Ordering::Relaxed);
            self.pages.set_vec(vec![]);
            ui.set_search_markers(ModelRc::default());
            self.sidebar.set_vec(vec![]);
            self.layout = None;
            self.render_window.clear();
            self.pending.clear();
            self.frames.clear();
            self.thumb_frames.clear();
            ui.set_search_visible(false);
            ui.set_print_visible(false);
            ui.set_page_ops_visible(false);
            ui.set_has_document(false);
            ui.set_page_label("0 / 0".into());
            ui.set_jump_text("".into());
            ui.set_status("Open a PDF to begin".into());
            self.tabs_model(ui);
        } else {
            self.show_tab(ui, self.active);
        }
    }

    fn relayout(&mut self, ui: &Workspace, jump: bool) {
        self.resize_anchor = None;
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        tab.marks.finish_drag();
        let width = ui.get_viewport_width().max(100.0);
        let height = ui.get_viewport_height().max(100.0);
        let layout = page_layout(&tab.info.pages, &tab.view, width, height);
        self.viewport = (width as i32, height as i32);
        self.generation += 1;
        self.worker
            .generation
            .store(self.generation, Ordering::Relaxed);
        self.pending.clear();
        self.failed.clear();
        self.frames.clear();
        self.pages.set_vec(vec![]);
        self.render_window.clear();
        ui.set_canvas_width(layout.width);
        ui.set_canvas_height(layout.height);
        ui.set_zoom_label(format!("{:.0}%", layout.scale * 100.0).into());
        if !ui.get_zoom_editing() {
            ui.set_zoom_entry(ui.get_zoom_label());
        }
        ui.set_view_label(
            match tab.view.mode {
                ViewMode::Single => "Single page",
                ViewMode::Continuous => "Continuous",
                ViewMode::Facing => "Two pages",
            }
            .into(),
        );
        if jump {
            let y = if tab.view.mode == ViewMode::Continuous {
                layout
                    .pages
                    .iter()
                    .find(|p| p.page == tab.view.page)
                    .map_or(0.0, |p| -(p.y - 16.0))
            } else {
                0.0
            };
            ui.set_scroll_y(y);
            ui.set_scroll_x(0.0);
        }
        self.layout = Some(layout);
        self.update_page_label(ui);
        self.refresh_markers(ui);
    }

    fn update_page_label(&self, ui: &Workspace) {
        if let Some(tab) = self.tabs.get(self.active) {
            ui.set_current_page(tab.view.page as i32);
            ui.set_page_count(tab.info.pages.len() as i32);
            if !ui.get_jump_editing() {
                ui.set_jump_text((tab.view.page + 1).to_string().into());
            }
            ui.set_page_label(format!("/ {}", tab.info.pages.len()).into());
            if ui.get_sidebar_visible()
                && ui.get_sidebar_tab() == 0
                && ui.get_sidebar_height() > 1.0
            {
                let row = self
                    .sidebar
                    .iter()
                    .position(|item| item.page as usize == tab.view.page)
                    .unwrap_or(tab.view.page);
                let top = row as f32 * 174.0;
                let bottom = top + 168.0;
                let offset = ui.get_sidebar_offset();
                if top < offset {
                    ui.set_sidebar_y(-top);
                } else if bottom > offset + ui.get_sidebar_height() {
                    ui.set_sidebar_y(-(bottom - ui.get_sidebar_height()));
                }
            }
        }
    }

    fn rebuild_sidebar(&mut self, ui: &Workspace) {
        self.thumb_frames.clear();
        ui.set_sidebar_y(0.0);
        let Some(tab) = self.tabs.get(self.active) else {
            self.sidebar.set_vec(vec![]);
            return;
        };
        self.sidebar.set_vec(if ui.get_sidebar_tab() == 0 {
            self.arrangement
                .as_ref()
                .filter(|a| a.document == tab.info.id)
                .map(|a| a.plan.pages().iter().map(|p| p.page).collect::<Vec<_>>())
                .unwrap_or_else(|| (0..tab.info.pages.len()).collect())
                .into_iter()
                .enumerate()
                .map(|(position, page)| {
                    let mut drag = slint::DataTransfer::default();
                    drag.set_user_data(Rc::new(PageDrag {
                        document: tab.info.id,
                        page,
                    }));
                    SidebarItem {
                        title: if self.arrangement.is_some() {
                            format!("{} · Original page {}", position + 1, page + 1)
                        } else {
                            format!("Page {}", page + 1)
                        }
                        .into(),
                        page: page as i32,
                        bitmap: Default::default(),
                        drag,
                    }
                })
                .collect::<Vec<_>>()
        } else if ui.get_sidebar_tab() == 2 {
            tab.review
                .as_ref()
                .map(|review| {
                    review
                        .model
                        .remarks
                        .iter()
                        .filter(|r| review.model.is_visible(r))
                        .map(|r| SidebarItem {
                            title: format!(
                                "p{} · {} · {}",
                                r.page + 1,
                                review
                                    .model
                                    .revisions
                                    .iter()
                                    .find(|rev| rev.id == r.revision)
                                    .map_or("", |r| r.label.as_str()),
                                r.text
                            )
                            .into(),
                            page: r.page as i32,
                            bitmap: Default::default(),
                            drag: Default::default(),
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            tab.info
                .bookmarks
                .iter()
                .map(|b| SidebarItem {
                    title: b.title.as_str().into(),
                    page: b.page as i32,
                    bitmap: Default::default(),
                    drag: Default::default(),
                })
                .collect::<Vec<_>>()
        });
    }

    fn apply_page_order(&mut self, ui: &Workspace) -> bool {
        let Some(tab) = self.tabs.get(self.active) else {
            return false;
        };
        let operation = PageOperation {
            kind: OperationKind::Reorder,
            pages: ui.get_operation_pages().to_string(),
            source_pages: String::new(),
        };
        match crate::assembly::build_plan(vec![tab.info.pages.len()], &operation) {
            Ok(plan) => {
                self.arrangement = Some(Arrangement {
                    document: tab.info.id,
                    plan,
                });
                let scroll = ui.get_sidebar_y();
                ui.set_sidebar_visible(true);
                ui.set_sidebar_tab(0);
                self.rebuild_sidebar(ui);
                ui.set_sidebar_y(scroll);
                true
            }
            Err(error) => {
                ui.set_status(error.to_string().into());
                false
            }
        }
    }

    fn drop_page_at(&mut self, ui: &Workspace, data: slint::DataTransfer, y: f32) {
        let height = ui.get_thumbnail_height();
        let stride = height + ui.get_thumbnail_spacing();
        let position = ui.get_sidebar_offset() + y;
        if !position.is_finite() || position < 0.0 || stride <= 0.0 || self.sidebar.row_count() == 0
        {
            return;
        }
        let row = ((position / stride) as usize).min(self.sidebar.row_count() - 1);
        self.drop_page(ui, data, row, position - row as f32 * stride < height / 2.0);
    }

    fn scroll_page_drag(&self, ui: &Workspace) {
        let direction = ui.get_page_drag_scroll();
        if direction == 0 || self.arrangement.is_none() {
            return;
        }
        let spacing = ui.get_thumbnail_spacing();
        let content = (self.sidebar.row_count() as f32 * (ui.get_thumbnail_height() + spacing)
            - spacing)
            .max(0.0);
        let maximum = (content - ui.get_sidebar_height()).max(0.0);
        let offset = (ui.get_sidebar_offset() + direction as f32 * 14.0).clamp(0.0, maximum);
        ui.set_sidebar_y(-offset);
    }

    fn drop_page(&mut self, ui: &Workspace, data: slint::DataTransfer, row: usize, before: bool) {
        if !ui.get_arranging() {
            return;
        }
        let Some(payload) = data.user_data() else {
            return;
        };
        let Some(payload) = payload.downcast_ref::<PageDrag>() else {
            return;
        };
        if !self
            .tabs
            .get(self.active)
            .is_some_and(|t| t.info.id == payload.document)
            || !self.apply_page_order(ui)
        {
            return;
        }
        let arrangement = self.arrangement.as_mut().expect("valid page order");
        let Some(from) = arrangement
            .plan
            .pages()
            .iter()
            .position(|p| p.page == payload.page)
        else {
            return;
        };
        if let Err(error) = arrangement.plan.move_page(from, row + usize::from(!before)) {
            ui.set_status(error.to_string().into());
            return;
        }
        let order: Vec<_> = arrangement.plan.pages().iter().map(|p| p.page).collect();
        ui.set_operation_pages(crate::pages::ranges(&order).into());
        let scroll = ui.get_sidebar_y();
        self.rebuild_sidebar(ui);
        ui.set_sidebar_y(scroll);
        ui.set_status("Output order updated — Save edited copy to apply".into());
    }

    fn go_to(&mut self, ui: &Workspace, page: usize) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        tab.view.page = page.min(tab.info.pages.len().saturating_sub(1));
        self.relayout(ui, true);
    }

    fn jump(&mut self, ui: &Workspace, value: &str) {
        match value.trim().parse::<usize>() {
            Ok(page) if page > 0 => self.go_to(ui, page - 1),
            _ => ui.set_status("Enter a page number starting at 1".into()),
        }
    }

    fn navigate(&mut self, ui: &Workspace, delta: i32) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let step = if tab.view.mode == ViewMode::Facing {
            2
        } else {
            1
        };
        let page = (tab.view.page as i32 + delta * step).max(0) as usize;
        self.go_to(ui, page);
    }

    fn search(&mut self, ui: &Workspace, query: String) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        tab.query = query.clone();
        tab.hits.clear();
        tab.hit = 0;
        tab.search_truncated = false;
        ui.set_search_markers(ModelRc::default());
        let id = tab.info.id;
        let token = self.worker.search_token.fetch_add(1, Ordering::Relaxed) + 1;
        ui.set_search_label("Searching…".into());
        self.search_queue.replace(id, token, query);
        self.flush_search(ui);
    }

    fn flush_search(&mut self, ui: &Workspace) {
        if self
            .search_queue
            .flush(
                &self.worker.commands,
                self.tabs.get(self.active).map(|tab| tab.info.id),
                self.worker.search_token.load(Ordering::Relaxed),
            )
            .is_err()
        {
            ui.set_search_label("Search unavailable".into());
            ui.set_status("Search could not start: PDF worker is unavailable".into());
        }
    }

    fn next_result(&mut self, ui: &Workspace, direction: i32) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        if tab.hits.is_empty() {
            return;
        }
        tab.hit = (tab.hit as i32 + direction).rem_euclid(tab.hits.len() as i32) as usize;
        ui.set_search_label(
            format!(
                "{} / {} matches{}",
                tab.hit + 1,
                tab.hits.len(),
                if tab.search_truncated { "+" } else { "" }
            )
            .into(),
        );
        self.reveal_hit(ui);
    }

    fn refresh_markers(&self, ui: &Workspace) {
        self.refresh_marks(ui);
        self.refresh_selection(ui);
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let Some(layout) = &self.layout else {
            return;
        };
        let mut markers = Vec::new();
        for placement in layout
            .pages
            .iter()
            .filter(|p| p.near_viewport(ui.get_scroll_y(), ui.get_viewport_height()))
        {
            for (index, hit) in tab
                .hits
                .iter()
                .enumerate()
                .filter(|(_, h)| h.page == placement.page)
            {
                for rect in &hit.bounds {
                    let rect = rect.rotated(tab.view.rotation);
                    markers.push(SearchMarker {
                        px: placement.x + rect.x * placement.width,
                        py: placement.y + rect.y * placement.height,
                        pw: rect.width * placement.width,
                        ph: rect.height * placement.height,
                        active: index == tab.hit,
                    });
                }
            }
        }
        ui.set_search_markers(ModelRc::new(VecModel::from(markers)));
    }

    fn clear_selection(&mut self, ui: &Workspace) {
        self.selection = None;
        self.selection_token += 1;
        self.selected_text.clear();
        self.selection_ready = false;
        self.copy_when_ready = false;
        ui.set_text_selected(false);
        ui.set_selection_markers(ModelRc::default());
    }

    fn refresh_selection(&self, ui: &Workspace) {
        let markers = (|| {
            let selection = self.selection.as_ref()?;
            let tab = self.tabs.get(self.active)?;
            if tab.info.id != selection.document {
                return None;
            }
            let placement = self
                .layout
                .as_ref()?
                .pages
                .iter()
                .find(|p| p.page == selection.page)?;
            let rect = selection.bounds.rotated(tab.view.rotation);
            Some(SearchMarker {
                px: placement.x + rect.x * placement.width,
                py: placement.y + rect.y * placement.height,
                pw: rect.width * placement.width,
                ph: rect.height * placement.height,
                active: true,
            })
        })();
        ui.set_selection_markers(ModelRc::new(VecModel::from(
            markers.into_iter().collect::<Vec<_>>(),
        )));
    }

    fn select_text(
        &mut self,
        ui: &Workspace,
        page: usize,
        start: (f32, f32),
        end: (f32, f32),
        complete: bool,
    ) {
        if self.pending_mark.is_some() {
            if complete {
                self.place_mark(ui, page, end);
            }
            return;
        }
        if ui.get_review_visible() && ui.get_review_tool() > 0 {
            if complete {
                self.add_remark(ui, page, start, end);
            }
            return;
        }
        self.clear_selection(ui);
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        if page >= tab.info.pages.len()
            || ![start.0, start.1, end.0, end.1]
                .iter()
                .all(|x| x.is_finite())
        {
            return;
        }
        let (x0, y0, x1, y1) = (
            start.0.clamp(0.0, 1.0),
            start.1.clamp(0.0, 1.0),
            end.0.clamp(0.0, 1.0),
            end.1.clamp(0.0, 1.0),
        );
        let bounds = NormalizedRect {
            x: x0.min(x1),
            y: y0.min(y1),
            width: (x1 - x0).abs(),
            height: (y1 - y0).abs(),
        }
        .rotated((4 - tab.view.rotation % 4) % 4);
        if bounds.width < 0.001 || bounds.height < 0.001 {
            return;
        }
        let id = tab.info.id;
        self.selection = Some(TextSelection {
            document: id,
            page,
            bounds,
        });
        self.refresh_selection(ui);
        if complete {
            self.command(
                ui,
                Command::SelectText {
                    id,
                    page,
                    token: self.selection_token,
                    bounds,
                },
            );
        }
    }

    fn share_controls(&self, ui: &Workspace) {
        ui.set_share_ready(
            self.pending_mark.is_none()
                && self.tabs.get(self.active).is_some_and(|tab| !tab.unsaved()),
        );
    }

    fn share_file(&mut self, ui: &Workspace, action: i32) {
        if (ui.get_share_busy() && action == 0) || self.export_pending {
            return;
        }
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        if tab.unsaved() || self.pending_mark.is_some() {
            ui.set_status(
                "Save a copy first to include unsaved marks or remarks before sharing".into(),
            );
            return;
        }
        if action == 1 {
            let result = (|| -> anyhow::Result<()> {
                let path = platform::saved_file_path(&tab.info.path)?;
                if self.clipboard.is_none() {
                    self.clipboard = Some(arboard::Clipboard::new()?);
                }
                self.clipboard.as_mut().unwrap().set_text(path)?;
                Ok(())
            })();
            ui.set_status(match result {
                Ok(()) => "Copied saved PDF file path".into(),
                Err(error) => format!("Cannot copy saved file path: {error:#}").into(),
            });
        } else if action == 0 {
            let path = tab.info.path.clone();
            let weak = ui.as_weak();
            ui.set_share_busy(true);
            std::thread::spawn(move || {
                let result =
                    platform::open_file_folder(&path).map_err(|error| format!("{error:#}"));
                let _ = weak.upgrade_in_event_loop(move |window| {
                    window.set_share_busy(false);
                    window.set_status(match result {
                        Ok(()) => "Opened saved PDF folder".into(),
                        Err(error) => format!("Cannot open folder: {error}").into(),
                    });
                });
            });
        }
    }

    fn copy_text(&mut self, ui: &Workspace) {
        if self.selection.is_none() {
            return;
        }
        if !self.selection_ready {
            self.copy_when_ready = true;
            ui.set_status("Reading selected text…".into());
            return;
        }
        if self.selected_text.is_empty() {
            ui.set_status("No selectable text in this region".into());
            return;
        }
        let result = (|| -> anyhow::Result<()> {
            if self.clipboard.is_none() {
                self.clipboard = Some(arboard::Clipboard::new()?);
            }
            self.clipboard
                .as_mut()
                .expect("initialized clipboard")
                .set_text(self.selected_text.clone())?;
            Ok(())
        })();
        ui.set_status(
            match result {
                Ok(()) => format!("Copied {} characters", self.selected_text.chars().count()),
                Err(error) => format!("Clipboard unavailable: {error:#}"),
            }
            .into(),
        );
    }

    fn reveal_hit(&mut self, ui: &Workspace) {
        let Some(hit) = self
            .tabs
            .get(self.active)
            .and_then(|t| t.hits.get(t.hit))
            .cloned()
        else {
            return;
        };
        self.go_to(ui, hit.page);
        if let Some(rect) = hit.bounds.first()
            && let Some(layout) = &self.layout
            && let Some(placement) = layout.pages.iter().find(|p| p.page == hit.page)
        {
            let rect = rect.rotated(self.tabs[self.active].view.rotation);
            let y = placement.y + (rect.y + rect.height / 2.0) * placement.height;
            let x = placement.x + (rect.x + rect.width / 2.0) * placement.width;
            ui.set_scroll_y(
                (ui.get_viewport_height() / 2.0 - y)
                    .clamp((ui.get_viewport_height() - layout.height).min(0.0), 0.0),
            );
            ui.set_scroll_x(
                (ui.get_viewport_width() / 2.0 - x)
                    .clamp((ui.get_viewport_width() - layout.width).min(0.0), 0.0),
            );
        }
        self.refresh_markers(ui);
    }

    fn zoom_center(&mut self, ui: &Workspace, factor: f32) {
        let Some(layout) = &self.layout else {
            return;
        };
        let Some(anchor) = layout.viewport_anchor(
            (ui.get_viewport_width(), ui.get_viewport_height()),
            (ui.get_scroll_x(), ui.get_scroll_y()),
        ) else {
            return;
        };
        let Some(page) = layout.pages.iter().find(|p| p.page == anchor.page) else {
            return;
        };
        self.zoom_around(
            ui,
            factor,
            page.x + anchor.page_point.0 * page.width,
            page.y + anchor.page_point.1 * page.height,
        );
    }

    fn zoom_around(&mut self, ui: &Workspace, factor: f32, x: f32, y: f32) {
        let Some(layout) = &self.layout else {
            return;
        };
        let Some(placement) = layout
            .pages
            .iter()
            .find(|p| x >= p.x && x <= p.x + p.width && y >= p.y && y <= p.y + p.height)
        else {
            return;
        };
        let page = placement.page;
        let relative_x = (x - placement.x) / placement.width;
        let relative_y = (y - placement.y) / placement.height;
        let old_x = ui.get_scroll_x();
        let old_y = ui.get_scroll_y();
        self.tabs[self.active].view.zoom = Zoom::Scale((layout.scale * factor).clamp(0.1, 6.0));
        self.relayout(ui, false);
        if let Some(layout) = &self.layout
            && let Some(placement) = layout.pages.iter().find(|p| p.page == page)
        {
            ui.set_scroll_x(anchored_scroll(
                old_x,
                x,
                placement.x + relative_x * placement.width,
                ui.get_viewport_width(),
                layout.width,
            ));
            ui.set_scroll_y(anchored_scroll(
                old_y,
                y,
                placement.y + relative_y * placement.height,
                ui.get_viewport_height(),
                layout.height,
            ));
        }
        self.refresh_markers(ui);
    }

    fn request_visible(&mut self, ui: &Workspace) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        let Some(layout) = &self.layout else {
            return;
        };
        let previous_page = tab.view.page;
        let offset = ui.get_scroll_y();
        let height = ui.get_viewport_height();
        tab.view.scroll_y = offset;
        tab.view.scroll_x = ui.get_scroll_x();
        if tab.view.mode == ViewMode::Continuous
            && let Some(page) = layout
                .pages
                .iter()
                .find(|p| p.y + p.height > -offset + 20.0)
        {
            tab.view.page = page.page;
        }
        let id = tab.info.id;
        let rotation = tab.view.rotation;
        let scale = ui.window().scale_factor();
        let visible: Vec<_> = layout
            .pages
            .iter()
            .filter(|p| p.near_viewport(offset, height))
            .collect();
        let window: Vec<_> = visible.iter().map(|p| p.page).collect();
        if self.render_window != window {
            self.generation += 1;
            self.worker
                .generation
                .store(self.generation, Ordering::Relaxed);
            self.pending.clear();
            self.frames.retain(|page, _| window.contains(page));
            let old: HashMap<_, _> = self
                .pages
                .iter()
                .map(|tile| (tile.page as usize, tile))
                .collect();
            self.pages.set_vec(
                visible
                    .iter()
                    .map(|p| {
                        let previous = old.get(&p.page);
                        PageTile {
                            page: p.page as i32,
                            px: p.x,
                            py: p.y,
                            pw: p.width,
                            ph: p.height,
                            bitmap: previous.map(|tile| tile.bitmap.clone()).unwrap_or_default(),
                            ready: previous.is_some_and(|tile| tile.ready),
                        }
                    })
                    .collect::<Vec<_>>(),
            );
            self.render_window = window;
        }
        let mut keys = Vec::new();
        for placement in visible {
            let width = ((placement.width * scale / 64.0).ceil() as i32 * 64).clamp(64, 3072);
            let key = RenderKey {
                document: id,
                page: placement.page,
                width,
                rotation,
                thumbnail: false,
            };
            if self.frames.get(&placement.page) != Some(&key) {
                keys.push(key);
            }
        }
        if ui.get_sidebar_visible() && ui.get_sidebar_tab() == 0 {
            let first = (ui.get_sidebar_offset().max(0.0) / 174.0) as usize;
            let end = (first + (ui.get_sidebar_height().max(0.0) / 174.0).ceil() as usize + 2)
                .min(tab.info.pages.len());
            let visible: HashSet<_> = (first..end)
                .filter_map(|row| self.sidebar.row_data(row).map(|item| item.page as usize))
                .collect();
            for &page in &visible {
                if !self.thumb_frames.contains(&page) {
                    keys.push(RenderKey {
                        document: id,
                        page,
                        width: 160,
                        rotation: 0,
                        thumbnail: true,
                    });
                }
            }
            // Release thumbnails outside the sidebar viewport as well as full page images.
            let to_clear: Vec<_> = self
                .thumb_frames
                .iter()
                .filter(|p| !visible.contains(p))
                .copied()
                .collect();
            for page in to_clear {
                self.thumb_frames.remove(&page);
                if let Some(row) = self
                    .sidebar
                    .iter()
                    .position(|item| item.page as usize == page)
                    && let Some(mut data) = self.sidebar.row_data(row)
                {
                    data.bitmap = Default::default();
                    self.sidebar.set_row_data(row, data);
                }
            }
        }
        for key in keys {
            if self.pending.contains(&key) || self.failed.contains(&key) {
                continue;
            }
            if self
                .worker
                .commands
                .try_send(Command::Render {
                    generation: self.generation,
                    key: key.clone(),
                })
                .is_ok()
            {
                self.pending.insert(key);
            } else {
                break;
            }
        }
        if self
            .tabs
            .get(self.active)
            .is_some_and(|t| t.view.page != previous_page)
        {
            self.update_page_label(ui);
        }
    }

    fn tick(&mut self, ui: &Workspace) {
        self.flush_search(ui);
        self.scroll_page_drag(ui);
        if let Some(error) = self
            .asset_usage
            .as_ref()
            .and_then(|worker| worker.poll_error())
        {
            ui.set_status(format!("Mark placed. Cannot record last-used time: {error}").into());
        }
        self.recovery_tick(ui);
        if let Some(tab) = self.tabs.get(self.active)
            && self.review_auto
            && !tab.review_requested
            && !self.frames.is_empty()
        {
            self.request_review(ui, tab.info.id);
        }
        if let Some(studio) = &self.mark_studio {
            studio.set_document(self.tabs.get(self.active).map(|t| t.info.id));
            if let Some(mark) = studio.take_mark() {
                if let Some(index) = self.tabs.iter().position(|t| t.info.id == mark.document) {
                    self.activate(ui, index);
                    self.pending_mark = Some(mark);
                    ui.set_review_tool(0);
                    self.mark_controls(ui);
                    self.clear_selection(ui);
                    ui.set_mark_pending(true);
                    ui.set_status("Click a page to place the mark; Esc cancels".into());
                } else {
                    ui.set_status("Placement cancelled: the target document was closed".into());
                }
            }
        }
        if ui.get_arranging() {
            let document = self.tabs.get(self.active).map(|t| t.info.id);
            if self.arrangement.as_ref().map(|a| a.document) != document {
                if let Some(tab) = self.tabs.get(self.active) {
                    ui.set_operation_pages(format!("1-{}", tab.info.pages.len()).into());
                }
                self.apply_page_order(ui);
            }
        } else if self.arrangement.take().is_some() {
            self.rebuild_sidebar(ui);
        }
        if let Ok(result) = self.printer_results.try_recv() {
            self.printer_loading = false;
            ui.set_printer_loading(false);
            match result {
                Ok(names) => {
                    ui.set_printer_note(if names.is_empty() { "No printers found. Use Other to enter a name, or configure a system printer." } else { "Blank range prints all pages. System default follows your desktop printer setting." }.into());
                    self.printer_names = names;
                    self.printer_choices(ui);
                }
                Err(message) => ui.set_printer_note(message.into()),
            }
        }
        // Limit each tick so a burst of thumbnails cannot monopolize the event loop.
        for _ in 0..8 {
            let Ok(event) = self.worker.events.try_recv() else {
                break;
            };
            match event {
                Event::Ready => {
                    ui.set_status(
                        self.preferences_warning
                            .as_deref()
                            .unwrap_or("Ready — open a PDF")
                            .into(),
                    );
                }
                Event::Unavailable(message) => {
                    self.engine_error = Some(message.clone());
                    self.passwords.clear();
                    self.password_controls(ui);
                    self.opening.clear();
                    ui.set_busy(false);
                    ui.set_status(message.into());
                }
                Event::Snapshot(id, bytes) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.info.id == id) {
                        tab.recovery.snapshot_pending = false;
                        if tab.unsaved() {
                            tab.recovery.original = Some(bytes);
                        }
                    }
                }
                Event::PageUnitsLoaded(id, generation, result) => {
                    if self
                        .tabs
                        .iter()
                        .any(|t| t.info.id == id && t.review_generation == generation)
                    {
                        self.page_units_loaded(ui, id, result);
                    }
                }
                Event::ReviewLoaded(id, generation, result) => {
                    if let Some(tab) = self
                        .tabs
                        .iter_mut()
                        .find(|t| t.info.id == id && t.review_generation == generation)
                    {
                        tab.review_ready = result.is_ok();
                        tab.review_error = result.as_ref().err().cloned();
                        match result {
                            Ok(model) => {
                                let present = model.is_some();
                                let model = model.unwrap_or_default();
                                if let Some(review) = &mut tab.review
                                    && review.dirty
                                {
                                    review.baseline = model;
                                    review.present = present;
                                } else {
                                    tab.review = Some(ReviewState::new(
                                        model.clone(),
                                        model,
                                        present,
                                        false,
                                    ));
                                }
                                if self.tabs[self.active].info.id == id {
                                    self.review_controls(ui);
                                    self.relayout(ui, false);
                                    self.refresh_reviews(ui);
                                }
                            }
                            Err(message) => {
                                if ui.get_review_visible() {
                                    ui.set_status(format!("Review unavailable: {message}").into());
                                }
                                self.review_controls(ui);
                            }
                        }
                    }
                }
                Event::PasswordRequired(id, path, wrong) => {
                    let first = self.passwords.is_empty();
                    self.passwords.push_back((id, path, wrong));
                    if first {
                        self.password_controls(ui);
                    }
                }
                Event::Opened(mut info) => {
                    let raw_sizes = info.pages.clone();
                    let units_ready = info.physical_pages.is_some();
                    if let Some(physical) = info.physical_pages.take() {
                        info.pages = physical;
                    }
                    self.opening.remove(&info.id);
                    let recovery = if self
                        .recovering
                        .as_ref()
                        .is_some_and(|(id, _, _)| *id == info.id)
                    {
                        ui.set_recovery_busy(false);
                        self.recovering.take()
                    } else {
                        None
                    };
                    if let Some((_, _, draft)) = &recovery
                        && let Err(error) = draft.validate_pages(&info.pages)
                    {
                        self.command(ui, Command::Close(info.id));
                        ui.set_busy(!self.opening.is_empty());
                        ui.set_status(format!("Cannot recover draft: {error:#}").into());
                        continue;
                    }
                    let signed = info.signatures > 0;
                    let protected = info.password_protected;
                    let restored = if self
                        .reopen_view
                        .as_ref()
                        .is_some_and(|(path, _)| path == &info.path)
                    {
                        self.reopen_view.take().map(|(_, view)| view)
                    } else {
                        None
                    };
                    let recovered = recovery.is_some();
                    let (marks, recovery_state, recovered_review) = if let Some((_, path, draft)) =
                        recovery
                    {
                        let mut previews = HashMap::new();
                        let items = draft
                            .marks
                            .into_iter()
                            .map(|m| {
                                let images = previews
                                    .entry(std::sync::Arc::as_ptr(&m.mark.placement.image) as usize)
                                    .or_insert_with(|| {
                                        let make = |rgba: &image::RgbaImage| {
                                            slint::Image::from_rgba8(
                                                SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                                                    rgba.as_raw(),
                                                    rgba.width(),
                                                    rgba.height(),
                                                ),
                                            )
                                        };
                                        [
                                            make(&m.preview),
                                            make(&image::imageops::rotate90(m.preview.as_ref())),
                                            make(&image::imageops::rotate180(m.preview.as_ref())),
                                            make(&image::imageops::rotate270(m.preview.as_ref())),
                                        ]
                                    });
                                crate::placement_ui::Mark {
                                    placement: m.mark.placement,
                                    previews: images.clone(),
                                    default_size: m.mark.default_size,
                                    aspect_locked: m.mark.aspect_locked,
                                }
                            })
                            .collect();
                        let mut marks = crate::placement_ui::Drafts::default();
                        marks.restore(items, draft.selected);
                        (
                            marks,
                            RecoveryState {
                                original: Some(draft.original),
                                supersedes: Some(path),
                                ..Default::default()
                            },
                            draft.review,
                        )
                    } else {
                        (Default::default(), Default::default(), None)
                    };
                    self.tabs.push(Tab {
                        review: recovered_review.map(|model| {
                            ReviewState::new(model, crate::review::Review::default(), false, true)
                        }),
                        review_requested: false,
                        review_generation: 0,
                        review_ready: false,
                        review_error: None,
                        recovery: recovery_state,
                        marks,
                        raw_sizes,
                        units_ready,
                        units_error: None,
                        info,
                        view: restored.unwrap_or_else(|| ViewState {
                            mode: match ui.get_view_mode() {
                                1 => ViewMode::Continuous,
                                2 => ViewMode::Facing,
                                _ => ViewMode::Single,
                            },
                            ..Default::default()
                        }),
                        query: String::new(),
                        hits: vec![],
                        search_truncated: false,
                        hit: 0,
                    });
                    self.activate(ui, self.tabs.len() - 1);
                    if recovered
                        && let Some(page) = self.tabs[self.active]
                            .marks
                            .selected
                            .and_then(|i| self.tabs[self.active].marks.items.get(i))
                            .or_else(|| self.tabs[self.active].marks.items.first())
                            .map(|m| m.placement.page)
                    {
                        self.go_to(ui, page);
                    }
                    ui.set_busy(!self.opening.is_empty());
                    ui.set_status(
                        if recovered {
                            "Recovered unsaved marks from a local snapshot — source file unchanged"
                        } else if protected {
                            "Opened protected PDF — Save As preserves encryption; edited copies are unencrypted"
                        } else if signed {
                            "Opened — contains digital signatures; Save As preserves original bytes"
                        } else {
                            "Opened — view rotation and zoom do not change the PDF"
                        }
                        .into(),
                    );
                }
                Event::OpenFailed(id, message) => {
                    if self
                        .recovering
                        .as_ref()
                        .is_some_and(|(pending, _, _)| *pending == id)
                    {
                        self.recovering = None;
                        ui.set_recovery_busy(false);
                    }
                    self.opening.remove(&id);
                    ui.set_busy(!self.opening.is_empty());
                    ui.set_status(message.into());
                }
                Event::Rendered {
                    generation,
                    key,
                    frame,
                } => {
                    if generation != self.generation {
                        continue;
                    }
                    self.pending.remove(&key);
                    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                        &frame.rgba,
                        frame.width,
                        frame.height,
                    );
                    let image = slint::Image::from_rgba8(buffer);
                    if key.thumbnail {
                        if ui.get_sidebar_tab() == 0
                            && let Some(row) = self
                                .sidebar
                                .iter()
                                .position(|item| item.page as usize == key.page)
                            && let Some(mut data) = self.sidebar.row_data(row)
                        {
                            data.bitmap = image;
                            self.sidebar.set_row_data(row, data);
                            self.thumb_frames.insert(key.page);
                        }
                    } else if let Some(row) = self
                        .pages
                        .iter()
                        .position(|tile| tile.page as usize == key.page)
                        && let Some(mut data) = self.pages.row_data(row)
                    {
                        data.bitmap = image;
                        data.ready = true;
                        self.pages.set_row_data(row, data);
                        self.frames.insert(key.page, key);
                    }
                }
                Event::RenderFailed {
                    generation,
                    key,
                    message,
                } => {
                    if generation == self.generation {
                        self.pending.remove(&key);
                        self.failed.insert(key);
                        ui.set_status(format!("Render failed: {message}").into());
                    }
                }
                Event::SearchResults {
                    id,
                    token,
                    hits,
                    truncated,
                } => {
                    if token != self.worker.search_token.load(Ordering::Relaxed) {
                        continue;
                    }
                    if let Some(tab) = self.tabs.get_mut(self.active).filter(|t| t.info.id == id) {
                        tab.hits = hits;
                        tab.search_truncated = truncated;
                        tab.hit = 0;
                        if !tab.hits.is_empty() {
                            ui.set_search_label(
                                format!(
                                    "1 / {} matches{}",
                                    tab.hits.len(),
                                    if truncated { "+" } else { "" }
                                )
                                .into(),
                            );
                            self.reveal_hit(ui);
                        } else {
                            ui.set_search_label("No matches".into());
                            self.refresh_markers(ui);
                        }
                    }
                }
                Event::SelectedText { id, token, text } => {
                    if token != self.selection_token
                        || !self.selection.as_ref().is_some_and(|s| s.document == id)
                    {
                        continue;
                    }
                    self.selection_ready = true;
                    match text {
                        Ok(text) => {
                            self.selected_text = text;
                            ui.set_text_selected(!self.selected_text.is_empty());
                            ui.set_status(if self.selected_text.is_empty() {
                                "No selectable text in this region".into()
                            } else {
                                format!(
                                    "{} characters selected — Ctrl+C to copy",
                                    self.selected_text.chars().count()
                                )
                                .into()
                            });
                            if self.copy_when_ready {
                                self.copy_when_ready = false;
                                self.copy_text(ui);
                            }
                        }
                        Err(message) => {
                            ui.set_status(format!("Text selection failed: {message}").into())
                        }
                    }
                }
                Event::ExportFinished(result) => {
                    let mark_export = self.mark_export.take();
                    self.export_pending = false;
                    ui.set_export_busy(false);
                    match result {
                        Ok(result) => {
                            if let Some((id, generation, view, clean)) = mark_export {
                                self.reopen_view = Some((result.path.clone(), view));
                                if let Some(tab) = self.tabs.iter_mut().find(|t| t.info.id == id)
                                    && !clean
                                    && tab.draft_generation() == generation
                                {
                                    tab.marks.clear_after_save(tab.marks.generation);
                                    if let Some(review) = &mut tab.review {
                                        review.dirty = false;
                                        review.model = review.baseline.clone();
                                        review.generation += 1;
                                        review.clear_history();
                                    }
                                }
                                self.tabs_model(ui);
                                self.mark_controls(ui);
                                self.review_controls(ui);
                                self.refresh_marks(ui);
                            }
                            ui.set_status(
                                format!(
                                    "Saved {} pages to {}",
                                    result.pages,
                                    result.path.display()
                                )
                                .into(),
                            );
                            self.open(ui, result.path);
                        }
                        Err(message) => {
                            ui.set_status(format!("Page export failed: {message}").into())
                        }
                    }
                }
                Event::PrintFinished(result) => {
                    self.print_pending = false;
                    ui.set_print_busy(false);
                    ui.set_status(
                        match result {
                            Ok(message) => message,
                            Err(message) => format!("Print failed: {message}"),
                        }
                        .into(),
                    );
                }
                Event::Notice(message) => ui.set_status(message.into()),
            }
        }
        let viewport = (
            ui.get_viewport_width() as i32,
            ui.get_viewport_height() as i32,
        );
        if viewport != self.viewport && !self.tabs.is_empty() {
            let anchor = self.resize_anchor.or_else(|| {
                self.layout.as_ref()?.viewport_anchor(
                    (self.viewport.0 as f32, self.viewport.1 as f32),
                    (ui.get_scroll_x(), ui.get_scroll_y()),
                )
            });
            self.relayout(ui, false);
            if let Some((x, y)) = anchor.and_then(|anchor| {
                self.layout.as_ref()?.restore_viewport_anchor(
                    anchor,
                    (ui.get_viewport_width(), ui.get_viewport_height()),
                )
            }) {
                ui.set_scroll_x(x);
                ui.set_scroll_y(y);
            }
            self.refresh_markers(ui);
        }
        let tab_width = ui.get_tab_strip_width() as i32;
        if !self.tabs.is_empty()
            && tab_width > 0
            && (self.reveal_active_tab || tab_width != self.tab_strip_width)
        {
            ui.set_tab_strip_x(-tab_scroll_offset(
                self.active,
                self.tabs.len(),
                ui.get_tab_width(),
                ui.get_tab_strip_width(),
                -ui.get_tab_strip_x(),
            ));
            self.reveal_active_tab = false;
        }
        self.tab_strip_width = tab_width;
        self.remember_preferences(ui, false);
        self.request_visible(ui);
        if self.marker_offset != ui.get_scroll_y() {
            self.marker_offset = ui.get_scroll_y();
            self.refresh_markers(ui);
        }
        self.remember_resize_anchor(ui);
    }

    fn remember_resize_anchor(&mut self, ui: &Workspace) {
        self.resize_anchor = self.layout.as_ref().and_then(|layout| {
            layout.viewport_anchor(
                (ui.get_viewport_width(), ui.get_viewport_height()),
                (ui.get_scroll_x(), ui.get_scroll_y()),
            )
        });
    }

    fn export_pages(&mut self, ui: &Workspace) {
        if self.guard_marks(ui) {
            return;
        }
        if self.export_pending {
            return;
        }
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let operation = PageOperation {
            kind: OperationKind::from_index(ui.get_page_operation()),
            pages: ui.get_operation_pages().to_string(),
            source_pages: ui.get_operation_source_pages().to_string(),
        };
        if !operation.kind.uses_source()
            && let Err(error) = crate::assembly::build_plan(vec![tab.info.pages.len()], &operation)
        {
            ui.set_status(error.to_string().into());
            return;
        }
        let mut additional = Vec::new();
        if operation.kind.uses_source() {
            let dialog = rfd::FileDialog::new()
                .add_filter("PDF or scanned page", &["pdf", "png", "jpg", "jpeg"])
                .add_filter("PDF document", &["pdf"])
                .add_filter("Page image", &["png", "jpg", "jpeg"]);
            if operation.kind == OperationKind::Merge {
                let Some(paths) = dialog.pick_files() else {
                    return;
                };
                additional = paths;
            } else {
                let Some(path) = dialog.pick_file() else {
                    return;
                };
                additional.push(path);
            }
        }
        if tab.info.password_protected && !confirm_unencrypted_copy() {
            return;
        }
        if tab.info.signatures > 0 && rfd::MessageDialog::new().set_title("Existing digital signatures")
            .set_level(rfd::MessageLevel::Warning)
            .set_description("This PDF contains cryptographic signatures. Page assembly will invalidate them in the edited copy. The source file will be preserved.")
            .set_buttons(rfd::MessageButtons::OkCancel).show() != rfd::MessageDialogResult::Ok { return; }
        let filename = format!(
            "{}-edited.pdf",
            tab.info
                .path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
        );
        let Some(mut destination) = rfd::FileDialog::new()
            .add_filter("PDF document", &["pdf"])
            .set_file_name(filename)
            .save_file()
        else {
            return;
        };
        if destination.extension().is_none() {
            destination.set_extension("pdf");
        }
        let id = tab.info.id;
        if self.command(
            ui,
            Command::ExportPages {
                id,
                operation,
                additional,
                destination,
            },
        ) {
            self.export_pending = true;
            ui.set_export_busy(true);
            ui.set_status("Assembling pages…".into());
        }
    }

    fn print(&mut self, ui: &Workspace) {
        if self.guard_marks(ui) {
            return;
        }
        if self.print_pending {
            ui.set_status("A print job is already being submitted".into());
            return;
        }
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let range = match platform::validate_range(&ui.get_print_range(), tab.info.pages.len()) {
            Ok(range) => range,
            Err(error) => {
                ui.set_status(error.to_string().into());
                return;
            }
        };
        let command = if let Some(review) = &tab.review
            && !review.model.remarks.is_empty()
        {
            Command::PrintReviewed {
                id: tab.info.id,
                review: review.model.clone(),
                current: tab.view.page,
                range,
                printer: ui.get_printer_name().to_string(),
                fit: ui.get_print_scaling() == 0,
                native: false,
            }
        } else {
            Command::Print {
                id: tab.info.id,
                range,
                printer: ui.get_printer_name().to_string(),
                fit: ui.get_print_scaling() == 0,
            }
        };
        if self.command(ui, command) {
            self.print_pending = true;
            ui.set_print_busy(true);
            ui.set_print_visible(false);
            ui.set_status("Submitting print job…".into());
        }
    }
}

fn confirm_unencrypted_copy() -> bool {
    rfd::MessageDialog::new().set_title("Save an unencrypted edited copy?")
        .set_description("This PDF was opened with a password. The edited copy will not require that password. The encrypted source remains unchanged.")
        .set_buttons(rfd::MessageButtons::OkCancel).show()==rfd::MessageDialogResult::Ok
}

fn confirm_discard() -> bool {
    rfd::MessageDialog::new()
        .set_title("Discard unsaved edits?")
        .set_description(
            "Placed marks or review changes have not been saved. Discard them and close?",
        )
        .set_buttons(rfd::MessageButtons::OkCancel)
        .show()
        == rfd::MessageDialogResult::Ok
}

impl Application {
    fn guard_marks(&self, ui: &Workspace) -> bool {
        let blocked =
            self.tabs.get(self.active).is_some_and(Tab::unsaved) || self.pending_mark.is_some();
        if blocked {
            ui.set_status("Save the edited copy first, then print or change its pages. Unsaved marks/review remain in this tab.".into());
        }
        blocked
    }
    fn mark_controls(&self, ui: &Workspace) {
        self.share_controls(ui);
        let tab = self.tabs.get(self.active);
        ui.set_has_marks(tab.is_some_and(|t| !t.marks.items.is_empty()));
        ui.set_can_undo_mark(
            self.pending_mark.is_some() || tab.is_some_and(|t| t.marks.can_undo()),
        );
        ui.set_can_redo_mark(
            self.pending_mark.is_none() && tab.is_some_and(|t| t.marks.can_redo()),
        );
        let mark = tab.and_then(|t| t.marks.selected.and_then(|i| t.marks.items.get(i)));
        ui.set_mark_selected(mark.is_some());
        if let Some(mark) = mark {
            ui.set_mark_width(format!("{:.3}", mark.placement.geometry.width_mm).into());
            ui.set_mark_height(format!("{:.3}", mark.placement.geometry.height_mm).into());
            ui.set_mark_opacity(format!("{:.1}", mark.placement.opacity * 100.0).into());
            ui.set_mark_lock(mark.aspect_locked);
        }
    }
    fn refresh_marks(&self, ui: &Workspace) {
        self.refresh_reviews(ui);
        let mut tiles = Vec::new();
        if let (Some(tab), Some(layout)) = (self.tabs.get(self.active), &self.layout) {
            for (index, mark) in tab.marks.items.iter().enumerate() {
                let Some(page) = layout.pages.iter().find(|p| p.page == mark.placement.page) else {
                    continue;
                };
                if !page.near_viewport(ui.get_scroll_y(), ui.get_viewport_height()) {
                    continue;
                }
                let size = tab.info.pages[page.page];
                let Ok(rect) = mark.placement.geometry.normalized(size.width, size.height) else {
                    continue;
                };
                let rect = rect.rotated(tab.view.rotation);
                tiles.push(MarkTile {
                    index: index as i32,
                    px: page.x + rect.x * page.width,
                    py: page.y + rect.y * page.height,
                    pw: rect.width * page.width,
                    ph: rect.height * page.height,
                    bitmap: mark.previews[((mark.placement.geometry.quarter_turns
                        + tab.view.rotation)
                        % 4) as usize]
                        .clone(),
                    opacity: mark.placement.opacity,
                    selected: tab.marks.selected == Some(index),
                });
            }
        }
        if self.mark_tiles.row_count() == tiles.len() {
            for (index, tile) in tiles.into_iter().enumerate() {
                self.mark_tiles.set_row_data(index, tile);
            }
        } else {
            self.mark_tiles.set_vec(tiles);
        }
    }
    fn page_units_loaded(
        &mut self,
        ui: &Workspace,
        id: DocumentId,
        result: Result<Vec<f64>, String>,
    ) {
        let Some(index) = self.tabs.iter().position(|tab| tab.info.id == id) else {
            return;
        };
        let dimensions = result.and_then(|units| {
            physical_page_sizes(&self.tabs[index].raw_sizes, &units)
                .ok_or_else(|| "Invalid physical PDF page dimensions".to_string())
        });
        let dimensions = match dimensions {
            Ok(value) => value,
            Err(error) => {
                self.tabs[index].units_error = Some(error);
                return;
            }
        };
        let changed = self.tabs[index]
            .info
            .pages
            .iter()
            .zip(&dimensions)
            .any(|(old, new)| old.width != new.width || old.height != new.height);
        let anchor = if changed && index == self.active {
            self.layout.as_ref().and_then(|layout| {
                layout.viewport_anchor(
                    (ui.get_viewport_width(), ui.get_viewport_height()),
                    (ui.get_scroll_x(), ui.get_scroll_y()),
                )
            })
        } else {
            None
        };
        let tab = &mut self.tabs[index];
        tab.info.pages = dimensions;
        tab.units_ready = true;
        tab.units_error = None;
        if changed && index == self.active {
            self.relayout(ui, false);
            if let Some((x, y)) = anchor.and_then(|anchor| {
                self.layout.as_ref()?.restore_viewport_anchor(
                    anchor,
                    (ui.get_viewport_width(), ui.get_viewport_height()),
                )
            }) {
                ui.set_scroll_x(x);
                ui.set_scroll_y(y);
            }
            self.refresh_markers(ui);
        }
    }

    fn place_mark(&mut self, ui: &Workspace, page: usize, point: (f32, f32)) {
        let (Some(mark), Some(tab)) = (&self.pending_mark, self.tabs.get_mut(self.active)) else {
            return;
        };
        if self.review_auto && !tab.units_ready {
            ui.set_status(
                if tab.units_error.is_some() {
                    "Cannot place a mark: physical PDF page dimensions could not be verified."
                } else {
                    "Checking physical page dimensions; try placement again shortly."
                }
                .into(),
            );
            return;
        }
        let Some(size) = tab.info.pages.get(page).copied() else {
            return;
        };
        if tab.info.id != mark.document {
            return;
        }
        let budget: usize = tab
            .marks
            .items
            .iter()
            .map(|m| m.placement.image.len())
            .sum();
        let preview_bytes = |images: &[slint::Image; 4]| {
            images
                .iter()
                .map(|image| u64::from(image.size().width) * u64::from(image.size().height) * 4)
                .sum::<u64>()
        };
        let preview_total = tab
            .marks
            .items
            .iter()
            .map(|m| preview_bytes(&m.previews))
            .sum::<u64>()
            + preview_bytes(&mark.previews);
        if tab.marks.items.len() >= 256
            || preview_total > 64 * 1024 * 1024
            || budget.saturating_add(mark.bytes.len()) > 64 * 1024 * 1024
        {
            ui.set_status("Draft limit reached (256 marks / 64 MiB images / 64 MiB previews); save a copy first".into());
            return;
        }
        let geometry = match crate::placement_ui::at_point(
            size,
            tab.view.rotation,
            point,
            mark.size.width_mm,
            mark.size.height_mm,
        ) {
            Ok(g) => g,
            Err(error) => {
                ui.set_status(error.to_string().into());
                return;
            }
        };
        let mark = self.pending_mark.take().unwrap();
        let asset_use = mark.asset_use;
        tab.marks.insert(crate::placement_ui::Mark {
            placement: crate::overlay::Placement {
                page,
                geometry,
                opacity: 1.0,
                image: mark.bytes,
            },
            previews: mark.previews,
            default_size: (mark.size.width_mm, mark.size.height_mm),
            aspect_locked: mark.size.aspect_locked,
        });
        ui.set_mark_pending(false);
        self.mark_controls(ui);
        self.refresh_marks(ui);
        self.tabs_model(ui);
        ui.set_status("Mark placed — drag to move or resize; Save copy to export".into());
        if let Some(asset) = asset_use {
            let result = self
                .asset_usage
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Asset usage worker is unavailable"))
                .and_then(|worker| worker.record(asset));
            if let Err(error) = result {
                ui.set_status(
                    format!("Mark placed. Cannot record last-used time: {error:#}").into(),
                );
            }
        }
    }
    fn begin_mark(&mut self, ui: &Workspace, index: usize, point: (f32, f32), resize: bool) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        let Some(mark) = tab.marks.items.get(index) else {
            return;
        };
        let size = tab.info.pages[mark.placement.page];
        let lock = (tab.marks.selected == Some(index)).then(|| ui.get_mark_lock());
        tab.marks
            .begin(index, point, resize, size, tab.view.rotation, lock);
        self.mark_controls(ui);
        self.refresh_marks(ui);
    }
    fn drag_mark(&mut self, ui: &Workspace, point: (f32, f32), finish: bool) {
        let (Some(tab), Some(layout)) = (self.tabs.get_mut(self.active), &self.layout) else {
            return;
        };
        let Some(mark) = tab.marks.selected.and_then(|i| tab.marks.items.get(i)) else {
            return;
        };
        let Some(page) = layout.pages.iter().find(|p| p.page == mark.placement.page) else {
            return;
        };
        let size = tab.info.pages[page.page];
        if let Err(error) = tab.marks.drag(
            point,
            (page.width, page.height),
            size,
            tab.view.rotation,
            finish,
        ) {
            ui.set_status(error.to_string().into());
        }
        self.mark_controls(ui);
        self.refresh_marks(ui);
        self.tabs_model(ui);
    }
    fn size_mark(&mut self, ui: &Workspace, reset: bool) {
        let result = (|| -> anyhow::Result<()> {
            let tab = self
                .tabs
                .get_mut(self.active)
                .ok_or_else(|| anyhow::anyhow!("Open a document"))?;
            let index = tab
                .marks
                .selected
                .ok_or_else(|| anyhow::anyhow!("Select a mark"))?;
            let mark = &mut tab.marks.items[index];
            let old = mark.placement.geometry;
            let (mut width, mut height) = if reset {
                mark.default_size
            } else {
                (
                    ui.get_mark_width().trim().parse::<f32>()?,
                    ui.get_mark_height().trim().parse::<f32>()?,
                )
            };
            if ui.get_mark_lock() && !reset {
                if (width - old.width_mm).abs() > 0.0005 {
                    height = width * old.height_mm / old.width_mm;
                } else {
                    width = height * old.width_mm / old.height_mm;
                }
            }
            if (width - old.width_mm).abs() < 0.0005 {
                width = old.width_mm;
            }
            if (height - old.height_mm).abs() < 0.0005 {
                height = old.height_mm;
            }
            let opacity = ui.get_mark_opacity().trim().parse::<f32>()? / 100.0;
            if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
                anyhow::bail!("Opacity must be between 0 and 100 percent");
            }
            let geometry = crate::overlay::Geometry {
                width_mm: width,
                height_mm: height,
                ..old
            };
            let size = tab.info.pages[mark.placement.page];
            geometry.normalized(size.width, size.height)?;
            tab.marks.edit(index, geometry, opacity, ui.get_mark_lock());
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.mark_controls(ui);
                self.refresh_marks(ui);
                self.tabs_model(ui);
            }
            Err(error) => ui.set_status(error.to_string().into()),
        }
    }
    fn export_marks(&mut self, ui: &Workspace) {
        if self.export_pending {
            return;
        }
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        if self.pending_mark.is_some() {
            ui.set_status("Place the pending mark or cancel it before saving".into());
            return;
        }
        if tab.info.password_protected && !confirm_unencrypted_copy() {
            return;
        }
        if tab.info.signatures>0 && rfd::MessageDialog::new().set_title("Existing digital signatures").set_description("Adding marks invalidates existing cryptographic signatures in the copy. The original is preserved.").set_buttons(rfd::MessageButtons::OkCancel).show()!=rfd::MessageDialogResult::Ok{return;}
        let filename = format!(
            "{}-marked.pdf",
            tab.info
                .path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
        );
        let Some(mut destination) = rfd::FileDialog::new()
            .add_filter("PDF document", &["pdf"])
            .set_file_name(filename)
            .save_file()
        else {
            return;
        };
        if destination.extension().is_none() {
            destination.set_extension("pdf");
        }
        self.save_marks_to(ui, destination);
    }
    fn cancel_mark_drag(&mut self, ui: &Workspace) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.marks.cancel_drag();
        }
        self.mark_controls(ui);
        self.refresh_marks(ui);
        self.tabs_model(ui);
    }
    fn history_mark(&mut self, ui: &Workspace, redo: bool) {
        if self.pending_mark.is_some() {
            if !redo {
                self.pending_mark = None;
                ui.set_mark_pending(false);
            }
        } else if let Some(tab) = self.tabs.get_mut(self.active) {
            let changed = if redo {
                tab.marks.redo()
            } else {
                tab.marks.undo()
            };
            if changed
                && let Some(page) = tab
                    .marks
                    .selected
                    .and_then(|i| tab.marks.items.get(i))
                    .map(|m| m.placement.page)
            {
                self.go_to(ui, page);
            }
        }
        self.mark_controls(ui);
        self.refresh_marks(ui);
        self.tabs_model(ui);
    }
    fn save_marks_to(&mut self, ui: &Workspace, destination: PathBuf) {
        self.save_workspace_to(ui, destination, crate::review::ExportMode::Current);
    }
    fn save_workspace_to(
        &mut self,
        ui: &Workspace,
        destination: PathBuf,
        mode: crate::review::ExportMode,
    ) {
        if self.export_pending {
            return;
        }
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.marks.finish_drag();
        }
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let protected: Vec<_> = self
            .tabs
            .iter()
            .map(|tab| tab.info.path.as_path())
            .collect();
        if let Err(error) = crate::storage::ensure_separate_output(&destination, &protected) {
            ui.set_status(error.to_string().into());
            return;
        }
        let mut view = tab.view.clone();
        view.scroll_x = ui.get_scroll_x();
        view.scroll_y = ui.get_scroll_y();
        let (id, generation) = (tab.info.id, tab.draft_generation());
        let placements = tab
            .marks
            .items
            .iter()
            .map(|m| m.placement.clone())
            .collect();
        let command = if let Some(review) = &tab.review
            && (review.present || review.dirty || !review.model.remarks.is_empty())
        {
            Command::ExportWorkspace {
                id,
                placements,
                review: review.model.clone(),
                mode,
                destination,
            }
        } else {
            Command::ExportMarks {
                id,
                placements,
                destination,
            }
        };
        if self.command(ui, command) {
            self.mark_export = Some((
                id,
                generation,
                view,
                matches!(mode, crate::review::ExportMode::Clean),
            ));
            self.export_pending = true;
            ui.set_export_busy(true);
            ui.set_status("Saving ordinary image overlays into a separate copy…".into());
        }
    }
}

pub fn run(paths: Vec<PathBuf>) -> Result<(), slint::PlatformError> {
    run_window(paths, false)
}

pub fn smoke(path: PathBuf) -> Result<(), slint::PlatformError> {
    run_window(vec![path], true)
}

fn cleared_message(count: usize) -> String {
    match count {
        0 => "No local recovery drafts to delete".into(),
        1 => "Deleted 1 local recovery draft".into(),
        n => format!("Deleted {n} local recovery drafts"),
    }
}

fn run_window(paths: Vec<PathBuf>, smoke: bool) -> Result<(), slint::PlatformError> {
    let ui = Workspace::new()?;
    slint::set_xdg_app_id("net.dpdf.dPDF")?;
    ui.set_native_printing(cfg!(any(target_os = "linux", target_os = "windows")) && !smoke);
    ui.set_app_version(env!("CARGO_PKG_VERSION").into());
    let smoke_preferences = std::env::var_os("DPDF_SMOKE_PREFERENCES").is_some();
    if smoke
        && std::env::var_os("DPDF_SMOKE_HELPER").is_some()
        && (std::env::var_os("DPDF_CONFIG_DIR").is_none()
            || std::env::var_os("DPDF_DATA_DIR").is_none())
    {
        return Err(slint::PlatformError::Other(
            "Helper diagnostic requires isolated config and data directories".into(),
        ));
    }
    if smoke && smoke_preferences && std::env::var_os("DPDF_CONFIG_DIR").is_none() {
        return Err(slint::PlatformError::Other(
            "Preference smoke test requires an explicit DPDF_CONFIG_DIR".into(),
        ));
    }
    if smoke
        && std::env::var_os("DPDF_SMOKE_RECOVERY").is_some()
        && std::env::var_os("DPDF_DATA_DIR").is_none()
    {
        return Err(slint::PlatformError::Other(
            "Recovery diagnostic requires an explicit DPDF_DATA_DIR".into(),
        ));
    }
    if smoke
        && std::env::var_os("DPDF_SMOKE_PASSWORD").is_some()
        && std::env::var_os("DPDF_SMOKE_PASSWORD_VALUE").is_none()
    {
        return Err(slint::PlatformError::Other(
            "Password diagnostic requires an explicit synthetic test value".into(),
        ));
    }
    let app = Rc::new(RefCell::new(Application::new(
        smoke && !smoke_preferences,
        !smoke || std::env::var_os("DPDF_SMOKE_RECOVERY").is_some(),
        !smoke
            || std::env::var_os("DPDF_SMOKE_REVIEW").is_some()
            || std::env::var_os("DPDF_SMOKE_HELPER").is_some()
            || std::env::var_os("DPDF_SMOKE_PHYSICAL").is_some(),
    )));
    {
        let state = app.borrow();
        ui.set_dark(state.preferences.dark_theme);
        ui.set_sidebar_visible(state.preferences.sidebar_visible);
        ui.set_view_mode(state.preferences.view_mode as i32);
        ui.set_printer_name(state.preferences.printer_name.as_str().into());
        ui.set_print_scaling(if state.preferences.print_fit { 0 } else { 1 });
        state.printer_choices(&ui);
    }
    ui.set_pages(ModelRc::from(app.borrow().pages.clone()));
    ui.set_mark_tiles(ModelRc::from(app.borrow().mark_tiles.clone()));
    ui.set_sidebar_items(ModelRc::from(app.borrow().sidebar.clone()));
    macro_rules! callback {
        ($name:ident, |$state:ident, $window:ident $(, $arg:ident)*| $body:block) => {{
            let state = app.clone(); let weak = ui.as_weak();
            ui.$name(move |$($arg),*| { if let Some($window) = weak.upgrade() { #[allow(unused_mut)] let mut $state = state.borrow_mut(); $body } });
        }};
    }
    callback!(on_share_file, |state, window, action| {
        state.share_file(&window, action);
    });
    callback!(on_show_about, |state, window| {
        if state.about.is_none() {
            match crate::about::Session::new() {
                Ok(about) => state.about = Some(about),
                Err(error) => {
                    window.set_status(format!("Cannot open About: {error:#}").into());
                    return;
                }
            }
        }
        if let Some(about) = &state.about
            && let Err(error) = about.show(window.get_dark())
        {
            window.set_status(format!("Cannot show About: {error:#}").into());
        }
    });
    callback!(on_open_mark_studio, |state, window| {
        if state.mark_studio.is_none() {
            match crate::mark_studio::StudioSession::new(window.get_dark()) {
                Ok(session) => state.mark_studio = Some(session),
                Err(error) => {
                    window.set_status(format!("Cannot open mark studio: {error:#}").into());
                    return;
                }
            }
        }
        if let Some(session) = &state.mark_studio {
            session.set_document(state.tabs.get(state.active).map(|t| t.info.id));
            if let Err(error) = session.show(window.get_dark()) {
                window.set_status(format!("Cannot show mark studio: {error:#}").into());
            }
        }
    });
    callback!(on_open, |state, window| {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("PDF document", &["pdf"])
            .pick_files()
        {
            for path in paths {
                state.open(&window, path);
            }
        }
    });
    callback!(on_retry_review, |state, window| {
        state.retry_review(&window);
    });
    callback!(on_choose_revision, |state, window, index| {
        state.choose_revision(&window, index.max(0) as usize);
    });
    callback!(on_update_remark, |state, window| {
        state.update_remark(&window);
    });
    callback!(on_new_revision, |state, window| {
        state.new_revision(&window);
    });
    callback!(on_filter_reviews, |state, window, index| {
        state.filter_reviews(&window, index);
    });
    callback!(on_select_remark, |state, window, index, remove| {
        state.select_remark(&window, index.max(0) as usize, remove);
    });
    callback!(on_export_review, |state, window| {
        state.export_review(&window);
    });
    callback!(on_submit_password, |state, window| {
        state.password_action(&window, false);
    });
    callback!(on_cancel_password, |state, window| {
        state.password_action(&window, true);
    });
    callback!(on_recover_draft, |state, window| {
        state.recover_draft(&window, false);
    });
    callback!(on_discard_recovery, |state, window| {
        state.recover_draft(&window, true);
    });
    callback!(on_set_recovery_enabled, |state, window, enabled| {
        state.set_recovery_enabled(&window, enabled);
    });
    callback!(on_clear_recovery, |state, window| {
        state.clear_recovery(&window);
    });
    callback!(on_show_privacy, |state, window| {
        state.privacy_controls(&window);
    });
    callback!(on_undo_remark, |state, window| {
        state.step_review(&window, true);
    });
    callback!(on_redo_remark, |state, window| {
        state.step_review(&window, false);
    });
    callback!(on_save_copy, |state, window| {
        if state.pending_mark.is_some() {
            window.set_status("Click to place the pending mark or press Esc".into());
            return;
        }
        if state.tabs.get(state.active).is_some_and(Tab::unsaved) {
            state.export_marks(&window);
            return;
        }
        if let Some(tab) = state.tabs.get(state.active) {
            let filename = tab
                .info
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            let id = tab.info.id;
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PDF document", &["pdf"])
                .set_file_name(filename.as_ref())
                .save_file()
            {
                state.command(&window, Command::SaveCopy(id, path));
            }
        }
    });
    callback!(on_select_tab, |state, window, index| {
        state.activate(&window, index.max(0) as usize);
    });
    callback!(on_close_tab, |state, window, index| {
        state.close(&window, index.max(0) as usize);
    });
    callback!(on_navigate, |state, window, delta| {
        state.navigate(&window, delta);
    });
    callback!(on_jump, |state, window, value| {
        state.jump(&window, &value);
    });
    callback!(on_zoom, |state, window, factor| {
        state.zoom_center(&window, factor);
    });
    callback!(on_set_zoom, |state, window, text| {
        let input = text.trim();
        let percent = input
            .strip_suffix('%')
            .unwrap_or(input)
            .trim()
            .parse::<u16>();
        match percent {
            Ok(percent) if (10..=600).contains(&percent) => {
                if let Some(layout) = &state.layout {
                    let factor = f32::from(percent) / 100.0 / layout.scale;
                    state.zoom_center(&window, factor);
                }
            }
            _ => window.set_status("Enter a whole zoom percentage from 10% to 600%".into()),
        }
        window.set_zoom_entry(window.get_zoom_label());
    });
    callback!(on_wheel_zoom, |state, window, factor, x, y| {
        state.zoom_around(&window, factor, x, y);
    });
    callback!(on_fit, |state, window, mode| {
        let active = state.active;
        if let Some(tab) = state.tabs.get_mut(active) {
            tab.view.zoom = match mode {
                1 => Zoom::FitWidth,
                2 => Zoom::FitPage,
                _ => Zoom::Scale(1.0),
            };
            state.relayout(&window, true);
        }
    });
    callback!(on_rotate, |state, window| {
        let active = state.active;
        if let Some(tab) = state.tabs.get_mut(active) {
            tab.view.rotation = (tab.view.rotation + 1) % 4;
            state.relayout(&window, true);
        }
    });
    callback!(on_change_view, |state, window, mode| {
        let active = state.active;
        if let Some(tab) = state.tabs.get_mut(active) {
            tab.view.mode = match mode {
                1 => ViewMode::Continuous,
                2 => ViewMode::Facing,
                _ => ViewMode::Single,
            };
            state.relayout(&window, true);
        }
    });
    callback!(on_search, |state, window, query| {
        if state
            .tabs
            .get(state.active)
            .is_some_and(|tab| tab.query == query.as_str() && !tab.hits.is_empty())
        {
            state.next_result(&window, 1);
        } else {
            state.search(&window, query.to_string());
        }
    });
    callback!(on_next_result, |state, window, direction| {
        state.next_result(&window, direction);
    });
    callback!(on_select_sidebar, |state, window, _index| {
        state.rebuild_sidebar(&window);
    });
    callback!(on_discover_printers, |state, window| {
        state.discover_printers(&window);
    });
    callback!(on_choose_printer, |state, window, index| {
        state.choose_printer(&window, index);
    });
    callback!(on_export_pages, |state, window| {
        state.export_pages(&window);
    });
    callback!(on_print_document, |state, window| {
        state.print(&window);
    });
    callback!(on_request_print, |state, window| {
        state.request_native_print(&window, false);
    });
    callback!(on_native_print_document, |state, window| {
        state.request_native_print(&window, true);
    });
    callback!(on_select_text, |state,
                               window,
                               page,
                               x0,
                               y0,
                               x1,
                               y1,
                               complete| {
        state.select_text(&window, page.max(0) as usize, (x0, y0), (x1, y1), complete);
    });
    callback!(on_select_page_text, |state, window| {
        if let Some(page) = state.tabs.get(state.active).map(|t| t.view.page) {
            state.select_text(&window, page, (0.0, 0.0), (1.0, 1.0), true);
        }
    });
    callback!(on_copy_text, |state, window| {
        state.copy_text(&window);
    });
    callback!(on_clear_selection, |state, window| {
        state.pending_mark = None;
        window.set_mark_pending(false);
        state.cancel_mark_drag(&window);
        state.clear_selection(&window);
    });
    callback!(on_cancel_mark, |state, window| {
        state.pending_mark = None;
        window.set_mark_pending(false);
        state.mark_controls(&window);
    });
    callback!(on_undo_mark, |state, window| {
        state.history_mark(&window, false);
    });
    callback!(on_redo_mark, |state, window| {
        state.history_mark(&window, true);
    });
    callback!(on_cancel_mark_drag, |state, window| {
        state.cancel_mark_drag(&window);
    });
    callback!(on_begin_mark, |state, window, index, x, y, resize| {
        state.begin_mark(&window, index.max(0) as usize, (x, y), resize);
    });
    callback!(on_drag_mark, |state, window, x, y, finish| {
        state.drag_mark(&window, (x, y), finish);
    });
    callback!(on_apply_mark_size, |state, window, reset| {
        state.size_mark(&window, reset);
    });
    callback!(on_remove_mark, |state, window| {
        let active = state.active;
        if let Some(tab) = state.tabs.get_mut(active) {
            tab.marks.remove_selected();
        }
        state.mark_controls(&window);
        state.refresh_marks(&window);
        state.tabs_model(&window);
    });
    ui.on_can_drop_page(|data| {
        data.user_data()
            .is_some_and(|payload| payload.is::<PageDrag>())
    });
    callback!(on_drop_page, |state, window, data, row, before| {
        state.drop_page(&window, data, row.max(0) as usize, before);
    });
    callback!(on_drop_page_at, |state, window, data, y| {
        state.drop_page_at(&window, data, y);
    });
    callback!(on_apply_page_order, |state, window| {
        if window.get_arranging() {
            state.apply_page_order(&window);
        }
    });
    callback!(on_shortcut, |state, window, action| {
        if !state.tabs.is_empty() {
            if action == "next-tab" {
                let next = (state.active + 1) % state.tabs.len();
                state.activate(&window, next);
            }
            if action == "previous-tab" {
                let previous = state.active.checked_sub(1).unwrap_or(state.tabs.len() - 1);
                state.activate(&window, previous);
            }
            if action == "close-tab" {
                let active = state.active;
                state.close(&window, active);
            }
        }
    });
    callback!(on_viewport_scrolled, |state, window| {
        state.remember_resize_anchor(&window);
    });
    ui.on_can_drop_files(|data| data.has_file_paths());
    callback!(on_drop_files, |state, window, data| {
        match data.file_paths() {
            Ok(paths) => {
                for path in paths {
                    state.open(&window, path.to_path_buf());
                }
            }
            Err(error) => window.set_status(format!("Cannot read dropped files: {error}").into()),
        }
    });
    // Slint 1.18's winit backend does not translate native DroppedFile events
    // into DropArea data. Consume the OS event through its pinned public hook.
    // Defer opening to the normal timer to avoid re-entering Application borrows.
    let dropped_paths = Rc::new(RefCell::new(Vec::<PathBuf>::new()));
    let native_drops = dropped_paths.clone();
    {
        use slint::winit_030::{EventResult, WinitWindowAccessor, winit};
        ui.window().on_winit_window_event(move |_, event| {
            if let winit::event::WindowEvent::DroppedFile(path) = event {
                native_drops.borrow_mut().push(path.clone());
                EventResult::PreventDefault
            } else {
                EventResult::Propagate
            }
        });
    }
    let close_state = app.clone();
    ui.window().on_close_requested(move || {
        if close_state.borrow().tabs.iter().any(Tab::unsaved) && !confirm_discard() {
            return slint::CloseRequestResponse::KeepWindowShown;
        }
        if let Some(studio) = &close_state.borrow().mark_studio {
            let _ = studio.hide();
        }
        if let Some(about) = &close_state.borrow().about {
            let _ = about.hide();
        }
        let mut state = close_state.borrow_mut();
        for index in 0..state.tabs.len() {
            state.remove_recovery(index);
        }
        slint::CloseRequestResponse::HideWindow
    });
    for path in paths {
        app.borrow_mut().open(&ui, path);
    }
    let smoke_state = Rc::new(RefCell::new(ui_smoke::SmokeCheck::default()));
    let smoke_result = smoke_state.clone();
    let application_result = app.clone();
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(24),
        move || {
            if let Some(ui) = weak.upgrade() {
                let mut state = app.borrow_mut();
                for path in std::mem::take(&mut *dropped_paths.borrow_mut()) {
                    state.open(&ui, path);
                }
                state.tick(&ui);
                let (input, print_dialog, selection, key, shift) = if smoke {
                    let mut check = smoke_state.borrow_mut();
                    check.advance(&mut state, &ui);
                    (
                        check.pending_input.take(),
                        std::mem::take(&mut check.pending_print_dialog),
                        check.pending_selection.take(),
                        check.pending_key.take(),
                        std::mem::take(&mut check.pending_shift),
                    )
                } else {
                    (None, false, None, None, false)
                };
                let pointer = if smoke {
                    std::mem::take(&mut smoke_state.borrow_mut().pending_pointer)
                } else {
                    vec![]
                };
                drop(state);
                for event in pointer {
                    ui.window().dispatch_event(event);
                }
                if let Some((start, end)) = selection {
                    use slint::platform::{PointerEventButton, WindowEvent};
                    ui.window()
                        .dispatch_event(WindowEvent::PointerMoved { position: start });
                    ui.window().dispatch_event(WindowEvent::PointerPressed {
                        position: start,
                        button: PointerEventButton::Left,
                    });
                    ui.window().dispatch_event(WindowEvent::PointerMoved {
                        position: slint::LogicalPosition::new(
                            (start.x + end.x) / 2.0,
                            (start.y + end.y) / 2.0,
                        ),
                    });
                    ui.window()
                        .dispatch_event(WindowEvent::PointerMoved { position: end });
                    ui.window().dispatch_event(WindowEvent::PointerReleased {
                        position: end,
                        button: PointerEventButton::Left,
                    });
                }
                if let Some((control, text)) = key {
                    use slint::platform::{Key, WindowEvent};
                    if shift {
                        ui.window().dispatch_event(WindowEvent::KeyPressed {
                            text: Key::Shift.into(),
                        });
                    }
                    if control {
                        ui.window().dispatch_event(WindowEvent::KeyPressed {
                            text: Key::Control.into(),
                        });
                    }
                    ui.window()
                        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
                    ui.window()
                        .dispatch_event(WindowEvent::KeyReleased { text });
                    if shift {
                        ui.window().dispatch_event(WindowEvent::KeyReleased {
                            text: Key::Shift.into(),
                        });
                    }
                    if control {
                        ui.window().dispatch_event(WindowEvent::KeyReleased {
                            text: Key::Control.into(),
                        });
                    }
                }
                if print_dialog {
                    use slint::platform::{Key, WindowEvent};
                    ui.window().dispatch_event(WindowEvent::KeyPressed {
                        text: Key::Control.into(),
                    });
                    ui.window()
                        .dispatch_event(WindowEvent::KeyPressed { text: "p".into() });
                    ui.window()
                        .dispatch_event(WindowEvent::KeyReleased { text: "p".into() });
                    ui.window().dispatch_event(WindowEvent::KeyReleased {
                        text: Key::Control.into(),
                    });
                }
                if let Some((control, delta_y)) = input {
                    use slint::platform::{Key, WindowEvent};
                    let position = slint::LogicalPosition::new(
                        ui.get_viewport_window_x() + ui.get_viewport_width() / 2.0,
                        ui.get_viewport_window_y() + ui.get_viewport_height() / 2.0,
                    );
                    ui.window()
                        .dispatch_event(WindowEvent::PointerMoved { position });
                    if control {
                        ui.window().dispatch_event(WindowEvent::KeyPressed {
                            text: Key::Control.into(),
                        });
                    }
                    ui.window().dispatch_event(WindowEvent::PointerScrolled {
                        position,
                        delta_x: 0.0,
                        delta_y,
                    });
                    if control {
                        ui.window().dispatch_event(WindowEvent::KeyReleased {
                            text: Key::Control.into(),
                        });
                    }
                }
            }
        },
    );
    ui.run()?;
    if let Some(worker) = application_result.borrow_mut().recovery.as_mut() {
        worker.finish();
    }
    application_result
        .borrow_mut()
        .remember_preferences(&ui, true);
    if let Some(failure) = smoke_result.borrow_mut().failure.take() {
        return Err(slint::PlatformError::Other(failure));
    }
    Ok(())
}

fn save_snapshot(ui: &Workspace, path: &std::path::Path) -> anyhow::Result<()> {
    crate::snapshot::write(ui.window(), path)
}
