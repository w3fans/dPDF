//! Explicit generated real-window tile diagnostics. Never opens private files or prints.
use super::*;

#[derive(Default)]
pub(super) struct Check {
    case: usize,
    round: usize,
    phase: u8,
    scroll: (f32, f32),
    peak_slots: usize,
    peak_bytes: usize,
    second_document: Option<tempfile::TempDir>,
    started: Option<Instant>,
    phase_started: Option<Instant>,
}

impl Check {
    fn page(&self) -> usize {
        6 + self.round * 10
    }
    fn case_number(&self) -> usize {
        self.round * 12 + self.case
    }

    fn bounded(&mut self, app: &Application) -> anyhow::Result<()> {
        use crate::raster_tiles::{DETAIL_BUDGET, MAX_PENDING_RENDERS, MAX_REGIONS};
        let slots = app.worker.render_requests_in_flight();
        let bytes: usize = app
            .detail_images
            .keys()
            .map(|key| key.region.unwrap().bytes())
            .sum();
        self.peak_slots = self.peak_slots.max(slots);
        self.peak_bytes = self.peak_bytes.max(bytes);
        anyhow::ensure!(
            slots <= MAX_PENDING_RENDERS,
            "Render admission exceeded eight slots"
        );
        anyhow::ensure!(
            bytes <= DETAIL_BUDGET && app.detail_keys.len() <= MAX_REGIONS,
            "Detail images exceeded their byte/metadata budget"
        );
        anyhow::ensure!(
            app.detail_tiles.row_count() == app.detail_keys.len(),
            "Tile model/key mismatch"
        );
        for (key, image) in &app.detail_images {
            let region = key.region.unwrap();
            anyhow::ensure!(
                app.detail_keys.contains(key) && region.valid_for(key.width as u32),
                "Stale or invalid detail image retained"
            );
            anyhow::ensure!(
                image.size().width == region.width && image.size().height == region.height,
                "Detail bitmap dimensions differ from the requested bounded region"
            );
            anyhow::ensure!(
                app.tabs
                    .get(app.active)
                    .is_some_and(|tab| tab.info.id == key.document),
                "Detail image belongs to the wrong active document"
            );
        }
        Ok(())
    }

    fn verify_landmarks(&self, app: &Application, ui: &Workspace) -> anyhow::Result<()> {
        let snapshot = ui.window().take_snapshot()?;
        let mut observations = [(0usize, 0.0f64, 0.0f64); 2];
        for (index, pixel) in snapshot.as_slice().iter().enumerate() {
            let color = if pixel.r > 240 && pixel.g < 16 && pixel.b > 240 {
                Some(0)
            } else if pixel.r < 16 && pixel.g > 240 && pixel.b > 240 {
                Some(1)
            } else {
                None
            };
            if let Some(color) = color {
                let value = &mut observations[color];
                value.0 += 1;
                value.1 += (index % snapshot.width() as usize) as f64 + 0.5;
                value.2 += (index / snapshot.width() as usize) as f64 + 0.5;
            }
        }
        let page = app
            .layout
            .as_ref()
            .unwrap()
            .pages
            .iter()
            .find(|p| p.page == self.page())
            .unwrap();
        let scale = ui.window().scale_factor() as f64;
        let mut maximum = 0.0f64;
        for (color, (fx, fy)) in [(0.51f32, 0.51f32), (0.49, 0.49)].into_iter().enumerate() {
            let (fx, fy) = match self.case % 4 {
                1 => (1.0 - fy, fx),
                2 => (1.0 - fx, 1.0 - fy),
                3 => (fy, 1.0 - fx),
                _ => (fx, fy),
            };
            let expected = (
                (ui.get_viewport_window_x() + ui.get_scroll_x() + page.x + fx * page.width) as f64
                    * scale,
                (ui.get_viewport_window_y() + ui.get_scroll_y() + page.y + fy * page.height) as f64
                    * scale,
            );
            let (count, x, y) = observations[color];
            anyhow::ensure!(
                count >= 16,
                "Native PDF landmark {color} absent, case={}",
                self.case
            );
            let actual = (x / count as f64, y / count as f64);
            let error = (actual.0 - expected.0)
                .abs()
                .max((actual.1 - expected.1).abs());
            maximum = maximum.max(error);
            anyhow::ensure!(
                error <= 2.0,
                "Native PDF landmark {color} moved, case={}: expected={expected:?}, actual={actual:?}, axis_error={error}",
                self.case
            );
        }
        println!(
            "raster: landmarks case={} max_axis_error_pixels={maximum:.3}",
            self.case_number()
        );
        Ok(())
    }

    fn ready(app: &Application) -> bool {
        !app.detail_keys.is_empty()
            && app.detail_tiles.iter().all(|tile| tile.ready)
            && app.detail_images.len() == app.detail_keys.len()
    }

