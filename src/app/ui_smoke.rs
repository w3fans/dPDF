//! Explicit real-window development diagnostics, separate from the reader controller.
use super::*;

// Explicit diagnostic mode: exercises a real desktop window and exits. It never prints.
#[derive(Default)]
pub(super) struct SmokeCheck {
    viewport: viewport_smoke::Check,
    physical: physical_smoke::Check,
    stage: u8,
    ticks: u32,
    pub(super) failure: Option<String>,
    pub(super) pending_input: Option<(bool, f32)>,
    scale_before_input: f32,
    scroll_before_input: f32,
    original_scroll: (f32, f32),
    second_document: Option<tempfile::TempDir>,
    pub(super) pending_print_dialog: bool,
    mark_geometry: Option<crate::overlay::Geometry>,
    pending_mark_fixture: Option<crate::mark_studio::StudioMark>,
    assembly_pages: usize,
    pub(super) pending_selection: Option<(slint::LogicalPosition, slint::LogicalPosition)>,
    pub(super) pending_key: Option<(bool, slint::SharedString)>,
    pub(super) pending_shift: bool,
    password_attempts: u8,
    password_queue_started: bool,
    review_source: Option<usize>,
    review_marks: usize,
    history_review: Option<crate::review::Review>,
    history_generation: u64,
    usage_fixture: Option<crate::marks::AssetUse>,
    pub(super) pending_pointer: Vec<slint::platform::WindowEvent>,
    tab_reopen: Option<PathBuf>,
    tab_identity: Option<DocumentId>,
    tab_previous_width: f32,
    tab_original_size: Option<slint::PhysicalSize>,
    order_expected: Vec<usize>,
    order_scroll_before: f32,
    order_ticks: u8,
    entry_target: usize,
    bookmark_target: Option<usize>,
    save_folder: Option<tempfile::TempDir>,
    save_source: Vec<u8>,
}

