//! Generated real-window coverage of asynchronous physical page facts.
use super::*;
use anyhow::{Context, ensure};

#[derive(Default)]
pub(super) struct Check {
    phase: u8,
    anchor: Option<ViewportAnchor>,
    second: Option<tempfile::TempDir>,
}

impl Check {
    pub(super) fn advance(
        &mut self,
        app: &mut Application,
        ui: &Workspace,
    ) -> anyhow::Result<bool> {
        let mode = std::env::var("DPDF_SMOKE_PHYSICAL").unwrap_or_default();
        if mode == "recovery-read" {
            if self.phase == 0 && ui.get_has_recovery() && !ui.get_recovery_busy() {
                app.recover_draft(ui, false);
                self.phase = 1;
            }
            if self.phase == 1
                && app.pages.iter().any(|page| page.ready)
                && let Some(tab) = app.tabs.iter().find(|tab| !tab.marks.items.is_empty())
            {
                ensure!(
                    tab.units_ready
                        && tab.raw_sizes[0].width == 306.0
                        && tab.info.pages[0].width == 612.0,
                    "Recovery did not verify physical dimensions first"
                );
                let mark = &tab.marks.items[0].placement.geometry;
                ensure!(
                    (mark.width_mm - 40.0).abs() < 0.01 && (mark.x_mm - 87.95).abs() < 0.01,
                    "Recovery changed the physical mark geometry"
                );
                ensure!(
                    mark.normalized(tab.raw_sizes[0].width, tab.raw_sizes[0].height)
                        .is_err(),
                    "Recovery fixture did not exceed raw coordinate bounds"
                );
                println!(
                    "physical: custom-unit recovery retains a mark beyond raw coordinate bounds"
                );
                if let Some(path) = std::env::var_os("DPDF_SMOKE_PHYSICAL_EXPORT") {
                    app.save_marks_to(ui, PathBuf::from(path));
                    self.phase = 2;
                    return Ok(false);
                }
                return Ok(true);
            }
            if self.phase == 2 && !app.export_pending {
                let output = PathBuf::from(std::env::var_os("DPDF_SMOKE_PHYSICAL_EXPORT").unwrap());
                ensure!(
                    output.is_file(),
                    "Recovered mark export failed: {}",
                    ui.get_status()
                );
                println!("physical: recovered custom-unit mark exports through the native worker");
                return Ok(true);
            }
            return Ok(false);
        }
        if self.phase == 3 {
            if let Some(tab) = app.tabs.first()
                && tab.recovery.saved_generation == Some(tab.draft_generation())
            {
                println!("physical: custom-unit mark checkpoint committed");
                return Ok(true);
            }
            return Ok(false);
        }
        if app.tabs.is_empty() || !app.pages.iter().any(|page| page.ready) {
            return Ok(false);
        }
        let rotated = mode == "rotated-crop";
        let dimensions = if rotated {
            (792.0, 612.0)
        } else {
            (612.0, 792.0)
        };
        match self.phase {
            0 => {
                // First frame is delivered before the controller requests metadata.
                ensure!(
                    !app.tabs[0].units_ready,
                    "Physical test missed the early-input window"
                );
                let rgba = image::RgbaImage::from_pixel(8, 4, image::Rgba([20, 40, 100, 255]));
                let preview = slint::Image::from_rgba8(
                    SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(rgba.as_raw(), 8, 4),
                );
                app.pending_mark = Some(crate::mark_studio::StudioMark {
                    document: app.tabs[0].info.id,
                    bytes: std::sync::Arc::new(crate::marks::png_ref(&rgba)?),
                    previews: [preview.clone(), preview.clone(), preview.clone(), preview],
                    size: crate::marks::PreferredSize {
                        width_mm: 40.0,
                        height_mm: 20.0,
                        aspect_locked: true,
                    },
                    asset_use: None,
                });
                app.place_mark(ui, 0, (0.5, 0.5));
                ensure!(
                    app.tabs[0].marks.items.is_empty() && app.pending_mark.is_some(),
                    "Early placement used unverified dimensions"
                );
                let tab = &mut app.tabs[0];
                tab.view.zoom = Zoom::Scale(1.0);
                tab.view.mode = ViewMode::Continuous;
                tab.view.page = 1;
                app.relayout(ui, false);
                let page = app
                    .layout
                    .as_ref()
                    .unwrap()
                    .pages
                    .iter()
                    .find(|page| page.page == 1)
                    .unwrap();
                ui.set_scroll_x(
                    (ui.get_viewport_width() / 2.0 - page.x - page.width / 2.0).min(0.0),
                );
                ui.set_scroll_y(
                    (ui.get_viewport_height() / 2.0 - page.y - page.height * 0.55).min(0.0),
                );
                self.anchor = app.layout.as_ref().unwrap().viewport_anchor(
                    (ui.get_viewport_width(), ui.get_viewport_height()),
                    (ui.get_scroll_x(), ui.get_scroll_y()),
                );
                self.phase = 1;
            }
            1 => {
                if mode == "missing-helper" || mode == "invalid-unit" {
                    if app.tabs[0].units_error.is_none() {
                        return Ok(false);
                    }
                    app.place_mark(ui, 0, (0.5, 0.5));
                    ensure!(
                        app.tabs[0].marks.items.is_empty() && !app.tabs[0].units_ready,
                        "Failed unit inspection allowed placement"
                    );
                    println!(
                        "physical: unavailable dimensions retain reader frames and refuse unverified placement"
                    );
                    return Ok(true);
                }
                if !app.tabs[0].units_ready {
                    return Ok(false);
                }
                ensure!(
                    app.tabs[0]
                        .info
                        .pages
                        .iter()
                        .all(|page| (page.width - dimensions.0).abs() < 0.01
                            && (page.height - dimensions.1).abs() < 0.01),
                    "Mixed-unit pages lost physical letter dimensions"
                );
                let layout = app.layout.as_ref().context("Missing physical layout")?;
                ensure!(
                    layout
                        .pages
                        .iter()
                        .all(|page| (page.width - dimensions.0 * 96.0 / 72.0).abs() < 0.1
                            && (page.height - dimensions.1 * 96.0 / 72.0).abs() < 0.1),
                    "100% layout did not use physical dimensions"
                );
                let (x, y) = layout
                    .restore_viewport_anchor(
                        self.anchor.context("Missing reading anchor")?,
                        (ui.get_viewport_width(), ui.get_viewport_height()),
                    )
                    .unwrap();
                ensure!(
                    (x - ui.get_scroll_x()).abs() < 1.0 && (y - ui.get_scroll_y()).abs() < 1.0,
                    "Metadata reflow moved the reading point"
                );
                if mode == "bad-review" {
                    ensure!(
                        app.tabs[0].review_error.is_some(),
                        "Malformed review fixture was not exercised"
                    );
                }
                let id = app.tabs[0].info.id;
                let generation = app.generation;
                app.page_units_loaded(ui, id, Ok(vec![2.0, 0.5, 1.0]));
                ensure!(
                    app.generation == generation,
                    "Repeated facts caused another reflow"
                );
                app.page_units_loaded(ui, id, Ok(vec![1.0]));
                ensure!(
                    app.tabs[0].units_error.is_some()
                        && app.tabs[0].info.pages[0].width == dimensions.0,
                    "Invalid fact count changed geometry"
                );
                app.page_units_loaded(ui, id, Ok(vec![2.0, 0.5, 1.0]));
                app.place_mark(ui, 0, (0.5, 0.5));
                let mark = &app.tabs[0].marks.items[0].placement.geometry;
                ensure!(
                    (mark.width_mm - 40.0).abs() < 0.01
                        && (mark.x_mm - (dimensions.0 * 25.4 / 72.0 / 2.0 - 20.0)).abs() < 0.01,
                    "Placed mark used raw coordinate dimensions"
                );
                if mode == "recovery-write" {
                    self.phase = 3;
                    return Ok(false);
                }
                app.tabs[0].marks = Default::default();
                for mode in [ViewMode::Single, ViewMode::Continuous, ViewMode::Facing] {
                    for rotation in [0, 1] {
                        app.tabs[0].view.mode = mode;
                        app.tabs[0].view.rotation = rotation;
                        app.tabs[0].view.zoom = Zoom::FitWidth;
                        app.relayout(ui, false);
                        let layout = app.layout.as_ref().unwrap();
                        let expected = if rotation == 0 {
                            dimensions.0 / dimensions.1
                        } else {
                            dimensions.1 / dimensions.0
                        };
                        ensure!(
                            layout
                                .pages
                                .iter()
                                .all(|page| (page.width / page.height - expected).abs() < 0.001),
                            "Fit layout lost physical aspect ratio"
                        );
                    }
                }
                println!(
                    "physical: mixed units, 100%, Fit modes, rotation/crop, reading anchor, repeated facts and mm placement passed"
                );
                let directory = tempfile::tempdir()?;
                let path = directory.path().join("Background physical fixture.pdf");
                std::fs::copy(&app.tabs[0].info.path, &path)?;
                self.second = Some(directory);
                app.open(ui, path);
                self.phase = 2;
            }
            2 if app.tabs.len() == 2 && app.tabs[1].units_ready => {
                app.activate(ui, 0);
                let id = app.tabs[1].info.id;
                let active = app.tabs[app.active].info.id;
                let generation = app.generation;
                app.page_units_loaded(ui, id, Ok(vec![1.0, 1.0, 1.0]));
                ensure!(
                    app.tabs[app.active].info.id == active && app.generation == generation,
                    "Background metadata changed the active tab"
                );
                app.page_units_loaded(ui, id, Ok(vec![2.0, 0.5, 1.0]));
                app.page_units_loaded(ui, u64::MAX, Ok(vec![2.0, 0.5, 1.0]));
                println!(
                    "physical: background and closed-document facts preserve active tab identity"
                );
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }
}
