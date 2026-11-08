//! Generated real-window regression for reading-position preservation.
use super::*;
use anyhow::Context;

#[derive(Default)]
pub(super) struct Check {
    case: usize,
    phase: u8,
    anchor: Option<ViewportAnchor>,
    original_size: Option<slint::PhysicalSize>,
    previous_viewport: (f32, f32),
    zoom_checked: bool,
    scale_before_zoom: f32,
    pub(super) pending_key: Option<(bool, slint::SharedString)>,
    pub(super) pending_selection: Option<(slint::LogicalPosition, slint::LogicalPosition)>,
}

impl Check {
    pub(super) fn advance(
        &mut self,
        app: &mut Application,
        ui: &Workspace,
    ) -> anyhow::Result<bool> {
        if app.tabs.is_empty() || !app.pages.iter().any(|p| p.ready) {
            return Ok(false);
        }
        let mode = [ViewMode::Single, ViewMode::Continuous, ViewMode::Facing][self.case / 4];
        let zoom = if self.case % 4 < 2 {
            Zoom::Scale(2.0)
        } else {
            Zoom::FitWidth
        };
        let changed = self.previous_viewport != (ui.get_viewport_width(), ui.get_viewport_height());
        match self.phase {
            0 => {
                self.original_size.get_or_insert_with(|| ui.window().size());
                ui.window().set_size(self.original_size.unwrap());
                ui.set_sidebar_visible(true);
                ui.set_search_visible(false);
                ui.set_review_visible(false);
                let tab = &mut app.tabs[app.active];
                tab.view.page = 6;
                tab.view.mode = mode;
                tab.view.zoom = zoom;
                tab.view.rotation = (self.case % 2) as u8;
                ui.set_view_mode(match mode {
                    ViewMode::Single => 0,
                    ViewMode::Continuous => 1,
                    ViewMode::Facing => 2,
                });
                app.relayout(ui, true);
                self.phase = 1;
            }
            1 => {
                let _ = ui.window().take_snapshot()?;
                let layout = app.layout.as_ref().unwrap();
                let p = layout.pages.iter().find(|p| p.page == 6).unwrap();
                let point_x = p.x
                    + p.width * if self.case >= 8 { 1.0 } else { 0.65 }
                    + if self.case >= 8 { 8.0 } else { 0.0 };
                let point_y = p.y
                    + p.height * if self.case == 5 { 1.0 } else { 0.67 }
                    + if self.case == 5 { 8.0 } else { 0.0 };
                ui.set_scroll_x(
                    (ui.get_viewport_width() / 2.0 - point_x)
                        .clamp((ui.get_viewport_width() - layout.width).min(0.0), 0.0),
                );
                ui.set_scroll_y(
                    (ui.get_viewport_height() / 2.0 - point_y)
                        .clamp((ui.get_viewport_height() - layout.height).min(0.0), 0.0),
                );
                self.phase = 2;
            }
            2 if !self.zoom_checked => {
                self.remember(app, ui);
                self.scale_before_zoom = app.layout.as_ref().unwrap().scale;
                self.pending_key = Some((true, "+".into()));
                self.phase = 9;
            }
            9 if app.layout.as_ref().unwrap().scale > self.scale_before_zoom => {
                self.verify(app, ui, "Ctrl+plus zoom")?;
                self.remember(app, ui);
                self.scale_before_zoom = app.layout.as_ref().unwrap().scale;
                self.pending_key = Some((true, "-".into()));
                self.phase = 10;
            }
            10 if app.layout.as_ref().unwrap().scale < self.scale_before_zoom => {
                self.verify(app, ui, "Ctrl+minus zoom")?;
                println!(
                    "viewport: zoom case={} retained the reading point in both directions",
                    self.case
                );
                self.zoom_checked = true;
                app.tabs[app.active].view.zoom = zoom;
                app.relayout(ui, true);
                self.phase = 1;
            }
            2 => {
                self.remember(app, ui);
                ui.set_sidebar_visible(false);
                self.phase = 3;
            }
            3 if changed => {
                self.verify(app, ui, "sidebar")?;
                self.remember(app, ui);
                ui.set_search_visible(true);
                self.phase = 4;
            }
            4 if changed => {
                self.verify(app, ui, "search panel")?;
                self.remember(app, ui);
                let size = self.original_size.unwrap();
                ui.window().set_size(slint::PhysicalSize::new(
                    size.width + 180,
                    size.height.saturating_sub(80).max(500),
                ));
                self.phase = 5;
            }
            5 if changed => {
                self.verify(app, ui, "window resize")?;
                self.remember(app, ui);
                ui.window().set_size(self.original_size.unwrap());
                self.phase = 6;
            }
            6 if changed => {
                self.verify(app, ui, "window restore")?;
                self.remember(app, ui);
                ui.set_sidebar_visible(true);
                self.phase = 7;
            }
            7 if changed => {
                self.verify(app, ui, "sidebar restored")?;
                self.remember(app, ui);
                ui.set_search_visible(false);
                self.phase = 8;
            }
            8 if changed => {
                self.verify(app, ui, "search panel closed")?;
                println!(
                    "viewport: case={} mode={mode:?} zoom={zoom:?} rotation={} retained the reading point",
                    self.case,
                    self.case % 2
                );
                self.case += 1;
                self.zoom_checked = false;
                if self.case == 12 {
                    println!(
                        "viewport: sidebar, panels and physical resize preserve page-relative reading position in all cases"
                    );
                    self.case = 11;
                    self.phase = 11;
                    return Ok(false);
                }
                self.phase = 0;
            }
            11 | 15 => {
                if self.phase == 15 {
                    anyhow::ensure!(
                        (app.layout.as_ref().unwrap().scale - self.scale_before_zoom).abs()
                            < 0.00001
                            && ui.get_zoom_entry() == ui.get_zoom_label(),
                        "Invalid numeric zoom changed the view or was not reset"
                    );
                }
                let p = slint::LogicalPosition::new(
                    ui.get_zoom_field_window_x() + 20.0,
                    ui.get_zoom_field_window_y() + 10.0,
                );
                anyhow::ensure!(
                    p.x > 80.0 && p.y >= 44.0 && p.y < 82.0,
                    "Zoom input coordinates are outside the zoom toolbar: {p:?}"
                );
                self.pending_selection = Some((p, p));
                self.phase += 1;
            }
            12 | 16 => {
                self.pending_key = Some((true, "a".into()));
                self.phase += 1;
            }
            13 | 17 => {
                self.pending_key =
                    Some((false, if self.phase == 13 { "0%" } else { "175%" }.into()));
                self.phase += 1;
            }
            14 | 18 => {
                self.remember(app, ui);
                self.scale_before_zoom = app.layout.as_ref().unwrap().scale;
                self.pending_key = Some((false, slint::platform::Key::Return.into()));
                self.phase += 1;
            }
            19 => {
                anyhow::ensure!(
                    (app.layout.as_ref().unwrap().scale - 1.75).abs() < 0.00001
                        && ui.get_zoom_entry() == "175%",
                    "Numeric zoom did not apply the requested percentage"
                );
                self.verify(app, ui, "numeric zoom entry")?;
                println!(
                    "viewport: numeric zoom entry rejects an invalid value and preserves the reading point at 175%"
                );
                let p = slint::LogicalPosition::new(
                    ui.get_zoom_field_window_x() + 20.0,
                    ui.get_zoom_field_window_y() + 10.0,
                );
                self.pending_selection = Some((p, p));
                self.phase = 20;
            }
            20 | 22 => {
                ui.set_zoom_entry("250%".into());
                self.pending_key = Some((
                    false,
                    if self.phase == 20 {
                        slint::platform::Key::Escape
                    } else {
                        slint::platform::Key::Tab
                    }
                    .into(),
                ));
                self.phase += 1;
            }
            21 | 23 => {
                anyhow::ensure!(
                    (app.layout.as_ref().unwrap().scale - 1.75).abs() < 0.00001
                        && ui.get_zoom_entry() == "175%",
                    "Uncommitted zoom did not restore on Escape/focus loss"
                );
                if self.phase == 23 {
                    println!(
                        "viewport: uncommitted numeric zoom restores on Escape and focus loss"
                    );
                    return Ok(true);
                }
                let p = slint::LogicalPosition::new(
                    ui.get_zoom_field_window_x() + 20.0,
                    ui.get_zoom_field_window_y() + 10.0,
                );
                self.pending_selection = Some((p, p));
                self.phase = 22;
            }
            _ => {}
        }
        Ok(false)
    }