impl SmokeCheck {
    fn helper_advance(&mut self, app: &mut Application, ui: &Workspace, mode: &str) {
        let result = (|| -> anyhow::Result<()> {
            if !matches!(
                mode,
                "close"
                    | "retry"
                    | "shutdown"
                    | "recovery"
                    | "protected-print"
                    | "protected-export"
                    | "overlay-close"
                    | "overlay-save"
                    | "overlay-units"
                    | "overlay-review"
            ) {
                anyhow::bail!("Unknown helper diagnostic mode");
            }
            if mode.starts_with("protected-") && ui.get_password_required() {
                ui.set_password_text(std::env::var("DPDF_SMOKE_PASSWORD_VALUE")?.into());
                self.pending_key = Some((false, slint::platform::Key::Return.into()));
                return Ok(());
            }
            let marker = PathBuf::from(
                std::env::var_os("DPDF_SMOKE_HELPER_READY")
                    .ok_or_else(|| anyhow::anyhow!("Missing helper readiness path"))?,
            );
            match self.stage {
                0 if mode.starts_with("overlay-")
                    && ui.get_review_ready()
                    && app.pages.iter().any(|page| page.ready) =>
                {
                    let arm = PathBuf::from(
                        std::env::var_os("DPDF_SMOKE_HELPER_ARM")
                            .ok_or_else(|| anyhow::anyhow!("Missing overlay helper arm path"))?,
                    );
                    let destination = PathBuf::from(
                        std::env::var_os("DPDF_SMOKE_HELPER_DESTINATION")
                            .ok_or_else(|| anyhow::anyhow!("Missing overlay destination"))?,
                    );
                    let image = PathBuf::from(
                        std::env::var_os("DPDF_SMOKE_PLACEMENT_IMAGE")
                            .ok_or_else(|| anyhow::anyhow!("Missing generated overlay image"))?,
                    );
                    let tab = &app.tabs[app.active];
                    let placements = vec![crate::overlay::Placement {
                        page: 0,
                        geometry: crate::overlay::Geometry {
                            x_mm: 20.0,
                            y_mm: 20.0,
                            width_mm: 35.0,
                            height_mm: 25.0,
                            quarter_turns: 0,
                        },
                        opacity: 0.7,
                        image: Arc::new(std::fs::read(image)?),
                    }];
                    let command = if mode == "overlay-save" || mode == "overlay-review" {
                        Command::ExportWorkspace {
                            id: tab.info.id,
                            placements,
                            review: tab
                                .review
                                .as_ref()
                                .ok_or_else(|| anyhow::anyhow!("Missing review model"))?
                                .model
                                .clone(),
                            mode: crate::review::ExportMode::Current,
                            destination,
                        }
                    } else {
                        Command::ExportMarks {
                            id: tab.info.id,
                            placements,
                            destination,
                        }
                    };
                    std::fs::write(arm, b"armed")?;
                    if !app.command(ui, command) {
                        anyhow::bail!("Overlay export was not admitted");
                    }
                    app.export_pending = true;
                    ui.set_export_busy(true);
                    self.stage = 8;
                }
                8 if marker.exists() => {
                    app.go_to(ui, 24);
                    self.stage = 9;
                }
                9 if app.pages.iter().any(|page| page.page == 24 && page.ready) => {
                    if !app.export_pending || !ui.get_export_busy() {
                        anyhow::bail!("Overlay export completed before the held helper");
                    }
                    println!("helper: rendered page 25 while export helper remained stalled");
                    if mode == "overlay-close" || mode == "overlay-units" {
                        app.close(ui, app.active);
                        self.stage = 10;
                    } else {
                        println!("helper: owned overlay save stayed pending until normal shutdown");
                        self.stage = 99;
                        let _ = slint::quit_event_loop();
                    }
                }
                10 if app.tabs.is_empty() && !app.export_pending && !ui.get_export_busy() => {
                    println!(
                        "helper: tab close cancelled overlay export without publishing a late result"
                    );
                    self.stage = 99;
                    let _ = slint::quit_event_loop();
                }
                0 if mode == "recovery"
                    && ui.get_review_ready()
                    && !app.recovery_candidates.is_empty()
                    && app.pages.iter().any(|p| p.ready) =>
                {
                    let arm = PathBuf::from(
                        std::env::var_os("DPDF_SMOKE_HELPER_ARM")
                            .ok_or_else(|| anyhow::anyhow!("Missing recovery helper arm path"))?,
                    );
                    std::fs::write(arm, b"armed")?;
                    self.tab_identity = Some(app.tabs[app.active].info.id);
                    app.recover_draft(ui, false);
                    self.stage = 5;
                }
                5 if marker.exists() && app.recovering.is_some() => {
                    app.go_to(ui, 24);
                    self.stage = 6;
                }
                6 if app.pages.iter().any(|p| p.page == 24 && p.ready) => {
                    if app.recovering.is_none()
                        || Some(app.tabs[app.active].info.id) != self.tab_identity
                        || !ui.get_recovery_busy()
                    {
                        anyhow::bail!("Recovery published a document before metadata validation");
                    }
                    println!("helper: rendered page 25 while metadata remained stalled");
                    println!("helper: recovery metadata stayed pending until normal shutdown");
                    use std::io::Write;
                    std::io::stdout().flush()?;
                    self.stage = 99;
                    let _ = slint::quit_event_loop();
                }
                0 if marker.exists()
                    && app.tabs.len() == 1
                    && app.pages.iter().any(|p| p.ready) =>
                {
                    if ui.get_review_ready() {
                        anyhow::bail!("Metadata fixture was not stalled");
                    }
                    self.tab_identity = Some(app.tabs[app.active].info.id);
                    self.tab_reopen = Some(app.tabs[app.active].info.path.clone());
                    if mode == "protected-print" {
                        // Ordinary smoke disables native dialog admission. This
                        // generated case must queue it while decryption is held;
                        // the fixture permits only GTK's file backend and quits
                        // before preparation can open any print dialog.
                        let native = ui.get_native_printing();
                        ui.set_native_printing(true);
                        app.request_native_print(ui, false);
                        ui.set_native_printing(native);
                        if !app.print_pending {
                            anyhow::bail!("Protected print was not admitted");
                        }
                    } else if mode == "protected-export" {
                        let destination = PathBuf::from(
                            std::env::var_os("DPDF_SMOKE_HELPER_DESTINATION").ok_or_else(|| {
                                anyhow::anyhow!("Missing protected export destination")
                            })?,
                        );
                        if !app.command(
                            ui,
                            Command::ExportPages {
                                id: app.tabs[app.active].info.id,
                                operation: crate::assembly::PageOperation {
                                    kind: crate::assembly::OperationKind::Extract,
                                    pages: "1".into(),
                                    source_pages: String::new(),
                                },
                                additional: vec![],
                                destination,
                            },
                        ) {
                            anyhow::bail!("Protected export was not admitted");
                        }
                        app.export_pending = true;
                        ui.set_export_busy(true);
                    }
                    app.go_to(ui, 24);
                    self.stage = 1;
                }
                1 if app.pages.iter().any(|p| p.page == 24 && p.ready) => {
                    if ui.get_review_ready() {
                        anyhow::bail!("Metadata completed before navigation could be checked");
                    }
                    println!("helper: rendered page 25 while metadata remained stalled");
                    match mode {
                        "protected-print" => {
                            if !app.print_pending {
                                anyhow::bail!("Protected print completed before decryption");
                            }
                            println!(
                                "helper: protected print preparation stayed pending until normal shutdown"
                            );
                            self.stage = 99;
                            let _ = slint::quit_event_loop();
                        }
                        "protected-export" => {
                            if !app.export_pending {
                                anyhow::bail!("Protected export completed before decryption");
                            }
                            app.close(ui, app.active);
                            self.stage = 7;
                        }
                        "close" => {
                            app.close(ui, app.active);
                            self.stage = 2;
                        }
                        "retry" => {
                            app.retry_review(ui);
                            self.stage = 4;
                        }
                        _ => {
                            println!("helper: quitting with a live metadata helper");
                            use std::io::Write;
                            std::io::stdout().flush()?;
                            self.stage = 99;
                            let _ = slint::quit_event_loop();
                        }
                    }
                }
                7 if app.tabs.is_empty() && !app.export_pending && !ui.get_export_busy() => {
                    println!(
                        "helper: tab close cancelled protected export without publishing a late result"
                    );
                    self.stage = 99;
                    let _ = slint::quit_event_loop();
                }
                2 if app.tabs.is_empty() => {
                    app.open(ui, self.tab_reopen.clone().unwrap());
                    self.stage = 3;
                }
                3 if ui.get_review_ready() && app.pages.iter().any(|p| p.ready) => {
                    if app.tabs[app.active].info.id == self.tab_identity.unwrap() {
                        anyhow::bail!("Reopened document reused its closed identity");
                    }
                    println!(
                        "helper: closed stalled document and reopened without a stale completion"
                    );
                    let _ = slint::quit_event_loop();
                    self.stage = 99;
                }
                4 if ui.get_review_ready() => {
                    if app.tabs[app.active].info.id != self.tab_identity.unwrap()
                        || app.tabs[app.active].review_error.is_some()
                    {
                        anyhow::bail!("Retry accepted a stale metadata result");
                    }
                    println!(
                        "helper: retry replaced stalled metadata without accepting its cancelled completion"
                    );
                    let _ = slint::quit_event_loop();
                    self.stage = 99;
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.failure = Some(format!("{error:#}"));
            let _ = slint::quit_event_loop();
        }
    }
    fn tabs_advance(&mut self, app: &mut Application, ui: &Workspace) {
        let result = (|| -> anyhow::Result<()> {
            let ready = app.pages.iter().any(|p| p.ready);
            let visible = |app: &Application| -> anyhow::Result<()> {
                let _ = ui.window().take_snapshot()?;
                let left = app.active as f32 * ui.get_tab_width() + ui.get_tab_strip_x();
                let right = left + ui.get_tab_width();
                if left < -0.5
                    || right > ui.get_tab_strip_width() + 0.5
                    || ui.get_tab_strip_height() < 32.0
                {
                    anyhow::bail!(
                        "Selected tab/close control is clipped: active={}, left={left}, right={right}, viewport={}x{}",
                        app.active,
                        ui.get_tab_strip_width(),
                        ui.get_tab_strip_height()
                    );
                }
                Ok(())
            };
            match self.stage {
                0 if ready && app.tabs.len() == 1 => {
                    let path = app.tabs[0].info.path.clone();
                    self.tab_reopen = Some(path.clone());
                    let directory = tempfile::tempdir()?;
                    for index in 1..12 {
                        let copy = directory
                            .path()
                            .join(format!("Tab {} — fixture.pdf", index + 1));
                        std::fs::copy(&path, &copy)?;
                        app.open(ui, copy);
                    }
                    self.second_document = Some(directory);
                    self.stage = 1;
                }
                1 if app.tabs.len() == 12 && app.opening.is_empty() && ready => {
                    // Settle the strip's new scrollbar geometry before checking visibility.
                    let _ = ui.window().take_snapshot()?;
                    self.stage = 2;
                }
                2 => {
                    if app.active != 11 {
                        anyhow::bail!("Newly opened document was not selected");
                    }
                    visible(app)?;
                    if let Some(path) = std::env::var_os("DPDF_SMOKE_TAB_SNAPSHOT") {
                        save_snapshot(ui, &PathBuf::from(path))?;
                    }
                    println!(
                        "tabs: opening twelve PDFs reveals the active tab and complete close control"
                    );
                    ui.set_tab_strip_x(0.0);
                    app.tabs_model(ui);
                    self.order_ticks = 0;
                    self.stage = 3;
                }
                3 => {
                    self.order_ticks += 1;
                    if self.order_ticks < 3 {
                        return Ok(());
                    }
                    if ui.get_tab_strip_x().abs() > 0.5 {
                        anyhow::bail!("A label refresh overrode manual tab scrolling");
                    }
                    self.pending_key = Some((true, slint::platform::Key::Tab.into()));
                    self.stage = 4;
                }
                4 if app.active == 0 => {
                    visible(app)?;
                    self.pending_shift = true;
                    self.pending_key = Some((true, slint::platform::Key::Tab.into()));
                    self.stage = 5;
                }
                5 if app.active == 11 => {
                    visible(app)?;
                    println!(
                        "tabs: dispatched Ctrl+Tab and Ctrl+Shift+Tab wrap and reveal their selected document"
                    );
                    self.stage = 20;
                }
                20 if ready => {
                    let _ = ui.window().take_snapshot()?;
                    let p = slint::LogicalPosition::new(
                        ui.get_page_field_window_x() + 15.0,
                        ui.get_page_field_window_y() + 12.0,
                    );
                    self.pending_selection = Some((p, p));
                    self.pending_key = Some((true, "a".into()));
                    self.stage = 21;
                }
                21 => {
                    if !ui.get_jump_editing() {
                        anyhow::bail!("Tab focus fixture did not focus the page field");
                    }
                    self.pending_key = Some((false, "9".into()));
                    self.stage = 22;
                }
                22 => {
                    if ui.get_jump_text().as_str() != "9" || app.active != 11 {
                        anyhow::bail!("Tab focus fixture did not create an uncommitted page value");
                    }
                    self.pending_key = Some((true, slint::platform::Key::Tab.into()));
                    self.stage = 23;
                }
                23 if app.active == 0 && ready => {
                    if ui.get_jump_editing()
                        || ui.get_text_field_focused()
                        || ui.get_jump_text().as_str() != (app.tabs[0].view.page + 1).to_string()
                        || app.tabs[11].view.page != 0
                    {
                        anyhow::bail!("Page field draft crossed a document switch");
                    }
                    let _ = ui.window().take_snapshot()?;
                    let p = slint::LogicalPosition::new(
                        ui.get_zoom_field_window_x() + 15.0,
                        ui.get_zoom_field_window_y() + 12.0,
                    );
                    self.scale_before_input = app.layout.as_ref().unwrap().scale;
                    self.pending_selection = Some((p, p));
                    self.pending_key = Some((true, "a".into()));
                    self.stage = 24;
                }
                24 => {
                    if !ui.get_zoom_editing() {
                        anyhow::bail!("Tab focus fixture did not focus the zoom field");
                    }
                    self.pending_key = Some((false, "250%".into()));
                    self.stage = 25;
                }
                25 => {
                    if ui.get_zoom_entry().as_str() != "250%" {
                        anyhow::bail!("Tab focus fixture did not create an uncommitted zoom value");
                    }
                    self.pending_shift = true;
                    self.pending_key = Some((true, slint::platform::Key::Tab.into()));
                    self.stage = 26;
                }
                26 if app.active == 11 && ready => {
                    if ui.get_zoom_editing()
                        || ui.get_text_field_focused()
                        || ui.get_zoom_entry() != ui.get_zoom_label()
                        || (app.layout.as_ref().unwrap().scale - self.scale_before_input).abs()
                            > 0.001
                    {
                        anyhow::bail!("Zoom field draft crossed a document switch");
                    }
                    println!(
                        "tabs: switching from page and zoom text cancels drafts and restores reader focus"
                    );
                    self.pending_key = Some((true, "w".into()));
                    self.stage = 6;
                }
                6 if app.tabs.len() == 11 && app.active == 10 => {
                    visible(app)?;
                    self.tab_identity = Some(app.tabs[app.active].info.id);
                    self.tab_previous_width = ui.get_tab_strip_width();
                    let size = ui.window().size();
                    self.tab_original_size = Some(size);
                    ui.window()
                        .set_size(slint::PhysicalSize::new(size.width + 240, size.height));
                    self.stage = 7;
                }
                7 if ui.get_tab_strip_width() > self.tab_previous_width + 10.0 => {
                    visible(app)?;
                    self.tab_previous_width = ui.get_tab_strip_width();
                    let size = self.tab_original_size.unwrap();
                    ui.window().set_size(slint::PhysicalSize::new(
                        size.width.saturating_sub(120).max(640),
                        size.height,
                    ));
                    self.stage = 8;
                }
                8 if ui.get_tab_strip_width() < self.tab_previous_width - 10.0 => {
                    visible(app)?;
                    while app.tabs.len() > 3 {
                        app.close(ui, 0);
                    }
                    self.stage = 9;
                }
                9 if app.tabs.len() == 3 => {
                    if Some(app.tabs[app.active].info.id) != self.tab_identity {
                        anyhow::bail!("Closing background tabs changed the selected document");
                    }
                    visible(app)?;
                    if ui.get_tab_strip_x().abs() > 0.5 {
                        anyhow::bail!("A short tab list retained an obsolete scroll offset");
                    }
                    while !app.tabs.is_empty() {
                        app.close(ui, 0);
                    }
                    self.stage = 10;
                }
                10 if app.tabs.is_empty() && !ui.get_has_document() => {
                    if ui.get_tab_strip_x().abs() > 0.5 {
                        anyhow::bail!("Closing every tab retained a scroll offset");
                    }
                    app.open(ui, self.tab_reopen.as_ref().unwrap().clone());
                    self.stage = 11;
                }
                11 if app.tabs.len() == 1 && ready => {
                    visible(app)?;
                    if app.active != 0 || ui.get_tab_strip_x().abs() > 0.5 {
                        anyhow::bail!("Reopening a document inherited a stale tab position");
                    }
                    println!(
                        "tabs: resizing, active/background closure and empty-to-reopen preserve tab visibility and identity"
                    );
                    let _ = slint::quit_event_loop();
                    self.stage = 12;
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.failure = Some(format!("{error:#}"));
            let _ = slint::quit_event_loop();
        }
    }
    fn review_advance(&mut self, app: &mut Application, ui: &Workspace, mode: &str) {
        let result = (|| -> anyhow::Result<()> {
            let ready = app.pages.iter().any(|p| p.ready)
                && app
                    .tabs
                    .get(app.active)
                    .is_some_and(|t| t.review_ready && t.review.is_some());
            let point = |app: &Application, x: f32, y: f32| {
                let page = &app.layout.as_ref().unwrap().pages[0];
                slint::LogicalPosition::new(
                    ui.get_viewport_window_x() + ui.get_scroll_x() + page.x + x * page.width,
                    ui.get_viewport_window_y() + ui.get_scroll_y() + page.y + y * page.height,
                )
            };
            if mode == "unavailable" {
                if self.stage == 0 && ui.get_review_failed() {
                    if ui.get_review_failure().is_empty()
                        || ui.get_review_ready()
                        || app.pages.iter().all(|p| !p.ready)
                    {
                        anyhow::bail!("Unavailable review hid the reader or its error");
                    }
                    app.retry_review(ui);
                    self.stage = 1;
                } else if self.stage == 1 && ui.get_review_failed() {
                    if ui.get_review_failure().is_empty() || ui.get_review_ready() {
                        anyhow::bail!("Review retry did not report its persistent failure");
                    }
                    println!(
                        "review: unavailable helper and retry preserve reading with a visible error"
                    );
                    let _ = slint::quit_event_loop();
                    self.stage = 6;
                }
                return Ok(());
            }
            match self.stage {
                0 if mode.starts_with("read") && ui.get_has_recovery() => {
                    self.review_marks = if mode == "read-notes" { 0 } else { 1 };
                    app.recover_draft(ui, false);
                    self.stage = 20;
                }
                20 if ready
                    && app.tabs[app.active].marks.items.len() == self.review_marks
                    && app.tabs[app.active]
                        .review
                        .as_ref()
                        .is_some_and(|r| r.model.remarks.len() == 3) =>
                {
                    let tab = &app.tabs[app.active];
                    let review = &tab.review.as_ref().unwrap().model;
                    if tab.info.pages.len() != 50
                        || review.revisions.len() != 3
                        || !review.remarks[0].text.contains("Updated note")
                    {
                        anyhow::bail!(
                            "Mixed review recovery lost its opened document, revision sets or remark text"
                        );
                    }
                    self.review_source = Some(app.active);
                    self.second_document = Some(tempfile::tempdir()?);
                    ui.set_review_visible(true);
                    app.review_controls(ui);
                    self.stage = 5;
                    println!(
                        "review: restored signatures and revision remarks from an interrupted session"
                    );
                }
                0 if !mode.starts_with("read") && ready => {
                    self.review_marks = if mode == "write-notes" { 0 } else { 1 };
                    self.review_source = Some(app.active);
                    self.second_document = Some(tempfile::tempdir()?);
                    ui.set_review_visible(true);
                    app.tabs[app.active].view.zoom = Zoom::FitPage;
                    app.relayout(ui, true);
                    self.stage = 10;
                }
                10 if ready => {
                    ui.set_review_tool(1);
                    ui.set_review_text("First revision note".into());
                    let _ = ui.window().take_snapshot();
                    let p = point(app, 0.72, 0.70);
                    self.pending_selection = Some((p, p));
                    self.stage = 1;
                }
                1 if app.tabs[app.active]
                    .review
                    .as_ref()
                    .is_some_and(|r| r.model.remarks.len() == 1) =>
                {
                    if ui.get_share_ready() {
                        anyhow::bail!("Unsaved review remarks incorrectly allow sharing");
                    }
                    ui.set_revision_name("Revision 2".into());
                    app.new_revision(ui);
                    ui.set_review_tool(2);
                    ui.set_review_text("Second revision highlight".into());
                    let _ = ui.window().take_snapshot();
                    self.pending_selection = Some((point(app, 0.15, 0.30), point(app, 0.65, 0.35)));
                    self.stage = 2;
                }
                2 if app.tabs[app.active]
                    .review
                    .as_ref()
                    .is_some_and(|r| r.model.remarks.len() == 2) =>
                {
                    ui.set_revision_name("Revision 3".into());
                    app.new_revision(ui);
                    ui.set_review_tool(3);
                    ui.set_review_text("Third revision underline".into());
                    let _ = ui.window().take_snapshot();
                    self.pending_selection = Some((point(app, 0.15, 0.45), point(app, 0.65, 0.50)));
                    self.stage = 3;
                }
                3 if app.tabs[app.active]
                    .review
                    .as_ref()
                    .is_some_and(|r| r.model.remarks.len() == 3) =>
                {
                    // Remark history: undo the underline, then the revision, then redo both.
                    let remarks = |app: &Application| {
                        app.tabs[app.active]
                            .review
                            .as_ref()
                            .map_or(0, |r| r.model.remarks.len())
                    };
                    let revisions = app.tabs[app.active]
                        .review
                        .as_ref()
                        .unwrap()
                        .model
                        .revisions
                        .len();
                    let hidden = !ui.get_remarks_on();
                    app.step_review(ui, true);
                    if remarks(app) != 2 || !ui.get_can_redo_remark() {
                        anyhow::bail!("Undo did not remove the last remark");
                    }
                    app.step_review(ui, true);
                    let after = app.tabs[app.active].review.as_ref().unwrap();
                    if after.model.revisions.len() + 1 != revisions
                        || !after
                            .model
                            .revisions
                            .iter()
                            .any(|r| r.id == after.model.current)
                    {
                        anyhow::bail!("Undo did not remove the newest revision consistently");
                    }
                    app.step_review(ui, false);
                    app.step_review(ui, false);
                    let after = app.tabs[app.active].review.as_ref().unwrap();
                    if remarks(app) != 3
                        || after.model.revisions.len() != revisions
                        || !after.dirty
                        || ui.get_can_redo_remark()
                        || hidden != !ui.get_remarks_on()
                    {
                        anyhow::bail!("Redo did not restore remarks, revisions and dirty state");
                    }
                    println!("review: remark Undo/Redo restored remarks and revisions");
                    if mode == "write-notes" {
                        ui.set_sidebar_tab(2);
                        app.rebuild_sidebar(ui);
                        app.select_remark(ui, 0, false);
                        ui.set_review_text("Updated note čšž".into());
                        app.update_remark(ui);
                        self.stage = 5;
                        return Ok(());
                    }
                    let path =
                        PathBuf::from(std::env::var_os("DPDF_SMOKE_PLACEMENT_IMAGE").ok_or_else(
                            || anyhow::anyhow!("Review diagnostic requires a synthetic mark image"),
                        )?);
                    let (image, _, _) = crate::imaging::decode(&std::fs::read(path)?)?;
                    let rgba = image.thumbnail(400, 400).to_rgba8();
                    let make = |rgba: &image::RgbaImage| {
                        slint::Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                            rgba.as_raw(),
                            rgba.width(),
                            rgba.height(),
                        ))
                    };
                    app.pending_mark = Some(crate::mark_studio::StudioMark {
                        asset_use: None,
                        document: app.tabs[app.active].info.id,
                        bytes: std::sync::Arc::new(crate::marks::png_ref(&rgba)?),
                        previews: [
                            make(&rgba),
                            make(&image::imageops::rotate90(&rgba)),
                            make(&image::imageops::rotate180(&rgba)),
                            make(&image::imageops::rotate270(&rgba)),
                        ],
                        size: crate::marks::PreferredSize {
                            width_mm: 45.0,
                            height_mm: 22.5,
                            aspect_locked: true,
                        },
                    });
                    ui.set_mark_pending(true);
                    let _ = ui.window().take_snapshot();
                    let p = point(app, 0.80, 0.80);
                    self.pending_selection = Some((p, p));
                    self.stage = 4;
                }
                4 if app.tabs[app.active].marks.items.len() == 1 => {
                    ui.set_remarks_on(false);
                    app.filter_reviews(ui, 0);
                    let detail = ui.get_status_detail();
                    if !detail.starts_with("Page ")
                        || !detail.contains("  ·  Remarks off · Revision 3")
                    {
                        anyhow::bail!("Status bar lacks page or hidden-remarks state: {detail}");
                    }
                    if ui.get_review_tiles().row_count() != 0
                        || ui.get_mark_tiles().row_count() != 1
                    {
                        anyhow::bail!("Remarks toggle affected signature placement");
                    }
                    ui.set_remarks_on(true);
                    app.filter_reviews(ui, 1);
                    if ui.get_review_tiles().row_count() != 1 {
                        anyhow::bail!("Current revision filter did not select Revision 3");
                    }
                    // All revision selectors share these controller state and filter updates.
                    // This exercises state synchronization; native widget interaction is separate.
                    app.choose_revision(ui, 0);
                    if ui.get_revision_index() != 0
                        || !ui.get_review_status().contains("Revision 1")
                        || ui.get_review_tiles().row_count() != 1
                        || ui.get_mark_tiles().row_count() != 1
                    {
                        anyhow::bail!(
                            "Revision selection did not synchronize controls/filter independently of marks"
                        );
                    }
                    ui.set_remarks_on(false);
                    app.filter_reviews(ui, 1);
                    app.choose_revision(ui, 1);
                    if ui.get_revision_index() != 1
                        || ui.get_remarks_on()
                        || ui.get_review_tiles().row_count() != 0
                        || ui.get_mark_tiles().row_count() != 1
                    {
                        anyhow::bail!(
                            "Revision selection changed hidden remarks or mark visibility"
                        );
                    }
                    app.choose_revision(ui, 2);
                    ui.set_remarks_on(true);
                    app.filter_reviews(ui, 1);
                    println!(
                        "review: revision selector state follows current filter and preserves hidden remarks and marks"
                    );
                    let before = app.tabs[app.active].review.as_ref().unwrap().model.clone();
                    ui.set_remarks_on(false);
                    ui.set_revision_filter_ids("999".into());
                    app.filter_reviews(ui, 2);
                    if app.tabs[app.active].review.as_ref().unwrap().model != before {
                        anyhow::bail!("Invalid revision filter partly changed visibility");
                    }
                    ui.set_revision_filter_ids("1,3".into());
                    app.filter_reviews(ui, 2);
                    if ui.get_review_tiles().row_count() != 2 {
                        anyhow::bail!("Selected revision filter lost its sets");
                    }
                    app.filter_reviews(ui, 0);
                    ui.set_sidebar_tab(2);
                    app.rebuild_sidebar(ui);
                    app.select_remark(ui, 0, false);
                    ui.set_review_text("Updated note čšž".into());
                    app.update_remark(ui);
                    if !app.tabs[app.active].review.as_ref().unwrap().model.remarks[0]
                        .text
                        .contains("Updated")
                    {
                        anyhow::bail!("Review text edit did not apply");
                    }
                    println!(
                        "review: dispatched note/highlight/underline, three revisions, independent signature visibility and text edit passed"
                    );
                    app.tabs[app.active].view.rotation = 1;
                    app.relayout(ui, true);
                    self.stage = 30;
                }
                30 if ready => {
                    app.step_review(ui, true);
                    if !ui.get_can_redo_remark() {
                        anyhow::bail!("Text focus fixture lacks independent review Redo");
                    }
                    self.history_review =
                        Some(app.tabs[app.active].review.as_ref().unwrap().model.clone());
                    self.history_generation = app.tabs[app.active].draft_generation();
                    ui.set_query("".into());
                    self.pending_key = Some((true, "f".into()));
                    self.stage = 36;
                }
                36 => {
                    if !ui.get_search_visible() || !ui.get_text_field_focused() {
                        anyhow::bail!("Search did not acquire native text focus");
                    }
                    self.pending_key = Some((false, "n".into()));
                    self.stage = 37;
                }
                37..=41 => {
                    let tab = &app.tabs[app.active];
                    if Some(&tab.review.as_ref().unwrap().model) != self.history_review.as_ref()
                        || tab.draft_generation() != self.history_generation
                        || !ui.get_can_redo_remark()
                    {
                        anyhow::bail!(
                            "Focused search shortcut changed independent document history"
                        );
                    }
                    match self.stage {
                        37 => {
                            if ui.get_query().as_str() != "n" {
                                anyhow::bail!("Focused search did not receive text");
                            }
                            self.pending_key = Some((true, "y".into()));
                        }
                        38 => {
                            self.pending_shift = true;
                            self.pending_key = Some((true, "z".into()));
                        }
                        39 => self.pending_key = Some((true, "z".into())),
                        40 => {
                            if !ui.get_query().is_empty() {
                                anyhow::bail!("Native text Undo did not remove search input");
                            }
                            self.pending_shift = !cfg!(target_os = "windows");
                            self.pending_key = Some((
                                true,
                                if cfg!(target_os = "windows") {
                                    "y"
                                } else {
                                    "z"
                                }
                                .into(),
                            ));
                        }
                        41 => {
                            if ui.get_query().as_str() != "n" {
                                anyhow::bail!("Native text Redo did not restore search input");
                            }
                            self.pending_key = Some((false, slint::platform::Key::Escape.into()));
                        }
                        _ => unreachable!(),
                    }
                    self.stage += 1;
                }
                42 => {
                    if ui.get_search_visible() || ui.get_text_field_focused() {
                        anyhow::bail!("Escape did not return input focus to the reader");
                    }
                    app.step_review(ui, false);
                    self.history_review = None;
                    println!(
                        "review: focused text owns Undo/Redo without changing document history"
                    );
                    self.stage = 43;
                }
                43 if ready => {
                    // A recovered review can have empty history while mark edits
                    // remain undoable. Exercise real key dispatch at that boundary.
                    app.tabs[app.active]
                        .review
                        .as_mut()
                        .unwrap()
                        .clear_history();
                    app.review_controls(ui);
                    if ui.get_can_undo_remark() || !ui.get_can_undo_mark() {
                        anyhow::bail!("History isolation fixture lacks independent mark Undo");
                    }
                    self.pending_key = Some((true, "z".into()));
                    self.stage = 31;
                }
                31 => {
                    if app.tabs[app.active].marks.items.len() != 1 || !ui.get_can_undo_mark() {
                        anyhow::bail!("Empty Review Undo fell through to mark history");
                    }
                    app.history_mark(ui, false);
                    if !app.tabs[app.active].marks.items.is_empty() || !ui.get_can_redo_mark() {
                        anyhow::bail!("History isolation fixture lacks independent mark Redo");
                    }
                    self.pending_key = Some((true, "y".into()));
                    self.stage = 32;
                }
                32 => {
                    if !app.tabs[app.active].marks.items.is_empty() || !ui.get_can_redo_mark() {
                        anyhow::bail!("Empty Review Ctrl+Y fell through to mark history");
                    }
                    self.pending_key = Some((true, "Z".into()));
                    self.pending_shift = true;
                    self.stage = 33;
                }
                33 => {
                    if !app.tabs[app.active].marks.items.is_empty() || !ui.get_can_redo_mark() {
                        anyhow::bail!("Empty Review Ctrl+Shift+Z fell through to mark history");
                    }
                    ui.set_review_visible(false);
                    self.pending_key = Some((true, "y".into()));
                    self.stage = 34;
                }
                34 => {
                    if app.tabs[app.active].marks.items.len() != 1 || ui.get_can_redo_mark() {
                        anyhow::bail!("Closed Review did not return Redo to mark history");
                    }
                    self.pending_key = Some((true, "z".into()));
                    self.stage = 35;
                }
                35 => {
                    if !app.tabs[app.active].marks.items.is_empty() || !ui.get_can_redo_mark() {
                        anyhow::bail!("Closed Review did not return Undo to mark history");
                    }
                    app.history_mark(ui, true);
                    ui.set_review_visible(true);
                    println!(
                        "review: exhausted Review history does not consume independent mark Undo or Redo"
                    );
                    self.stage = 5;
                }
                5 if ready => {
                    let source = self.review_source.unwrap();
                    if ui.get_has_marks() != (!app.tabs[source].marks.items.is_empty())
                        || ui.get_mark_selected() != app.tabs[source].marks.selected.is_some()
                        || ui.get_can_undo_mark() != app.tabs[source].marks.can_undo()
                        || ui.get_export_busy() != app.export_pending
                    {
                        anyhow::bail!("Mark controls do not match the active review workspace");
                    }
                    if let Some(path) = std::env::var_os("DPDF_SMOKE_REVIEW_SNAPSHOT") {
                        let _ = save_snapshot(ui, &PathBuf::from(path));
                    }
                    if mode.starts_with("write") {
                        if app.tabs[source].recovery.saved_generation
                            == Some(app.tabs[source].draft_generation())
                        {
                            println!(
                                "review: mixed checkpoint durable; simulating interrupted process"
                            );
                            use std::io::Write;
                            let _ = std::io::stdout().flush();
                            std::process::exit(0);
                        }
                        return Ok(());
                    }
                    let path = self
                        .second_document
                        .as_ref()
                        .unwrap()
                        .path()
                        .join("missing/output.pdf");
                    app.save_workspace_to(ui, path, crate::review::ExportMode::Current);
                    self.stage = 6;
                }
                6 if !app.export_pending => {
                    let source = self.review_source.unwrap();
                    if !app.tabs[source].unsaved() {
                        anyhow::bail!("Failed mixed save discarded drafts");
                    }
                    self.assembly_pages = app.tabs.len();
                    app.select_remark(ui, 0, false);
                    ui.set_review_tool(1);
                    let path = self
                        .second_document
                        .as_ref()
                        .unwrap()
                        .path()
                        .join("clean-copy.pdf");
                    app.save_workspace_to(ui, path, crate::review::ExportMode::Clean);
                    self.stage = 7;
                }
                7 if !app.export_pending && ready && app.tabs.len() == self.assembly_pages + 1 => {
                    let source = self.review_source.unwrap();
                    if ui.get_review_selected() != -1
                        || ui.get_review_tool() != 0
                        || !ui.get_review_text().is_empty()
                    {
                        anyhow::bail!(
                            "Review edit selection/text leaked into another document tab"
                        );
                    }
                    if app.tabs[source].marks.items.len() != self.review_marks
                        || !app.tabs[source].review.as_ref().unwrap().dirty
                    {
                        anyhow::bail!("Clean export discarded the source review/signature drafts");
                    }
                    if app.tabs[app.active]
                        .review
                        .as_ref()
                        .is_some_and(|r| !r.model.remarks.is_empty())
                    {
                        anyhow::bail!("Clean copy still contains dPDF remarks");
                    }
                    app.activate(ui, source);
                    self.stage = 8;
                }
                8 if ready => {
                    let path = self
                        .second_document
                        .as_ref()
                        .unwrap()
                        .path()
                        .join("older-review-copy.pdf");
                    app.save_workspace_to(ui, path, crate::review::ExportMode::Current);
                    app.select_remark(ui, 0, false);
                    ui.set_review_text("Updated note after older export".into());
                    app.update_remark(ui);
                    self.stage = 9;
                }
                9 if !app.export_pending && ready && app.tabs.len() == self.assembly_pages + 2 => {
                    let source = self.review_source.unwrap();
                    if app.tabs[source].marks.items.len() != self.review_marks
                        || !app.tabs[source].review.as_ref().unwrap().dirty
                    {
                        anyhow::bail!(
                            "Older mixed export discarded newer remarks or signature drafts"
                        );
                    }
                    app.activate(ui, source);
                    self.stage = 11;
                }
                11 if ready => {
                    let path = PathBuf::from(
                        std::env::var_os("DPDF_SMOKE_REVIEW_OUTPUT")
                            .ok_or_else(|| anyhow::anyhow!("Missing review diagnostic output"))?,
                    );
                    app.save_workspace_to(ui, path, crate::review::ExportMode::Current);
                    self.stage = 12;
                }
                12 if !app.export_pending && ready && app.tabs.len() == self.assembly_pages + 3 => {
                    let source = self.review_source.unwrap();
                    if app.tabs[source].unsaved()
                        || !app.tabs[source].marks.items.is_empty()
                        || app.tabs[app.active]
                            .review
                            .as_ref()
                            .unwrap()
                            .model
                            .remarks
                            .len()
                            != 3
                    {
                        anyhow::bail!(
                            "Mixed export lost remarks or did not retire only matching source drafts"
                        );
                    }
                    if app.tabs[source].recovery.saved_generation.is_some() {
                        return Ok(());
                    }
                    println!(
                        "review: clean copy, failed retry, older-export guard, mixed save/reopen and recovery retirement passed"
                    );
                    let _ = slint::quit_event_loop();
                    self.stage = 6;
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.failure = Some(format!("{e:#}"));
            let _ = slint::quit_event_loop();
        }
    }
    fn password_advance(&mut self, app: &mut Application, ui: &Workspace, mode: &str) -> bool {
        if mode == "cancel"
            && self.stage == 60
            && app.opening.is_empty()
            && !ui.get_password_required()
        {
            println!("password: Escape cancelled opening without creating a tab");
            let _ = slint::quit_event_loop();
            return true;
        }
        if mode == "queue"
            && app.tabs.len() == 2
            && app.opening.is_empty()
            && !ui.get_password_required()
        {
            if app.tabs.iter().any(|t| !t.info.password_protected) {
                self.failure = Some("Queued protected PDFs were not authenticated".into());
            }
            println!("password: queued prompts retained in-progress text and opened both PDFs");
            let _ = slint::quit_event_loop();
            return true;
        }
        if !ui.get_password_required() {
            return mode == "cancel" || mode == "queue";
        }
        if mode == "cancel" {
            self.pending_key = Some((false, slint::platform::Key::Escape.into()));
            self.stage = 60;
            return true;
        }
        if mode == "queue" && !self.password_queue_started {
            self.password_queue_started = true;
            ui.set_password_text("in-progress".into());
            if let Some(path) = std::env::var_os("DPDF_SMOKE_PASSWORD_SECOND") {
                app.open(ui, PathBuf::from(path));
            }
            return true;
        }
        if mode == "queue" && self.password_attempts == 0 && app.passwords.len() < 2 {
            return true;
        }
        if mode == "queue"
            && self.password_attempts == 0
            && ui.get_password_text().as_str() != "in-progress"
        {
            self.failure = Some("A queued PDF cleared in-progress password text".into());
            let _ = slint::quit_event_loop();
            return true;
        }
        if self.password_attempts == 0 {
            if let Some(path) = std::env::var_os("DPDF_SMOKE_PASSWORD_SNAPSHOT") {
                let _ = save_snapshot(ui, &PathBuf::from(path));
            }
            ui.set_password_text("deliberately-wrong-test-value".into());
            self.password_attempts = 1;
        } else {
            if self.password_attempts == 1
                && mode != "queue"
                && !ui.get_password_error().contains("Incorrect")
            {
                self.failure = Some("Wrong password was not reported as a retry".into());
                let _ = slint::quit_event_loop();
                return true;
            }
            ui.set_password_text(
                std::env::var("DPDF_SMOKE_PASSWORD_VALUE")
                    .unwrap_or_default()
                    .into(),
            );
            self.password_attempts += 1;
        }
        self.pending_key = Some((false, slint::platform::Key::Return.into()));
        true
    }
    fn recovery_advance(&mut self, app: &mut Application, ui: &Workspace, mode: &str) {
        let result = (|| -> anyhow::Result<()> {
            match self.stage {
                0 if mode != "clear" && ui.get_has_recovery() && !ui.get_recovery_busy() => {
                    app.recover_draft(ui, mode == "discard");
                    self.stage = 1;
                }
                0 if mode == "clear" && ui.get_has_recovery() && !ui.get_recovery_busy() => {
                    let root = crate::recovery::default_root()
                        .ok_or_else(|| anyhow::anyhow!("Recovery root unavailable"))?;
                    let drafts = || -> anyhow::Result<usize> {
                        Ok(std::fs::read_dir(&root)?
                            .filter_map(Result::ok)
                            .filter(|e| e.path().extension().is_some_and(|x| x == "dpdf"))
                            .count())
                    };
                    app.set_recovery_enabled(ui, false);
                    if app.recovery.is_some() || ui.get_has_recovery() || ui.get_recovery_enabled()
                    {
                        anyhow::bail!("Disabling recovery left the store active or offered");
                    }
                    if drafts()? == 0 {
                        anyhow::bail!("Disabling recovery deleted existing drafts");
                    }
                    app.clear_recovery(ui);
                    if drafts()? != 0 {
                        anyhow::bail!("Deleting drafts while recovery is off left files behind");
                    }
                    app.set_recovery_enabled(ui, true);
                    if app.recovery.is_none() || !ui.get_recovery_enabled() {
                        anyhow::bail!("Recovery could not be re-enabled");
                    }
                    app.clear_recovery(ui);
                    self.stage = 1;
                }
                1 if mode == "clear" && !ui.get_recovery_busy() && !ui.get_has_recovery() => {
                    app.privacy_controls(ui);
                    ui.set_privacy_visible(true);
                    self.stage = 2;
                }
                2 if mode == "clear" => {
                    if !ui.get_recovery_enabled()
                        || !ui.get_recovery_summary().starts_with("0 saved draft")
                    {
                        anyhow::bail!("Privacy panel does not report the active, empty store");
                    }
                    if let Some(path) = std::env::var_os("DPDF_SMOKE_SNAPSHOT") {
                        save_snapshot(ui, &PathBuf::from(path))?;
                    }
                    println!(
                        "recovery: disabling kept drafts; delete-all removed them while off and on"
                    );
                    let _ = slint::quit_event_loop();
                    self.stage = 6;
                }
                1 if mode == "discard" && !ui.get_has_recovery() && !ui.get_recovery_busy() => {
                    println!("recovery: explicit discard removed the abandoned draft");
                    let _ = slint::quit_event_loop();
                    self.stage = 6;
                }
                1 if mode == "read"
                    && app.tabs.iter().any(|t| !t.marks.items.is_empty())
                    && app.pages.iter().any(|p| p.ready) =>
                {
                    let tab = &app.tabs[app.active];
                    let mark = &tab.marks.items[0];
                    if tab.info.pages.len() != 50
                        || tab.marks.items.len() != 1
                        || (mark.placement.geometry.width_mm - 35.0).abs() > 0.001
                        || (mark.placement.geometry.height_mm - 25.0).abs() > 0.001
                        || (mark.placement.opacity - 0.7).abs() > 0.001
                        || mark.default_size != (45.0, 22.5)
                        || mark.aspect_locked
                    {
                        anyhow::bail!(
                            "Recovered snapshot or physical mark controls differ from the checkpoint"
                        );
                    }
                    if tab.recovery.saved_generation == Some(tab.marks.generation) {
                        println!(
                            "recovery: opened retained 50-page snapshot and physical marks; replacement checkpoint durable"
                        );
                        let output = PathBuf::from(
                            std::env::var_os("DPDF_SMOKE_RECOVERY_OUTPUT")
                                .ok_or_else(|| anyhow::anyhow!("Missing recovery output path"))?,
                        );
                        app.save_marks_to(ui, output);
                        self.stage = 2;
                    }
                }
                2 if !app.export_pending
                    && app.tabs.iter().all(|t| t.marks.items.is_empty())
                    && app.tabs.len() >= 2 =>
                {
                    self.stage = 3;
                }
                3 if app
                    .tabs
                    .iter()
                    .all(|t| t.recovery.saved_generation.is_none()) =>
                {
                    println!("recovery: saved recovered copy and retired its checkpoint");
                    let _ = slint::quit_event_loop();
                    self.stage = 6;
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.failure = Some(format!("{error:#}"));
            let _ = slint::quit_event_loop();
        }
    }
    pub(super) fn advance(&mut self, app: &mut Application, ui: &Workspace) {
        self.ticks += 1;
        if self.ticks > 1250 {
            self.failure = Some(format!(
                "UI smoke timed out at stage {}: {}",
                self.stage,
                ui.get_status()
            ));
            let _ = slint::quit_event_loop();
            return;
        }
        if let Some(error) = &app.engine_error {
            self.failure = Some(error.clone());
            let _ = slint::quit_event_loop();
            return;
        }
        if let Ok(mode) = std::env::var("DPDF_SMOKE_HELPER") {
            self.helper_advance(app, ui, &mode);
            return;
        }
        if std::env::var_os("DPDF_SMOKE_PHYSICAL").is_some() {
            match self.physical.advance(app, ui) {
                Ok(true) => {
                    let _ = slint::quit_event_loop();
                }
                Ok(false) => {}
                Err(error) => {
                    self.failure = Some(format!("{error:#}"));
                    let _ = slint::quit_event_loop();
                }
            }
            return;
        }
        if std::env::var_os("DPDF_SMOKE_VIEWPORT").is_some() {
            match self.viewport.advance(app, ui) {
                Ok(true) => {
                    let _ = slint::quit_event_loop();
                }
                Ok(false) => {}
                Err(error) => {
                    self.failure = Some(format!("{error:#}"));
                    let _ = slint::quit_event_loop();
                }
            }
            self.pending_key = self.viewport.pending_key.take();
            self.pending_selection = self.viewport.pending_selection.take();
            return;
        }
        if std::env::var_os("DPDF_SMOKE_TABS").is_some() {
            self.tabs_advance(app, ui);
            return;
        }
        if let Ok(mode) = std::env::var("DPDF_SMOKE_REVIEW") {
            self.review_advance(app, ui, &mode);
            return;
        }
        if let Ok(mode) = std::env::var("DPDF_SMOKE_PASSWORD")
            && self.password_advance(app, ui, &mode)
        {
            return;
        }
        if let Ok(mode) = std::env::var("DPDF_SMOKE_RECOVERY")
            && (mode == "read" || mode == "discard" || mode == "clear")
        {
            self.recovery_advance(app, ui, &mode);
            return;
        }
        let Some(tab) = app.tabs.get(app.active) else {
            return;
        };
        let ready = app.pages.iter().any(|p| p.ready);
        match self.stage {
            0 if ready && !app.thumb_frames.is_empty() => {
                if std::env::var_os("DPDF_SMOKE_PASSWORD").is_some() {
                    if !tab.info.password_protected || self.password_attempts < 2 {
                        self.failure =
                            Some("Password authentication/retry path was not exercised".into());
                        let _ = slint::quit_event_loop();
                        return;
                    }
                    if let Some(path) = std::env::var_os("DPDF_SMOKE_PASSWORD_COPY") {
                        app.command(ui, Command::SaveCopy(tab.info.id, PathBuf::from(path)));
                    }
                    println!(
                        "password: incorrect-password retry succeeded; authenticated reading ready"
                    );
                }
                println!(
                    "smoke: first page and thumbnails ready; pages={} bookmarks={}",
                    tab.info.pages.len(),
                    tab.info.bookmarks.len()
                );
                let placement = &app.layout.as_ref().unwrap().pages[0];
                let position = |x: f32, y: f32| {
                    slint::LogicalPosition::new(
                        ui.get_viewport_window_x()
                            + ui.get_scroll_x()
                            + placement.x
                            + x * placement.width,
                        ui.get_viewport_window_y()
                            + ui.get_scroll_y()
                            + placement.y
                            + y * placement.height,
                    )
                };
                self.pending_selection = Some((position(0.06, 0.04), position(0.95, 0.25)));
                self.stage = 20;
            }
            20 if app.selection_ready => {
                if !app.selected_text.contains("payment")
                    || app.selected_text.contains("Authorized signature:")
                {
                    self.failure = Some(
                        "Dispatched selection did not extract only the selected region".into(),
                    );
                    let _ = slint::quit_event_loop();
                    return;
                }
                self.pending_key = Some((true, "a".into()));
                self.stage = 21;
            }
            21 if app.selection_ready && app.selected_text.contains("Authorized signature:") => {
                println!(
                    "smoke: dispatched text-region drag and Ctrl+A selection extract expected text"
                );
                let detail = ui.get_status_detail();
                let expected = format!(
                    "Page {} of {}  ·  ",
                    ui.get_current_page() + 1,
                    app.tabs[app.active].info.pages.len()
                );
                if !detail.starts_with(&expected) || !detail.contains(ui.get_zoom_label().as_str())
                {
                    self.failure = Some(format!("Status bar summary is incomplete: {detail}"));
                    let _ = slint::quit_event_loop();
                    return;
                }
                if std::env::var_os("DPDF_SMOKE_CLIPBOARD").is_some() {
                    // Opt-in only: use an isolated display. Ordinary smoke never changes clipboard contents.
                    self.pending_key = Some((true, "c".into()));
                    self.stage = 22;
                    return;
                }
                app.clear_selection(ui);
                ui.set_page_ops_visible(true);
                ui.set_page_operation(7);
                self.stage = 23;
            }
            22 if ui.get_status().starts_with("Copied ") => {
                let readback =
                    arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text());
                if readback.ok().as_deref() != Some(app.selected_text.as_str()) {
                    self.failure =
                        Some("Native clipboard did not return the selected fixture text".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                println!(
                    "smoke: dispatched Ctrl+C copied text; independent native clipboard readback matched"
                );
                app.share_file(ui, 1);
                let expected = platform::saved_file_path(&app.tabs[app.active].info.path).unwrap();
                let copied =
                    arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text());
                if copied.ok().as_deref() != Some(expected.as_str()) || !ui.get_share_ready() {
                    self.failure =
                        Some("Saved-file sharing did not copy the exact fixture path".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                println!(
                    "smoke: saved-file sharing copied exact path to the independent native clipboard"
                );
                app.clear_selection(ui);
                ui.set_share_visible(true);
                self.stage = 60;
            }
            60 if ui.get_share_visible() && ready => {
                let result = if let Some(path) = std::env::var_os("DPDF_SMOKE_SHARE_SNAPSHOT") {
                    save_snapshot(ui, &PathBuf::from(path))
                } else {
                    ui.window()
                        .take_snapshot()
                        .map(|_| ())
                        .map_err(|error| anyhow::anyhow!("{error}"))
                };
                if let Err(error) = result {
                    self.failure = Some(format!("Share panel snapshot failed: {error:#}"));
                    let _ = slint::quit_event_loop();
                    return;
                }
                self.pending_key = Some((false, slint::platform::Key::Escape.into()));
                self.stage = 61;
            }
            61 if !ui.get_share_visible() => {
                println!("smoke: share panel rendered and dispatched Escape closed it");
                app.clear_selection(ui);
                ui.set_page_ops_visible(true);
                ui.set_page_operation(7);
                self.stage = 23;
            }
            23 if app.arrangement.is_some()
                && app.sidebar.row_count() >= 2
                && app.thumb_frames.contains(&0)
                && app.thumb_frames.contains(&1) =>
            {
                // The arrangement header changes layout. Render that layout before dispatching
                // synthetic input, rather than reading geometry from the preceding frame.
                if let Err(error) = ui.window().take_snapshot() {
                    self.failure = Some(format!("Cannot settle arrangement layout: {error}"));
                    let _ = slint::quit_event_loop();
                    return;
                }
                let y = ui.get_sidebar_window_y();
                let x = ui.get_sidebar_window_x() + ui.get_sidebar_width() / 2.0;
                let height = ui.get_thumbnail_height();
                let stride = height + ui.get_thumbnail_spacing();
                self.pending_selection = Some((
                    slint::LogicalPosition::new(x, y + height * 0.25),
                    slint::LogicalPosition::new(x, y + stride + height * 0.75),
                ));
                self.stage = 24;
            }
            24 if ui.get_operation_pages().starts_with("2,1")
                && app.thumb_frames.contains(&0)
                && app.thumb_frames.contains(&1) =>
            {
                if app.sidebar.row_data(0).map(|p| p.page) != Some(1)
                    || app.sidebar.row_data(1).map(|p| p.page) != Some(0)
                {
                    self.failure =
                        Some("Thumbnail order does not match exported page-order field".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                println!(
                    "smoke: dispatched thumbnail drag reordered output slots without modifying the open PDF"
                );
                if let Some(path) = std::env::var_os("DPDF_SMOKE_ORDER_SNAPSHOT")
                    && let Err(error) = save_snapshot(ui, &PathBuf::from(path))
                {
                    self.failure = Some(format!("Order snapshot failed: {error:#}"));
                    let _ = slint::quit_event_loop();
                    return;
                }
                self.stage = 70;
            }
            70 => {
                let _ = ui.window().take_snapshot();
                let x = ui.get_sidebar_window_x() + ui.get_sidebar_width() / 2.0;
                let y = ui.get_sidebar_window_y();
                let start = slint::LogicalPosition::new(x, y + ui.get_thumbnail_height() * 0.25);
                let end = slint::LogicalPosition::new(x, y + ui.get_sidebar_height() - 10.0);
                use slint::platform::{PointerEventButton, WindowEvent};
                self.pending_pointer = vec![
                    WindowEvent::PointerMoved { position: start },
                    WindowEvent::PointerPressed {
                        position: start,
                        button: PointerEventButton::Left,
                    },
                    WindowEvent::PointerMoved {
                        position: slint::LogicalPosition::new(start.x, start.y + 20.0),
                    },
                    WindowEvent::PointerMoved { position: end },
                ];
                self.stage = 71;
            }
            71 if ui.get_sidebar_offset()
                >= 6.0 * (ui.get_thumbnail_height() + ui.get_thumbnail_spacing()) =>
            {
                self.order_scroll_before = ui.get_sidebar_offset();
                self.order_ticks = 0;
                self.pending_pointer
                    .push(slint::platform::WindowEvent::PointerMoved {
                        position: slint::LogicalPosition::new(
                            ui.get_sidebar_window_x() + ui.get_sidebar_width() / 2.0,
                            ui.get_sidebar_window_y() + ui.get_sidebar_height() / 2.0,
                        ),
                    });
                self.stage = 72;
            }
            72 => {
                self.order_ticks += 1;
                if self.order_ticks < 3 {
                    return;
                }
                if (ui.get_sidebar_offset() - self.order_scroll_before).abs() > 1.0 {
                    self.failure =
                        Some("Thumbnail scrolling continued after leaving the edge".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                let height = ui.get_thumbnail_height();
                let stride = height + ui.get_thumbnail_spacing();
                let position = ui.get_sidebar_offset() + ui.get_sidebar_height() - 10.0;
                let row = (position / stride) as usize;
                let after = position - row as f32 * stride >= height / 2.0;
                self.order_expected = app
                    .arrangement
                    .as_ref()
                    .unwrap()
                    .plan
                    .pages()
                    .iter()
                    .map(|p| p.page)
                    .collect();
                let moved = self.order_expected.remove(0);
                self.order_expected
                    .insert(row + usize::from(after) - 1, moved);
                let end = slint::LogicalPosition::new(
                    ui.get_sidebar_window_x() + ui.get_sidebar_width() / 2.0,
                    ui.get_sidebar_window_y() + ui.get_sidebar_height() - 10.0,
                );
                use slint::platform::{PointerEventButton, WindowEvent};
                self.pending_pointer = vec![
                    WindowEvent::PointerMoved { position: end },
                    WindowEvent::PointerReleased {
                        position: end,
                        button: PointerEventButton::Left,
                    },
                ];
                self.stage = 73;
            }
            73 if ui.get_page_drag_scroll() == 0 => {
                let actual: Vec<_> = app
                    .arrangement
                    .as_ref()
                    .unwrap()
                    .plan
                    .pages()
                    .iter()
                    .map(|p| p.page)
                    .collect();
                if actual != self.order_expected {
                    self.failure = Some(format!(
                        "Scrolled thumbnail drop targeted the wrong output slot: {actual:?}"
                    ));
                    let _ = slint::quit_event_loop();
                    return;
                }
                println!(
                    "smoke: held thumbnail drag scrolled beyond the viewport and dropped in the exact output slot"
                );
                let _ = ui.window().take_snapshot();
                let stride = ui.get_thumbnail_height() + ui.get_thumbnail_spacing();
                let row = (ui.get_sidebar_offset() / stride).ceil();
                let x = ui.get_sidebar_window_x() + ui.get_sidebar_width() / 2.0;
                let start = slint::LogicalPosition::new(
                    x,
                    ui.get_sidebar_window_y() + row * stride - ui.get_sidebar_offset()
                        + ui.get_thumbnail_height() * 0.5,
                );
                let end = slint::LogicalPosition::new(x, ui.get_sidebar_window_y() + 10.0);
                use slint::platform::{PointerEventButton, WindowEvent};
                self.pending_pointer = vec![
                    WindowEvent::PointerMoved { position: start },
                    WindowEvent::PointerPressed {
                        position: start,
                        button: PointerEventButton::Left,
                    },
                    WindowEvent::PointerMoved {
                        position: slint::LogicalPosition::new(start.x, start.y - 20.0),
                    },
                    WindowEvent::PointerMoved { position: end },
                ];
                self.order_ticks = 0;
                self.stage = 74;
            }
            74 if ui.get_sidebar_offset() <= 0.01 => {
                self.order_ticks += 1;
                if self.order_ticks < 3 {
                    return;
                }
                if ui.get_sidebar_offset() < -0.01 {
                    self.failure = Some("Thumbnail edge drag scrolled past the first page".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                let end = slint::LogicalPosition::new(
                    ui.get_viewport_window_x() + ui.get_viewport_width() / 2.0,
                    ui.get_viewport_window_y() + ui.get_viewport_height() / 2.0,
                );
                use slint::platform::{PointerEventButton, WindowEvent};
                self.pending_pointer = vec![
                    WindowEvent::PointerMoved { position: end },
                    WindowEvent::PointerReleased {
                        position: end,
                        button: PointerEventButton::Left,
                    },
                ];
                self.order_ticks = 0;
                self.stage = 75;
            }
            75 => {
                self.order_ticks += 1;
                if self.order_ticks < 3 {
                    return;
                }
                let actual: Vec<_> = app
                    .arrangement
                    .as_ref()
                    .unwrap()
                    .plan
                    .pages()
                    .iter()
                    .map(|p| p.page)
                    .collect();
                if actual != self.order_expected
                    || ui.get_page_drag_scroll() != 0
                    || ui.get_sidebar_offset().abs() > 0.01
                {
                    self.failure = Some("Cancelling a scrolled thumbnail drag changed the order or left scrolling active".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                println!(
                    "smoke: upward thumbnail edge scroll stopped at the first page; cancelled drag retained output order"
                );
                ui.set_page_ops_visible(false);
                ui.set_page_operation(0);
                self.stage = 25;
            }
            25 if app.arrangement.is_none()
                && app.sidebar.row_data(0).map(|p| p.page) == Some(0) =>
            {
                println!("smoke: closing arrangement restored original thumbnail order");
                app.go_to(ui, 6);
                self.stage = 1;
            }
            1 if ready => {
                println!("smoke: page navigation ready");
                // A saturated worker must not lose the final typed query. The
                // real worker is restored before the timer retries the request.
                let (blocked, queued) = mpsc::sync_channel(1);
                let id = tab.info.id;
                assert!(blocked.try_send(Command::Snapshot(id)).is_ok());
                let live = std::mem::replace(&mut app.worker.commands, blocked);
                app.search(ui, "obsolete-query-that-is-not-in-the-fixture".into());
                ui.set_query("payment".into());
                app.search(ui, "payment".into());
                app.worker.commands = live;
                assert!(matches!(queued.try_recv(), Ok(Command::Snapshot(_))));
                println!("smoke: latest search deferred behind a full PDF worker queue");
                self.stage = 2;
            }
            2 if !tab.hits.is_empty() => {
                println!("smoke: search found {} occurrences", tab.hits.len());
                if tab.query != "payment" {
                    self.failure = Some("Deferred search did not retain the latest query".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                ui.set_search_visible(true);
                if tab.hits.iter().any(|hit| hit.bounds.is_empty()) {
                    self.failure =
                        Some("Search returned a fixture occurrence without visible bounds".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                app.next_result(ui, 1);
                let active = app.active;
                app.tabs[active].view.mode = ViewMode::Continuous;
                ui.set_view_mode(1);
                app.go_to(ui, 49);
                self.stage = 3;
            }
            3 if ready
                && app.tabs[app.active].view.page
                    == app.tabs[app.active].info.pages.len().min(50) - 1 =>
            {
                println!(
                    "smoke: continuous navigation ready; {} UI tiles for {} pages",
                    app.pages.row_count(),
                    app.tabs[app.active].info.pages.len()
                );
                if app.tabs[app.active].info.pages.len() >= 50 && app.pages.row_count() >= 10 {
                    self.failure = Some("Continuous view instantiated too many page tiles".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                self.scale_before_input = app.layout.as_ref().unwrap().scale;
                self.pending_input = Some((true, 120.0));
                self.stage = 7;
            }
            7 if ready && app.layout.as_ref().unwrap().scale > self.scale_before_input => {
                println!("smoke: dispatched Ctrl+wheel changed zoom");
                self.scale_before_input = app.layout.as_ref().unwrap().scale;
                self.scroll_before_input = ui.get_scroll_y();
                self.pending_input = Some((false, 120.0));
                self.stage = 8;
            }
            8 if ui.get_scroll_y() > self.scroll_before_input + 1.0 => {
                if (app.layout.as_ref().unwrap().scale - self.scale_before_input).abs() > 0.0001 {
                    self.failure = Some("Normal scrolling unexpectedly changed zoom".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                println!("smoke: dispatched ordinary wheel scrolls without zoom");
                self.original_scroll = (ui.get_scroll_x(), ui.get_scroll_y());
                let copy = (|| -> anyhow::Result<tempfile::TempDir> {
                    let folder = tempfile::tempdir()?;
                    let path = folder.path().join("second-tab.pdf");
                    std::fs::copy(&app.tabs[app.active].info.path, &path)?;
                    app.open(ui, path);
                    Ok(folder)
                })();
                match copy {
                    Ok(folder) => self.second_document = Some(folder),
                    Err(error) => {
                        self.failure = Some(format!("Second-tab fixture failed: {error:#}"));
                        let _ = slint::quit_event_loop();
                        return;
                    }
                }
                self.stage = 9;
            }
            9 if app.tabs.len() == 2 && app.active == 1 && ready => {
                app.tabs[1].view.mode = ViewMode::Continuous;
                ui.set_view_mode(1);
                app.go_to(ui, 6);
                self.stage = 10;
            }
            10 if ready => {
                app.close(ui, 1);
                self.stage = 11;
            }
            11 if app.tabs.len() == 1 && ready => {
                if (ui.get_scroll_x() - self.original_scroll.0).abs() > 2.0
                    || (ui.get_scroll_y() - self.original_scroll.1).abs() > 2.0
                {
                    self.failure = Some(format!(
                        "Closing a tab corrupted saved scroll: expected {:?}, got ({}, {})",
                        self.original_scroll,
                        ui.get_scroll_x(),
                        ui.get_scroll_y()
                    ));
                    let _ = slint::quit_event_loop();
                    return;
                }
                println!("smoke: closing second tab restored original view and scroll");
                let active = app.active;
                app.tabs[active].view.mode = ViewMode::Facing;
                app.tabs[active].view.rotation = 1;
                app.tabs[active].view.zoom = Zoom::FitPage;
                ui.set_view_mode(2);
                ui.set_dark(true);
                app.relayout(ui, true);
                self.stage = 4;
            }
            4 if app.pages.iter().all(|page| page.ready) => {
                println!("smoke: facing/rotation/fit/dark ready");
                self.stage = 80;
            }
            80 if ready => {
                // A relayout or page change while the user types must not reset the entry.
                let count = tab.info.pages.len();
                self.entry_target = count.min(12);
                let typed = self.entry_target.to_string();
                ui.set_jump_editing(true);
                ui.set_zoom_editing(true);
                ui.set_jump_text(typed[..1].into());
                ui.set_zoom_entry("17".into());
                app.go_to(ui, 2.min(count - 1));
                if ui.get_jump_text().as_str() != &typed[..1] || ui.get_zoom_entry() != "17" {
                    self.failure = Some(
                        "Navigation overwrote a page/zoom entry while it was being typed".into(),
                    );
                    let _ = slint::quit_event_loop();
                    return;
                }
                ui.set_jump_text(typed.as_str().into());
                app.relayout(ui, true);
                if ui.get_jump_text() != typed.as_str() || ui.get_zoom_entry() != "17" {
                    self.failure = Some(
                        "Relayout overwrote a page/zoom entry while it was being typed".into(),
                    );
                    let _ = slint::quit_event_loop();
                    return;
                }
                ui.set_zoom_editing(false);
                ui.set_zoom_entry(ui.get_zoom_label());
                app.jump(ui, &typed);
                self.stage = 81;
            }
            81 if ready && tab.view.page == self.entry_target - 1 => {
                ui.set_jump_editing(false);
                if ui.get_current_page() as usize != self.entry_target - 1 {
                    self.failure =
                        Some("Accepted page entry did not update the current page".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                ui.set_status("".into());
                for invalid in ["0", "page", "-3", ""] {
                    app.jump(ui, invalid);
                }
                if app.tabs[app.active].view.page != self.entry_target - 1
                    || !ui.get_status().contains("starting at 1")
                {
                    self.failure =
                        Some("Invalid page entries moved the view or gave no explanation".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                println!(
                    "smoke: slowly typed page entry survived navigation/relayout; invalid entries refused"
                );
                let last = app.tabs[app.active].info.pages.len();
                app.jump(ui, &(last + 5).to_string());
                self.stage = 82;
            }
            82 if ready && tab.view.page == tab.info.pages.len() - 1 => {
                app.jump(ui, "1");
                self.stage = 83;
            }
            83 if ready && tab.view.page == 0 => {
                println!("smoke: page entry reached the last page (clamped) and page 1");
                let bookmarks = tab.info.bookmarks.len();
                ui.set_sidebar_visible(true);
                ui.set_sidebar_tab(1);
                app.rebuild_sidebar(ui);
                let rows = app.sidebar.row_count();
                if rows != bookmarks {
                    self.failure = Some("Bookmark sidebar does not list every bookmark".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                // Use the sidebar row exactly as its click handler does: jump(page + 1).
                self.bookmark_target = (rows > 0)
                    .then(|| app.sidebar.row_data(rows / 2))
                    .flatten()
                    .map(|row| row.page as usize);
                match self.bookmark_target {
                    Some(page) if page > 0 => app.jump(ui, &(page + 1).to_string()),
                    Some(_) | None => self.bookmark_target = None,
                }
                self.stage = 84;
            }
            84 if ready
                && self
                    .bookmark_target
                    .is_none_or(|page| tab.view.page == page) =>
            {
                match self.bookmark_target {
                    Some(page) => println!(
                        "smoke: bookmark sidebar navigation reached page {}",
                        page + 1
                    ),
                    None => println!(
                        "smoke: fixture has no bookmark beyond page 1; bookmark navigation skipped"
                    ),
                }
                let (id, source) = (tab.info.id, tab.info.path.clone());
                ui.set_sidebar_tab(0);
                app.rebuild_sidebar(ui);
                let result = (|| -> anyhow::Result<tempfile::TempDir> {
                    let folder = tempfile::tempdir()?;
                    self.save_source = std::fs::read(&source)?;
                    Ok(folder)
                })();
                match result {
                    Ok(folder) => {
                        let destination = folder.path().join("saved copy Ω.pdf");
                        self.save_folder = Some(folder);
                        ui.set_status("".into());
                        app.command(ui, Command::SaveCopy(id, destination));
                        self.stage = 85;
                    }
                    Err(error) => {
                        self.failure = Some(format!("Save As fixture failed: {error:#}"));
                        let _ = slint::quit_event_loop();
                    }
                }
            }
            85 if ui.get_status().starts_with("Saved unchanged PDF to")
                || ui.get_status().starts_with("Save failed") =>
            {
                let folder = self.save_folder.as_ref().unwrap().path().to_path_buf();
                let saved = std::fs::read(folder.join("saved copy Ω.pdf")).unwrap_or_default();
                if ui.get_status().starts_with("Save failed") || saved != self.save_source {
                    self.failure = Some(format!(
                        "Save As after zoom/rotation/navigation did not write identical bytes: {}",
                        ui.get_status()
                    ));
                    let _ = slint::quit_event_loop();
                    return;
                }
                println!("smoke: Save As after view changes wrote byte-identical PDF");
                let directory_target = folder.join("existing folder.pdf");
                if let Err(error) = std::fs::create_dir(&directory_target) {
                    self.failure = Some(format!("Save As refusal fixture failed: {error}"));
                    let _ = slint::quit_event_loop();
                    return;
                }
                ui.set_status("".into());
                app.command(
                    ui,
                    Command::SaveCopy(tab.info.id, folder.join("missing folder").join("out.pdf")),
                );
                self.stage = 86;
            }
            86 if !ui.get_status().is_empty() => {
                if !ui.get_status().starts_with("Save failed") {
                    self.failure = Some(format!(
                        "Save into a missing folder did not fail clearly: {}",
                        ui.get_status()
                    ));
                    let _ = slint::quit_event_loop();
                    return;
                }
                let folder = self.save_folder.as_ref().unwrap().path().to_path_buf();
                ui.set_status("".into());
                app.command(
                    ui,
                    Command::SaveCopy(tab.info.id, folder.join("existing folder.pdf")),
                );
                self.stage = 87;
            }
            87 if !ui.get_status().is_empty() => {
                let folder = self.save_folder.as_ref().unwrap().path().to_path_buf();
                let intact = std::fs::read(&tab.info.path)
                    .is_ok_and(|bytes| bytes == self.save_source)
                    && std::fs::read(folder.join("saved copy Ω.pdf"))
                        .is_ok_and(|bytes| bytes == self.save_source)
                    && folder.join("existing folder.pdf").is_dir()
                    && !folder.join("missing folder").exists();
                if !ui.get_status().starts_with("Save failed") || !intact {
                    self.failure = Some(format!(
                        "Unwritable Save As destinations were not refused safely: {}",
                        ui.get_status()
                    ));
                    let _ = slint::quit_event_loop();
                    return;
                }
                println!(
                    "smoke: unwritable Save As destinations failed clearly; source and earlier copy intact"
                );
                self.save_folder = None;
                self.pending_print_dialog = true;
                self.stage = 14;
            }
            14 if ui.get_print_visible() && !app.printer_loading => {
                println!(
                    "smoke: print panel discovery completed; {} printer choices",
                    ui.get_printer_choices().row_count()
                );
                if ui.get_printer_choices().row_count() < 2 {
                    self.failure = Some("Print destination choices are missing".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                let pages = tab.info.pages.len();
                for range in ["0", "3-1", &(pages + 1).to_string(), "1-3;7", "a", "1-2-3"] {
                    ui.set_status("".into());
                    ui.set_print_range(range.into());
                    app.print(ui);
                    if app.print_pending || ui.get_status().is_empty() || !ui.get_print_visible() {
                        self.failure = Some(format!(
                            "Invalid print range {range:?} was not refused before submission"
                        ));
                        let _ = slint::quit_event_loop();
                        return;
                    }
                }
                ui.set_print_range("".into());
                println!(
                    "smoke: invalid/out-of-bounds print ranges refused without submitting a job"
                );
                ui.set_print_visible(false);
                self.stage = 15;
            }
            15 if app.pages.iter().all(|page| page.ready) => {
                if std::env::var_os("DPDF_SMOKE_PLACEMENT_IMAGE").is_some() {
                    app.tabs[0].view.mode = ViewMode::Single;
                    app.tabs[0].view.rotation = 0;
                    app.tabs[0].view.zoom = Zoom::FitPage;
                    ui.set_view_mode(0);
                    app.go_to(ui, 0);
                    self.stage = 30;
                } else if std::env::var_os("DPDF_SMOKE_ASSEMBLY_SOURCE").is_some() {
                    let result = (|| -> anyhow::Result<()> {
                        let folder = self
                            .second_document
                            .as_ref()
                            .ok_or_else(|| anyhow::anyhow!("Missing private smoke workspace"))?;
                        let path = folder.path().join("working-original.pdf");
                        std::fs::copy(&app.tabs[0].info.path, &path)?;
                        self.assembly_pages = app.tabs[0].info.pages.len();
                        app.open(ui, path);
                        Ok(())
                    })();
                    if let Err(error) = result {
                        self.failure = Some(error.to_string());
                        let _ = slint::quit_event_loop();
                        return;
                    }
                    self.stage = 16;
                } else {
                    self.stage = 5;
                }
            }
            16 if app.tabs.len() == 2 && app.active == 1 && ready => {
                // The file is a diagnostic-owned copy, never the input supplied by the user.
                // Corrupting that copy after opening proves export uses the displayed snapshot.
                let source = PathBuf::from(std::env::var_os("DPDF_SMOKE_ASSEMBLY_SOURCE").unwrap());
                let folder = self.second_document.as_ref().unwrap();
                let working = folder.path().join("working-original.pdf");
                if let Err(error) = std::fs::write(&working, b"changed on disk after opening") {
                    self.failure = Some(error.to_string());
                    let _ = slint::quit_event_loop();
                    return;
                }
                let id = app.tabs[1].info.id;
                let destination = folder.path().join("assembled-output.pdf");
                ui.set_page_ops_visible(true);
                ui.set_operation_pages(self.assembly_pages.to_string().into());
                let operation = PageOperation {
                    kind: OperationKind::Replace,
                    pages: self.assembly_pages.to_string(),
                    source_pages: "1".into(),
                };
                if app.command(
                    ui,
                    Command::ExportPages {
                        id,
                        operation,
                        additional: vec![source],
                        destination,
                    },
                ) {
                    app.export_pending = true;
                    ui.set_export_busy(true);
                    self.stage = 17;
                }
            }
            17 if app.tabs.len() == 3 && app.active == 2 && ready && !app.export_pending => {
                if app.tabs[2].info.pages.len() != self.assembly_pages {
                    self.failure = Some("Assembled copy has the wrong page count".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                app.go_to(ui, self.assembly_pages - 1);
                self.stage = 18;
            }
            18 if ready => {
                println!(
                    "smoke: page assembly exported the opened snapshot after its disk file changed; edited copy opened and rendered"
                );
                self.stage = 5;
            }
            30 if ready => {
                let outcome = (|| -> anyhow::Result<()> {
                    let source = crate::marks::read_image(&PathBuf::from(
                        std::env::var_os("DPDF_SMOKE_PLACEMENT_IMAGE").unwrap(),
                    ))?;
                    let cleaned = crate::marks::cleanup(&source.image, &Default::default())?;
                    let preview = image::imageops::thumbnail(&cleaned, 400, 400);
                    let make = |rgba: image::RgbaImage| {
                        slint::Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                            rgba.as_raw(),
                            rgba.width(),
                            rgba.height(),
                        ))
                    };
                    let root =
                        PathBuf::from(std::env::var_os("DPDF_DATA_DIR").ok_or_else(|| {
                            anyhow::anyhow!(
                                "Placement usage diagnostic requires isolated DPDF_DATA_DIR"
                            )
                        })?)
                        .join("marks");
                    let asset = crate::marks::import_snapshot(
                        &root,
                        &source,
                        crate::marks::ImportOptions {
                            name: "Placement usage fixture".into(),
                            kind: crate::marks::MarkKind::Stamp,
                            notes: "Generated placement diagnostic".into(),
                            preferred_size: Some(crate::marks::PreferredSize {
                                width_mm: 45.0,
                                height_mm: 22.5,
                                aspect_locked: true,
                            }),
                            cleanup: Default::default(),
                        },
                    )?;
                    let asset_use = crate::marks::AssetUse { root, id: asset.id };
                    self.usage_fixture = Some(asset_use.clone());
                    app.pending_mark = Some(crate::mark_studio::StudioMark {
                        asset_use: Some(asset_use),
                        document: app.tabs[0].info.id,
                        bytes: std::sync::Arc::new(crate::marks::png_ref(&cleaned)?),
                        previews: [
                            make(preview.clone()),
                            make(image::imageops::rotate90(&preview)),
                            make(image::imageops::rotate180(&preview)),
                            make(image::imageops::rotate270(&preview)),
                        ],
                        size: crate::marks::PreferredSize {
                            width_mm: 45.0,
                            height_mm: 22.5,
                            aspect_locked: true,
                        },
                    });
                    ui.set_mark_pending(true);
                    Ok(())
                })();
                if let Err(error) = outcome {
                    self.failure = Some(error.to_string());
                    let _ = slint::quit_event_loop();
                    return;
                }
                self.stage = 31;
            }
            31 if ready => {
                self.pending_mark_fixture = app.pending_mark.clone();
                self.pending_key = Some((false, slint::platform::Key::Escape.into()));
                self.stage = 39;
            }
            39 if app.pending_mark.is_none() && !ui.get_mark_pending() => {
                if let Some(asset) = &self.usage_fixture {
                    if crate::marks::load(&asset.root, &asset.id)
                        .unwrap()
                        .last_used_at
                        .is_some()
                    {
                        self.failure =
                            Some("Cancelled placement recorded an unused saved asset".into());
                        let _ = slint::quit_event_loop();
                        return;
                    }
                    println!("smoke: cancelled placement left saved asset usage unchanged");
                }
                println!("smoke: Esc cancelled pending placement without placing a mark");
                app.pending_mark = self.pending_mark_fixture.take();
                ui.set_mark_pending(true);
                self.stage = 40;
            }
            40 if ready => {
                let _ = ui.window().take_snapshot();
                let page = &app.layout.as_ref().unwrap().pages[0];
                let point = slint::LogicalPosition::new(
                    ui.get_viewport_window_x() + ui.get_scroll_x() + page.x + page.width * 0.6,
                    ui.get_viewport_window_y() + ui.get_scroll_y() + page.y + page.height * 0.7,
                );
                self.pending_selection = Some((point, point));
                self.stage = 32;
            }
            32 if !app.tabs[0].marks.items.is_empty() => {
                if let Some(asset) = &self.usage_fixture {
                    let stored = crate::marks::load(&asset.root, &asset.id).unwrap();
                    if stored.last_used_at.is_none() {
                        return;
                    }
                    if (stored.preferred_width_mm - 45.0).abs() > 0.001
                        || (stored.preferred_height_mm - 22.5).abs() > 0.001
                    {
                        self.failure = Some(
                            "Recording asset usage changed its preferred physical size".into(),
                        );
                        let _ = slint::quit_event_loop();
                        return;
                    }
                    println!(
                        "smoke: saved asset usage recorded on placement without changing its default size"
                    );
                }
                if std::env::var_os("DPDF_SMOKE_CLIPBOARD").is_some() {
                    let before = arboard::Clipboard::new()
                        .and_then(|mut clipboard| clipboard.get_text())
                        .unwrap();
                    app.share_file(ui, 1);
                    let after = arboard::Clipboard::new()
                        .and_then(|mut clipboard| clipboard.get_text())
                        .unwrap();
                    if before != after
                        || ui.get_share_ready()
                        || !ui.get_status().starts_with("Save a copy first")
                    {
                        self.failure = Some("Sharing unsaved signature drafts changed the clipboard or skipped its guard".into());
                        let _ = slint::quit_event_loop();
                        return;
                    }
                    println!("smoke: unsaved signature sharing refused without clipboard changes");
                }
                self.mark_geometry = Some(app.tabs[0].marks.items[0].placement.geometry);
                let _ = ui.window().take_snapshot();
                let tile = ui.get_mark_tiles().row_data(0).unwrap();
                let point = slint::LogicalPosition::new(
                    ui.get_viewport_window_x() + ui.get_scroll_x() + tile.px + tile.pw * 0.5,
                    ui.get_viewport_window_y() + ui.get_scroll_y() + tile.py + tile.ph * 0.5,
                );
                self.pending_selection = Some((
                    point,
                    slint::LogicalPosition::new(point.x + 25.0, point.y + 10.0),
                ));
                self.stage = 33;
            }
            33 if app.tabs[0].marks.items[0].placement.geometry != self.mark_geometry.unwrap() => {
                println!("smoke: dispatched page click and mark drag changed physical placement");
                self.mark_geometry = Some(app.tabs[0].marks.items[0].placement.geometry);
                let _ = ui.window().take_snapshot();
                let point = slint::LogicalPosition::new(
                    ui.get_mark_handle_window_x() + 6.0,
                    ui.get_mark_handle_window_y() + 6.0,
                );
                self.pending_selection = Some((
                    point,
                    slint::LogicalPosition::new(point.x + 20.0, point.y + 20.0),
                ));
                self.stage = 34;
            }
            34 if app.tabs[0].marks.items[0].placement.geometry.width_mm
                > self.mark_geometry.unwrap().width_mm + 0.1 =>
            {
                println!("smoke: dispatched corner drag resized the mark");
                self.mark_geometry = Some(app.tabs[0].marks.items[0].placement.geometry);
                ui.set_mark_lock(false);
                ui.set_mark_width("35".into());
                ui.set_mark_height("25".into());
                ui.set_mark_opacity("70".into());
                app.size_mark(ui, false);
                let mark = &app.tabs[0].marks.items[0];
                if (mark.placement.geometry.width_mm - 35.0).abs() > 0.001
                    || (mark.placement.opacity - 0.7).abs() > 0.001
                {
                    self.failure = Some("Numeric mark controls did not apply".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                self.pending_key = Some((true, "z".into()));
                self.stage = 41;
            }
            41 => {
                if app.tabs[0].marks.items[0].placement.geometry != self.mark_geometry.unwrap()
                    || app.tabs[0].marks.items[0].placement.opacity != 1.0
                    || !ui.get_can_redo_mark()
                {
                    self.failure = Some("Ctrl+Z did not undo the numeric placement edit".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                self.pending_key = Some((true, "Z".into()));
                self.pending_shift = true;
                self.stage = 42;
            }
            42 => {
                if (app.tabs[0].marks.items[0].placement.opacity - 0.7).abs() > 0.001 {
                    self.failure = Some("Ctrl+Shift+Z did not redo the placement edit".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                self.pending_key = Some((false, slint::platform::Key::Delete.into()));
                self.stage = 43;
            }
            43 => {
                if !app.tabs[0].marks.items.is_empty()
                    || !ui.get_can_undo_mark()
                    || ui.get_has_marks()
                {
                    self.failure = Some(
                        "Delete did not remove the selected mark with recoverable history".into(),
                    );
                    let _ = slint::quit_event_loop();
                    return;
                }
                self.pending_key = Some((true, "z".into()));
                self.stage = 44;
            }
            44 if !app.tabs[0].marks.items.is_empty() => {
                self.pending_key = Some((true, "y".into()));
                self.stage = 45;
            }
            45 => {
                if !app.tabs[0].marks.items.is_empty() {
                    self.failure = Some("Ctrl+Y did not redo mark removal".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                self.pending_key = Some((true, "z".into()));
                self.stage = 46;
            }
            46 if !app.tabs[0].marks.items.is_empty() => {
                println!(
                    "smoke: dispatched Undo/Redo/Delete restored physical placement and opacity"
                );
                let _ = ui.window().take_snapshot();
                let point = slint::LogicalPosition::new(
                    ui.get_mark_width_window_x() + 20.0,
                    ui.get_mark_width_window_y() + 12.0,
                );
                self.pending_selection = Some((point, point));
                self.pending_key = Some((true, "a".into()));
                self.stage = 50;
            }
            50 => {
                self.pending_key = Some((false, "9".into()));
                self.stage = 51;
            }
            51 => {
                if ui.get_mark_width().as_str() != "9" {
                    self.failure = Some("Numeric field did not receive text input".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                self.pending_key = Some((true, "z".into()));
                self.stage = 52;
            }
            52 => {
                // Slint records selected-text removal and replacement insertion separately.
                // First Undo removes the inserted character; the next restores the old text.
                if !ui.get_mark_width().is_empty()
                    || (app.tabs[0].marks.items[0].placement.opacity - 0.7).abs() > 0.001
                {
                    self.failure = Some("Text-field Undo changed document history".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                self.pending_key = Some((true, "z".into()));
                self.stage = 53;
            }
            53 => {
                if ui.get_mark_width().as_str() != "35.000"
                    || (app.tabs[0].marks.items[0].placement.opacity - 0.7).abs() > 0.001
                {
                    self.failure = Some(format!(
                        "Text-field Undo changed width to {:?}, draft opacity to {}",
                        ui.get_mark_width(),
                        app.tabs[0].marks.items[0].placement.opacity
                    ));
                    let _ = slint::quit_event_loop();
                    return;
                }
                println!("smoke: numeric text-field Undo retained native editing behavior");
                app.print(ui);
                app.export_pages(ui);
                if app.print_pending || app.export_pending {
                    self.failure = Some("Printing/page operations ignored unsaved marks".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                app.tabs[0].view.rotation = 1;
                app.relayout(ui, true);
                self.stage = 35;
            }
            35 if ready => {
                if std::env::var("DPDF_SMOKE_RECOVERY").as_deref() == Ok("write") {
                    if app.tabs[0].recovery.saved_generation == Some(app.tabs[0].marks.generation) {
                        println!(
                            "recovery: durable checkpoint written; simulating interrupted process"
                        );
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                        std::process::exit(0);
                    }
                    return;
                }
                if let Some(path) = std::env::var_os("DPDF_SMOKE_PLACEMENT_SNAPSHOT") {
                    let _ = save_snapshot(ui, &PathBuf::from(path));
                }
                let folder = self.second_document.as_ref().unwrap();
                app.open(ui, folder.path().join("second-tab.pdf"));
                self.stage = 36;
            }
            36 if ready && app.tabs.len() == 2 && app.active == 1 => {
                if ui.get_has_marks()
                    || ui.get_can_undo_mark()
                    || ui.get_can_redo_mark()
                    || ui.get_mark_tiles().row_count() != 0
                    || app.tabs[0].marks.items.len() != 1
                {
                    self.failure = Some("Draft marks leaked between tabs".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                app.activate(ui, 0);
                self.stage = 37;
            }
            37 if ready && app.active == 0 => {
                println!(
                    "smoke: mm/opacity controls, rotated preview, tab isolation and print/page guards passed"
                );
                let destination = self
                    .second_document
                    .as_ref()
                    .unwrap()
                    .path()
                    .join("missing-directory/output.pdf");
                app.save_marks_to(ui, destination);
                self.stage = 47;
            }
            47 if !app.export_pending => {
                if app.tabs.len() != 2
                    || app.tabs[0].marks.items.len() != 1
                    || !app.tabs[0].marks.can_undo()
                    || !ui.get_status().contains("failed")
                {
                    self.failure = Some("Failed save discarded drafts or history".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                println!("smoke: failed save retained drafts and history for retry");
                let destination = self
                    .second_document
                    .as_ref()
                    .unwrap()
                    .path()
                    .join("marked-copy-snapshot.pdf");
                app.save_marks_to(ui, destination);
                ui.set_mark_opacity("80".into());
                app.size_mark(ui, false);
                self.stage = 48;
            }
            48 if ready && app.tabs.len() == 3 && app.active == 2 && !app.export_pending => {
                if app.tabs[0].marks.items.len() != 1
                    || (app.tabs[0].marks.items[0].placement.opacity - 0.8).abs() > 0.001
                    || !app.tabs[0].marks.can_undo()
                {
                    self.failure = Some("Older export discarded newer draft edits/history".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                println!("smoke: older export retained edits made while saving");
                app.activate(ui, 0);
                app.history_mark(ui, false);
                self.stage = 49;
            }
            49 if ready && app.active == 0 => {
                if (app.tabs[0].marks.items[0].placement.opacity - 0.7).abs() > 0.001 {
                    self.failure = Some("Undo after an older export did not retain history".into());
                    let _ = slint::quit_event_loop();
                    return;
                }
                let destination = self
                    .second_document
                    .as_ref()
                    .unwrap()
                    .path()
                    .join("marked-copy.pdf");
                app.save_marks_to(ui, destination);
                self.stage = 38;
            }
            38 if ready && app.tabs.len() == 4 && app.active == 3 && !app.export_pending => {
                if !app.tabs[0].marks.items.is_empty()
                    || app.tabs[0].marks.can_undo()
                    || app.tabs[0].marks.can_redo()
                    || app.tabs[3].info.pages.len() != app.tabs[0].info.pages.len()
                {
                    self.failure = Some(
                        "Marked export lost pages or failed to clear saved drafts/history".into(),
                    );
                    let _ = slint::quit_event_loop();
                    return;
                }
                if let Some(path) = std::env::var_os("DPDF_SMOKE_PASSWORD_EDITED_OUTPUT") {
                    let source = self
                        .second_document
                        .as_ref()
                        .unwrap()
                        .path()
                        .join("marked-copy.pdf");
                    let path = PathBuf::from(path);
                    let result = crate::storage::ensure_separate_output(
                        &path,
                        &app.tabs
                            .iter()
                            .map(|t| t.info.path.as_path())
                            .collect::<Vec<_>>(),
                    )
                    .and_then(|_| Ok(std::fs::copy(source, path)?));
                    if let Err(error) = result {
                        self.failure = Some(format!("{error:#}"));
                        let _ = slint::quit_event_loop();
                        return;
                    }
                }
                if std::env::var_os("DPDF_SMOKE_CLIPBOARD").is_some() {
                    app.share_file(ui, 1);
                    let expected =
                        platform::saved_file_path(&app.tabs[app.active].info.path).unwrap();
                    let copied =
                        arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text());
                    if copied.ok().as_deref() != Some(expected.as_str()) || !ui.get_share_ready() {
                        self.failure =
                            Some("Saved marked copy was unavailable for file-path sharing".into());
                        let _ = slint::quit_event_loop();
                        return;
                    }
                    println!("smoke: exported marked copy is ready for sharing");
                }
                println!(
                    "smoke: worker exported marked copy and reopened it; source drafts cleared only after success"
                );
                self.stage = 5;
            }
            5 => {
                if let Some(path) = std::env::var_os("DPDF_SMOKE_SNAPSHOT") {
                    let result = save_snapshot(ui, &PathBuf::from(path));
                    if let Err(error) = result {
                        self.failure = Some(format!("Snapshot failed: {error:#}"));
                    }
                }
                println!("smoke: complete");
                let _ = slint::quit_event_loop();
                self.stage = 6;
            }
            _ => {}
        }
    }
}