    pub(super) fn advance(
        &mut self,
        app: &mut Application,
        ui: &Workspace,
    ) -> anyhow::Result<bool> {
        let started = *self.started.get_or_insert_with(Instant::now);
        self.bounded(app)?;
        let rounds = std::env::var("DPDF_SMOKE_RASTER_ROUNDS")
            .ok()
            .map(|value| value.parse::<usize>())
            .transpose()?
            .unwrap_or(1);
        anyhow::ensure!(
            (1..=3).contains(&rounds),
            "Raster diagnostic rounds exceed their budget"
        );
        if self.case == 12 && self.round + 1 < rounds {
            self.round += 1;
            self.case = 0;
            self.phase = 0;
        }
        if self.case == 12 {
            match self.phase {
                0 => {
                    let folder = tempfile::tempdir()?;
                    let path = folder.path().join("Second generated raster fixture.pdf");
                    std::fs::copy(&app.tabs[0].info.path, &path)?;
                    app.open(ui, path);
                    self.second_document = Some(folder);
                    self.phase = 1;
                }
                1 if app.tabs.len() == 2 && app.pages.iter().any(|page| page.ready) => {
                    app.tabs[app.active].view.zoom = Zoom::Scale(6.0);
                    app.relayout(ui, true);
                    app.request_visible(ui);
                    app.activate(ui, 0);
                    app.request_visible(ui);
                    app.activate(ui, 1);
                    app.request_visible(ui);
                    self.phase = 2;
                }
                2 if Self::ready(app) => {
                    app.close(ui, 1);
                    app.request_visible(ui);
                    self.phase = 3;
                }
                3 if Self::ready(app) => {
                    app.close(ui, 0);
                    self.phase = 4;
                }
                4 if app.worker.render_requests_in_flight() == 0 => {
                    anyhow::ensure!(
                        app.detail_images.is_empty()
                            && app.detail_keys.is_empty()
                            && app.detail_tiles.row_count() == 0,
                        "Closing all tabs retained detail images"
                    );
                    println!(
                        "raster: summary peak_render_slots={} peak_detail_bytes={} cases={} tab_switch_close=true",
                        self.peak_slots,
                        self.peak_bytes,
                        rounds * 12
                    );
                    return Ok(true);
                }
                _ => {}
            }
            return Ok(false);
        }
        if app.tabs.is_empty() || !app.pages.iter().any(|page| page.ready) {
            return Ok(false);
        }
        match self.phase {
            0 => {
                if self.case == 0 && self.round == 0 {
                    let _ = ui.window().take_snapshot()?;
                    println!(
                        "raster: initial_preview_presented_ms={:.3}",
                        started.elapsed().as_secs_f64() * 1000.0
                    );
                }
                self.phase_started = Some(Instant::now());
                let mode =
                    [ViewMode::Single, ViewMode::Continuous, ViewMode::Facing][self.case / 4];
                let tab = &mut app.tabs[app.active];
                tab.view.page = self.page();
                tab.view.mode = mode;
                tab.view.zoom = Zoom::Scale(6.0);
                tab.view.rotation = (self.case % 4) as u8;
                ui.set_view_mode(match mode {
                    ViewMode::Single => 0,
                    ViewMode::Continuous => 1,
                    ViewMode::Facing => 2,
                });
                app.relayout(ui, true);
                self.phase = 1;
            }
            1 if Self::ready(app) => {
                let _ = ui.window().take_snapshot()?;
                println!(
                    "raster: detail_presented case={} ms={:.3}",
                    self.case,
                    self.phase_started.unwrap().elapsed().as_secs_f64() * 1000.0
                );
                let layout = app.layout.as_ref().unwrap();
                let p = layout.pages.iter().find(|p| p.page == self.page()).unwrap();
                ui.set_scroll_x(
                    (ui.get_viewport_width() / 2.0 - p.x - p.width * 0.51)
                        .clamp((ui.get_viewport_width() - layout.width).min(0.0), 0.0),
                );
                ui.set_scroll_y(
                    (ui.get_viewport_height() / 2.0 - p.y - p.height * 0.51)
                        .clamp((ui.get_viewport_height() - layout.height).min(0.0), 0.0),
                );
                self.scroll = (ui.get_scroll_x(), ui.get_scroll_y());
                self.phase_started = Some(Instant::now());
                app.request_visible(ui);
                self.phase = 2;
            }
            2 if Self::ready(app) => {
                anyhow::ensure!(
                    (ui.get_scroll_x() - self.scroll.0).abs() < 1.5
                        && (ui.get_scroll_y() - self.scroll.1).abs() < 1.5,
                    "Detail delivery moved the viewport"
                );
                self.verify_landmarks(app, ui)?;
                println!(
                    "raster: pan_presented case={} ms={:.3}",
                    self.case,
                    self.phase_started.unwrap().elapsed().as_secs_f64() * 1000.0
                );
                self.phase_started = Some(Instant::now());
                // Do not drain replies between generations: permits must survive cancellation.
                for zoom in [4.0, 5.5, 6.0] {
                    app.tabs[app.active].view.zoom = Zoom::Scale(zoom);
                    app.relayout(ui, true);
                    app.request_visible(ui);
                    self.bounded(app)?;
                }
                self.phase = 3;
            }
            3 if Self::ready(app) => {
                let _ = ui.window().take_snapshot()?;
                println!(
                    "raster: case={} rotation={} detail_images={} render_slots={}",
                    self.case_number(),
                    self.case % 4,
                    app.detail_images.len(),
                    app.worker.render_requests_in_flight()
                );
                self.case += 1;
                self.phase = 0;
            }
            _ => {}
        }
        Ok(false)
    }
}