    fn remember(&mut self, app: &Application, ui: &Workspace) {
        self.previous_viewport = (ui.get_viewport_width(), ui.get_viewport_height());
        self.anchor = app.layout.as_ref().unwrap().viewport_anchor(
            (ui.get_viewport_width(), ui.get_viewport_height()),
            (ui.get_scroll_x(), ui.get_scroll_y()),
        );
    }

    fn verify(&self, app: &Application, ui: &Workspace, change: &str) -> anyhow::Result<()> {
        let anchor = self.anchor.unwrap();
        let layout = app.layout.as_ref().unwrap();
        let p = layout
            .pages
            .iter()
            .find(|p| p.page == anchor.page)
            .context("Reading-point page left the layout")?;
        let expected = (
            (anchor.screen_fraction.0 * ui.get_viewport_width()
                - p.x
                - anchor.page_point.0 * p.width)
                .clamp((ui.get_viewport_width() - layout.width).min(0.0), 0.0),
            (anchor.screen_fraction.1 * ui.get_viewport_height()
                - p.y
                - anchor.page_point.1 * p.height)
                .clamp((ui.get_viewport_height() - layout.height).min(0.0), 0.0),
        );
        if (ui.get_scroll_x() - expected.0).abs() > 1.5
            || (ui.get_scroll_y() - expected.1).abs() > 1.5
        {
            anyhow::bail!(
                "Reading point jumped during {change}, case={}: expected={expected:?}, actual=({}, {})",
                self.case,
                ui.get_scroll_x(),
                ui.get_scroll_y()
            );
        }
        Ok(())
    }
}
