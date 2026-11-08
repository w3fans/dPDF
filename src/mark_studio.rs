//! A small asynchronous image cleanup/library window. No PDFium work happens here.
use crate::{
    MarkStudioWindow,
    marks::{self, Asset, Cleanup, Crop, ImportOptions, MarkKind, MarkSource, PreferredSize},
};
use anyhow::{Context, Result, bail};
use image::RgbaImage;
use slint::{ComponentHandle, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

enum Reply {
    Library(marks::LibraryScan),
    Opened {
        source: Arc<MarkSource>,
        asset: Option<Asset>,
        name: String,
        preview: RgbaImage,
    },
    Preview {
        settings: Cleanup,
        pixels: (u32, u32),
        preview: RgbaImage,
    },
    Saved(Asset),
    Placement {
        document: u64,
        bytes: Vec<u8>,
        preview: RgbaImage,
        size: PreferredSize,
        asset_use: Option<marks::AssetUse>,
    },
}

#[derive(Clone)]
pub struct StudioMark {
    pub document: u64,
    pub bytes: Arc<Vec<u8>>,
    pub previews: [slint::Image; 4],
    pub size: PreferredSize,
    pub asset_use: Option<marks::AssetUse>,
}

struct State {
    document: Option<u64>,
    placement: Option<StudioMark>,
    root: PathBuf,
    assets: Vec<Asset>,
    source: Option<Arc<MarkSource>>,
    asset: Option<Asset>,
    preview_settings: Option<Cleanup>,
    attempted: Option<Cleanup>,
    observed: Option<Cleanup>,
    pixels: Option<(u32, u32)>,
    changed: Instant,
    last_width: String,
    last_height: String,
    generation: u64,
    in_flight: bool,
    closed: bool,
    refresh_needed: bool,
    sender: mpsc::SyncSender<(u64, Result<Reply, String>)>,
    receiver: mpsc::Receiver<(u64, Result<Reply, String>)>,
}

pub struct StudioSession {
    ui: MarkStudioWindow,
    state: Rc<RefCell<State>>,
    _timer: slint::Timer,
}

fn image(image: RgbaImage) -> slint::Image {
    slint::Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        image.as_raw(),
        image.width(),
        image.height(),
    ))
}

impl State {
    fn job(
        &mut self,
        ui: &MarkStudioWindow,
        action: impl FnOnce() -> Result<Reply> + Send + 'static,
    ) {
        if self.in_flight {
            return;
        }
        self.in_flight = true;
        self.generation += 1;
        let token = self.generation;
        let sender = self.sender.clone();
        ui.set_busy(true);
        ui.set_can_save(false);
        std::thread::spawn(move || {
            let _ = sender.send((token, action().map_err(|e| format!("{e:#}"))));
        });
    }

    fn refresh(&mut self, ui: &MarkStudioWindow) {
        self.refresh_needed = true;
        if self.in_flight {
            return;
        }
        self.refresh_needed = false;
        let root = self.root.clone();
        ui.set_status("Loading library…".into());
        self.job(ui, move || Ok(Reply::Library(marks::scan(&root)?)));
    }

    fn names(&self, ui: &MarkStudioWindow) {
        ui.set_assets(ModelRc::new(VecModel::from(
            self.assets
                .iter()
                .map(|a| a.name.as_str().into())
                .collect::<Vec<_>>(),
        )));
    }

    fn open(&mut self, ui: &MarkStudioWindow, path: PathBuf, asset: Option<Asset>) {
        if self.in_flight {
            return;
        }
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        ui.set_status("Reading image…".into());
        self.job(ui, move || {
            let source = Arc::new(marks::read_image(&path)?);
            let preview = source.image.thumbnail(1000, 700).to_rgba8();
            Ok(Reply::Opened {
                source,
                asset,
                name,
                preview,
            })
        });
    }

    fn paste(&mut self, ui: &MarkStudioWindow) {
        if self.in_flight {
            return;
        }
        ui.set_status("Reading clipboard image…".into());
        self.job(ui, move || {
            let image = arboard::Clipboard::new()?
                .get_image()
                .context("Clipboard does not contain an available image")?;
            let source = Arc::new(marks::source_from_rgba(
                image.width.try_into()?,
                image.height.try_into()?,
                image.bytes.into_owned(),
            )?);
            let preview = source.image.thumbnail(1000, 700).to_rgba8();
            Ok(Reply::Opened {
                source,
                asset: None,
                name: "Clipboard image".into(),
                preview,
            })
        });
    }

    fn reset_crop(&self, ui: &MarkStudioWindow) {
        if let Some(source) = &self.source {
            ui.set_crop_x("0".into());
            ui.set_crop_y("0".into());
            ui.set_crop_width(source.image.width().to_string().into());
            ui.set_crop_height(source.image.height().to_string().into());
        }
    }

    fn settings(&self, ui: &MarkStudioWindow) -> Result<Cleanup> {
        let source = self.source.as_ref().context("Import an image first")?;
        let crop = Crop {
            x: ui
                .get_crop_x()
                .trim()
                .parse()
                .context("Crop X must be a pixel number")?,
            y: ui
                .get_crop_y()
                .trim()
                .parse()
                .context("Crop Y must be a pixel number")?,
            width: ui
                .get_crop_width()
                .trim()
                .parse()
                .context("Crop width must be a pixel number")?,
            height: ui
                .get_crop_height()
                .trim()
                .parse()
                .context("Crop height must be a pixel number")?,
        };
        if crop.width == 0
            || crop.height == 0
            || crop
                .x
                .checked_add(crop.width)
                .is_none_or(|v| v > source.image.width())
            || crop
                .y
                .checked_add(crop.height)
                .is_none_or(|v| v > source.image.height())
        {
            bail!("Crop must lie inside the source image");
        }
        let full = crop.x == 0
            && crop.y == 0
            && crop.width == source.image.width()
            && crop.height == source.image.height();
        Ok(Cleanup {
            crop: (!full).then_some(crop),
            deskew_degrees: ui.get_angle(),
            remove_white: ui.get_remove_white(),
            white_level: ui.get_white_level().round().clamp(0.0, 255.0) as u8,
            black_level: ui.get_black_level().round().clamp(0.0, 255.0) as u8,
        })
    }

    fn preferred(ui: &MarkStudioWindow) -> Result<PreferredSize> {
        let size = PreferredSize {
            width_mm: ui
                .get_width_mm()
                .trim()
                .parse()
                .context("Enter width in millimetres")?,
            height_mm: ui
                .get_height_mm()
                .trim()
                .parse()
                .context("Enter height in millimetres")?,
            aspect_locked: ui.get_aspect_locked(),
        };
        if !size.width_mm.is_finite()
            || !size.height_mm.is_finite()
            || !(0.1..=300.0).contains(&size.width_mm)
            || !(0.1..=300.0).contains(&size.height_mm)
        {
            bail!("Mark dimensions must be between 0.1 and 300 mm");
        }
        Ok(size)
    }

    fn sync_size(&mut self, ui: &MarkStudioWindow) {
        if ui.get_aspect_locked()
            && let Some((width, height)) = self.pixels
        {
            if ui.get_width_mm().as_str() != self.last_width {
                if let Ok(value) = ui.get_width_mm().trim().parse::<f32>()
                    && value.is_finite()
                {
                    ui.set_height_mm(format!("{:.3}", value * height as f32 / width as f32).into());
                }
            } else if ui.get_height_mm().as_str() != self.last_height
                && let Ok(value) = ui.get_height_mm().trim().parse::<f32>()
                && value.is_finite()
            {
                ui.set_width_mm(format!("{:.3}", value * width as f32 / height as f32).into());
            }
        }
        self.last_width = ui.get_width_mm().to_string();
        self.last_height = ui.get_height_mm().to_string();
    }

    fn crop(&self, ui: &MarkStudioWindow, points: [f32; 4]) {
        let Some(source) = &self.source else {
            return;
        };
        if !points.iter().all(|p| p.is_finite()) {
            return;
        }
        let [x0, y0, x1, y1] = points.map(|p| p.clamp(0.0, 1.0));
        let width = source.image.width();
        let height = source.image.height();
        let left = (x0.min(x1) * width as f32).floor() as u32;
        let top = (y0.min(y1) * height as f32).floor() as u32;
        let right = (x0.max(x1) * width as f32).ceil().min(width as f32) as u32;
        let bottom = (y0.max(y1) * height as f32).ceil().min(height as f32) as u32;
        if right <= left || bottom <= top {
            return;
        }
        ui.set_crop_x(left.to_string().into());
        ui.set_crop_y(top.to_string().into());
        ui.set_crop_width((right - left).to_string().into());
        ui.set_crop_height((bottom - top).to_string().into());
    }

    fn save(&mut self, ui: &MarkStudioWindow, size_only: bool) {
        if self.in_flight {
            return;
        }
        self.sync_size(ui);
        let request = (|| -> Result<_> {
            let source = self.source.clone().context("Import an image first")?;
            let settings = self.settings(ui)?;
            if self.preview_settings.as_ref() != Some(&settings) {
                bail!("Wait for the current cleanup preview");
            }
            let size = Self::preferred(ui)?;
            Ok((source, settings, size))
        })();
        let (source, settings, preferred) = match request {
            Ok(request) => request,
            Err(error) => {
                ui.set_status(error.to_string().into());
                return;
            }
        };
        let root = self.root.clone();
        if size_only {
            let Some(asset) = &self.asset else {
                return;
            };
            if asset.cleanup != settings {
                ui.set_status("Save a new asset to keep the changed cleanup".into());
                return;
            }
            let id = asset.id.clone();
            self.job(ui, move || {
                Ok(Reply::Saved(marks::save_size(
                    &root,
                    &id,
                    preferred.width_mm,
                    preferred.height_mm,
                    preferred.aspect_locked,
                )?))
            });
        } else {
            let options = ImportOptions {
                name: ui.get_asset_name().to_string(),
                notes: ui.get_notes().to_string(),
                kind: match ui.get_kind() {
                    1 => MarkKind::Stamp,
                    2 => MarkKind::Initials,
                    3 => MarkKind::Other,
                    _ => MarkKind::Signature,
                },
                preferred_size: Some(preferred),
                cleanup: settings,
            };
            self.job(ui, move || {
                Ok(Reply::Saved(marks::import_snapshot(
                    &root, &source, options,
                )?))
            });
        }
        ui.set_status("Saving local asset…".into());
    }

    fn place(&mut self, ui: &MarkStudioWindow) {
        self.sync_size(ui);
        let request = (|| -> Result<_> {
            let document = self.document.context("Open a PDF first")?;
            let source = self.source.clone().context("Import an image first")?;
            let settings = self.settings(ui)?;
            if self.preview_settings.as_ref() != Some(&settings) {
                bail!("Wait for the cleanup preview");
            }
            let asset_use = self.asset.as_ref().map(|asset| marks::AssetUse {
                root: self.root.clone(),
                id: asset.id.clone(),
            });
            Ok((document, source, settings, Self::preferred(ui)?, asset_use))
        })();
        match request {
            Ok((document, source, settings, size, asset_use)) => self.job(ui, move || {
                let cleaned = marks::cleanup(&source.image, &settings)?;
                let bytes = marks::png_ref(&cleaned)?;
                if bytes.len() as u64 > crate::imaging::MAX_INPUT {
                    bail!("Cleaned mark exceeds 64 MiB");
                }
                let preview = image::imageops::thumbnail(&cleaned, 400, 400);
                Ok(Reply::Placement {
                    document,
                    bytes,
                    preview,
                    size,
                    asset_use,
                })
            }),
            Err(error) => ui.set_status(error.to_string().into()),
        }
    }

    fn tick(&mut self, ui: &MarkStudioWindow) {
        if let Ok((token, result)) = self.receiver.try_recv() {
            self.in_flight = false;
            ui.set_busy(false);
            if token == self.generation && !self.closed {
                match result {
                    Ok(Reply::Placement {
                        document,
                        bytes,
                        preview,
                        size,
                        asset_use,
                    }) => {
                        let previews = [
                            image(preview.clone()),
                            image(image::imageops::rotate90(&preview)),
                            image(image::imageops::rotate180(&preview)),
                            image(image::imageops::rotate270(&preview)),
                        ];
                        self.placement = Some(StudioMark {
                            document,
                            bytes: Arc::new(bytes),
                            previews,
                            size,
                            asset_use,
                        });
                        ui.set_status("Click the document to place this mark".into());
                    }
                    Ok(Reply::Library(scan)) => {
                        self.assets = scan.assets;
                        self.names(ui);
                        ui.set_status(if scan.warnings.is_empty() {
                            "Choose a saved mark or import an image".into()
                        } else {
                            format!(
                                "Loaded {} marks; {} unavailable items were left unchanged",
                                self.assets.len(),
                                scan.warnings.len()
                            )
                            .into()
                        });
                    }
                    Ok(Reply::Opened {
                        source,
                        asset,
                        name,
                        preview,
                    }) => {
                        ui.set_original(image(preview));
                        ui.set_cleaned(Default::default());
                        ui.set_source_label(
                            format!(
                                "{} × {} source px · {}",
                                source.image.width(),
                                source.image.height(),
                                source.format
                            )
                            .into(),
                        );
                        ui.set_asset_name(
                            asset
                                .as_ref()
                                .map(|a| a.name.as_str())
                                .unwrap_or(&name)
                                .into(),
                        );
                        ui.set_notes(
                            asset
                                .as_ref()
                                .map(|a| a.notes.as_str())
                                .unwrap_or("")
                                .into(),
                        );
                        ui.set_kind(
                            match asset
                                .as_ref()
                                .map(|a| a.kind)
                                .unwrap_or(MarkKind::Signature)
                            {
                                MarkKind::Signature => 0,
                                MarkKind::Stamp => 1,
                                MarkKind::Initials => 2,
                                MarkKind::Other => 3,
                            },
                        );
                        let settings = asset
                            .as_ref()
                            .map(|a| a.cleanup.clone())
                            .unwrap_or_default();
                        ui.set_angle(settings.deskew_degrees);
                        ui.set_remove_white(settings.remove_white);
                        ui.set_white_level(settings.white_level as f32);
                        ui.set_black_level(settings.black_level as f32);
                        ui.set_width_mm(
                            asset
                                .as_ref()
                                .map(|a| a.preferred_width_mm)
                                .unwrap_or(45.0)
                                .to_string()
                                .into(),
                        );
                        ui.set_height_mm(
                            asset
                                .as_ref()
                                .map(|a| a.preferred_height_mm)
                                .unwrap_or(20.0)
                                .to_string()
                                .into(),
                        );
                        ui.set_aspect_locked(asset.as_ref().is_none_or(|a| a.aspect_locked));
                        self.source = Some(source);
                        self.asset = asset;
                        self.reset_crop(ui);
                        if let Some(crop) = settings.crop {
                            ui.set_crop_x(crop.x.to_string().into());
                            ui.set_crop_y(crop.y.to_string().into());
                            ui.set_crop_width(crop.width.to_string().into());
                            ui.set_crop_height(crop.height.to_string().into());
                        }
                        self.preview_settings = None;
                        self.attempted = None;
                        self.observed = None;
                        self.pixels = None;
                        self.last_width.clear();
                        self.last_height.clear();
                        ui.set_has_source(true);
                    }
                    Ok(Reply::Preview {
                        settings,
                        pixels,
                        preview,
                    }) => {
                        ui.set_cleaned(image(preview));
                        ui.set_result_label(
                            format!("{} × {} cleaned px", pixels.0, pixels.1).into(),
                        );
                        self.preview_settings = Some(settings);
                        self.pixels = Some(pixels);
                        if ui.get_aspect_locked()
                            && let Ok(width) = ui.get_width_mm().trim().parse::<f32>()
                        {
                            ui.set_height_mm(
                                format!("{:.3}", width * pixels.1 as f32 / pixels.0 as f32).into(),
                            );
                        }
                        self.last_width = ui.get_width_mm().to_string();
                        self.last_height = ui.get_height_mm().to_string();
                        ui.set_status("Preview ready — dimensions are printed millimetres".into());
                    }
                    Ok(Reply::Saved(asset)) => {
                        self.assets.retain(|a| a.id != asset.id);
                        self.assets.push(asset.clone());
                        self.assets.sort_by_key(|a| a.name.to_lowercase());
                        self.names(ui);
                        ui.set_selected_asset(
                            self.assets
                                .iter()
                                .position(|a| a.id == asset.id)
                                .unwrap_or(0) as i32,
                        );
                        ui.set_status(format!("Saved {} to the local library", asset.name).into());
                        self.asset = Some(asset);
                    }
                    Err(message) => ui.set_status(message.into()),
                }
            }
        }
        if self.closed {
            return;
        }
        if self.refresh_needed && !self.in_flight {
            self.refresh(ui);
        }
        self.sync_size(ui);
        let settings = match self.settings(ui) {
            Ok(settings) => settings,
            Err(error) => {
                ui.set_can_save(false);
                if self.source.is_some() {
                    ui.set_status(error.to_string().into());
                }
                return;
            }
        };
        if self.observed.as_ref() != Some(&settings) {
            self.observed = Some(settings.clone());
            self.changed = Instant::now();
        }
        if let Some(source) = &self.source {
            let crop = settings.crop.unwrap_or(Crop {
                x: 0,
                y: 0,
                width: source.image.width(),
                height: source.image.height(),
            });
            ui.set_crop_left(crop.x as f32 / source.image.width() as f32);
            ui.set_crop_top(crop.y as f32 / source.image.height() as f32);
            ui.set_crop_right((crop.x + crop.width) as f32 / source.image.width() as f32);
            ui.set_crop_bottom((crop.y + crop.height) as f32 / source.image.height() as f32);
        }
        let ready = self.preview_settings.as_ref() == Some(&settings);
        let preferred = Self::preferred(ui);
        ui.set_can_save(ready && !self.in_flight && preferred.is_ok());
        if ready
            && !self.in_flight
            && let Err(error) = preferred
        {
            ui.set_status(error.to_string().into());
        }
        ui.set_saved_item(self.asset.as_ref().is_some_and(|a| a.cleanup == settings));
        if !ready
            && !self.in_flight
            && self.attempted.as_ref() != Some(&settings)
            && self.changed.elapsed() >= Duration::from_millis(150)
        {
            self.attempted = Some(settings.clone());
            let source = self.source.clone().expect("settings require a source");
            ui.set_status("Updating preview…".into());
            self.job(ui, move || {
                let cleaned = marks::cleanup(&source.image, &settings)?;
                let pixels = cleaned.dimensions();
                let preview = image::DynamicImage::ImageRgba8(cleaned)
                    .thumbnail(1000, 700)
                    .to_rgba8();
                Ok(Reply::Preview {
                    settings,
                    pixels,
                    preview,
                })
            });
        }
    }
}

impl StudioSession {
    pub fn new(dark: bool) -> Result<Self> {
        let ui = MarkStudioWindow::new()?;
        ui.set_dark(dark);
        let root =
            marks::default_root().context("No local application-data directory is configured")?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let state = Rc::new(RefCell::new(State {
            document: None,
            placement: None,
            root,
            assets: vec![],
            source: None,
            asset: None,
            preview_settings: None,
            attempted: None,
            observed: None,
            pixels: None,
            changed: Instant::now(),
            last_width: String::new(),
            last_height: String::new(),
            generation: 0,
            in_flight: false,
            closed: true,
            refresh_needed: true,
            sender,
            receiver,
        }));
        macro_rules! callback { ($name:ident, |$state:ident,$window:ident $(,$arg:ident)*| $body:block) => {{
            let state=state.clone(); let weak=ui.as_weak(); ui.$name(move |$($arg),*| { if let Some($window)=weak.upgrade() { #[allow(unused_mut)] let mut $state=state.borrow_mut(); $body } });
        }}; }
        callback!(on_import_image, |state, window| {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Signature/stamp image", &["png", "jpg", "jpeg"])
                .pick_file()
            {
                state.open(&window, path, None);
            }
        });
        callback!(on_paste_image, |state, window| {
            state.paste(&window);
        });
        callback!(on_choose_asset, |state, window, index| {
            if let Some(asset) = state.assets.get(index.max(0) as usize).cloned() {
                let path = state.root.join(&asset.id).join("original");
                state.open(&window, path, Some(asset));
            }
        });
        callback!(on_refresh_library, |state, window| {
            state.refresh(&window);
        });
        callback!(on_reset_crop, |state, window| {
            state.reset_crop(&window);
        });
        callback!(on_crop, |state, window, x0, y0, x1, y1| {
            state.crop(&window, [x0, y0, x1, y1]);
        });
        callback!(on_save_asset, |state, window| {
            state.save(&window, false);
        });
        callback!(on_save_size, |state, window| {
            state.save(&window, true);
        });
        callback!(on_place_mark, |state, window| {
            state.place(&window);
        });
        callback!(on_close_studio, |state, window| {
            state.clear(&window);
            let _ = window.hide();
        });
        {
            let state = state.clone();
            let weak = ui.as_weak();
            ui.window().on_close_requested(move || {
                if let Some(ui) = weak.upgrade() {
                    state.borrow_mut().clear(&ui);
                }
                slint::CloseRequestResponse::HideWindow
            });
        }
        let timer = slint::Timer::default();
        let weak = ui.as_weak();
        let timer_state = state.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(40),
            move || {
                if let Some(ui) = weak.upgrade() {
                    timer_state.borrow_mut().tick(&ui);
                }
            },
        );
        Ok(Self {
            ui,
            state,
            _timer: timer,
        })
    }

    pub fn show(&self, dark: bool) -> Result<()> {
        self.ui.set_dark(dark);
        self.state.borrow_mut().closed = false;
        self.state.borrow_mut().refresh_needed = true;
        self.ui.show()?;
        Ok(())
    }

    pub fn set_document(&self, document: Option<u64>) {
        self.state.borrow_mut().document = document;
        self.ui.set_can_place(document.is_some());
    }

    pub fn take_mark(&self) -> Option<StudioMark> {
        let mark = self.state.borrow_mut().placement.take();
        if mark.is_some() {
            let _ = self.hide();
        }
        mark
    }

    pub fn hide(&self) -> Result<()> {
        self.state.borrow_mut().clear(&self.ui);
        self.ui.hide()?;
        Ok(())
    }
}

impl State {
    fn clear(&mut self, ui: &MarkStudioWindow) {
        self.generation += 1;
        self.closed = true;
        self.source = None;
        self.asset = None;
        self.preview_settings = None;
        self.pixels = None;
        self.placement = None;
        ui.set_original(Default::default());
        ui.set_cleaned(Default::default());
        ui.set_has_source(false);
        ui.set_can_save(false);
        ui.set_saved_item(false);
        ui.set_selected_asset(-1);
        ui.set_asset_name("".into());
        ui.set_notes("".into());
        ui.set_source_label("".into());
        ui.set_result_label("".into());
    }
}

/// Owns its input copy and requires an empty explicit library. Never changes normal saved marks.
pub fn smoke(fixture: PathBuf) -> Result<()> {
    if std::env::var_os("DPDF_DATA_DIR").is_none() {
        bail!("Studio smoke requires an explicit DPDF_DATA_DIR");
    }
    let session = StudioSession::new(false)?;
    let root = session.state.borrow().root.clone();
    if root.exists() && std::fs::read_dir(&root)?.next().is_some() {
        bail!("Studio smoke requires an empty mark-library directory");
    }
    let workspace = tempfile::tempdir()?;
    let source_path = workspace.path().join("fixture-source.img");
    let clipboard_mode = std::env::var_os("DPDF_SMOKE_MARKS_CLIPBOARD").is_some();
    let mut clipboard_owner = None;
    let expected = if clipboard_mode {
        let source = marks::read_image(&fixture)?;
        let rgba = source.image.to_rgba8();
        let snapshot = marks::source_from_rgba(rgba.width(), rgba.height(), rgba.as_raw().clone())?;
        let mut clipboard = arboard::Clipboard::new()?;
        clipboard.set_image(arboard::ImageData {
            width: rgba.width() as usize,
            height: rgba.height() as usize,
            bytes: std::borrow::Cow::Owned(rgba.into_raw()),
        })?;
        clipboard_owner = Some(clipboard);
        snapshot.original
    } else {
        std::fs::read(fixture)?
    };
    std::fs::write(&source_path, &expected)?;
    session.show(false)?;
    let state = session.state.clone();
    let weak = session.ui.as_weak();
    let failure = Rc::new(RefCell::new(None));
    let result = failure.clone();
    let check = slint::Timer::default();
    let mut phase = 0u8;
    let mut ticks = 0;
    check.start(slint::TimerMode::Repeated,Duration::from_millis(40),move || {
        let Some(ui)=weak.upgrade() else { return; };
        let mut state=state.borrow_mut();
        ticks+=1;
        if ticks > 1000 {
            *failure.borrow_mut()=Some(format!("Studio smoke timed out at phase {phase}: {}",ui.get_status()));
            let _=slint::quit_event_loop(); return;
        }
        let mut drag=None; let mut save=false; let mut save_size=false; let mut place=false;
        let outcome=(|| -> Result<()> {
            match phase {
                0 if !state.in_flight && !state.refresh_needed => {
                    if clipboard_mode { state.paste(&ui); } else { state.open(&ui,source_path.clone(),None); } phase=1;
                }
                1 if ui.get_can_save() => {
                    // This is a diagnostic-owned copy. The original supplied fixture stays intact.
                    std::fs::write(&source_path,b"disk file changed after import")?;
                    if let Some(clipboard) = &mut clipboard_owner { clipboard.set_text("Changed isolated fixture clipboard after image capture")?; }
                    ui.set_asset_name("Studio fixture stamp".into()); ui.set_kind(1);
                    ui.set_notes("Generated diagnostic fixture".into());
                    ui.set_angle(2.0); ui.set_width_mm("60".into());
                    let point=|x:f32,y:f32| slint::LogicalPosition::new(ui.get_source_window_x()+x*ui.get_source_view_width(),ui.get_source_window_y()+y*ui.get_source_view_height());
                    drag=Some((point(0.35,0.1),point(0.65,0.85))); phase=2;
                }
                2 if ui.get_can_save() && state.preview_settings.as_ref().is_some_and(|s| s.crop.is_some() && s.deskew_degrees==2.0) => {
                    save=true; phase=3;
                }
                3 if !state.in_flight && state.asset.is_some() => {
                    let asset=state.asset.as_ref().expect("saved asset");
                    let stored=marks::load(&root,&asset.id)?;
                    if std::fs::read(root.join(&asset.id).join("original"))? != expected { bail!("Studio did not preserve the imported source snapshot"); }
                    if stored.kind != MarkKind::Stamp || stored.cleanup.crop.is_none() || (stored.preferred_width_mm-60.0).abs()>0.001 { bail!("Saved cleanup, type or physical size did not match the studio"); }
                    println!("studio smoke: crop drag, deskew and captured-source save passed");
                    if clipboard_mode { println!("studio smoke: native clipboard image snapshot survived later clipboard changes"); }
                    // Save in the same event turn: no preview timer may repair the height first.
                    ui.set_width_mm("72".into());save_size=true;phase=5;
                }
                5 if !state.in_flight && state.asset.as_ref().is_some_and(|a| (a.preferred_width_mm-72.0).abs()<0.001) => {
                    let asset=state.asset.as_ref().expect("saved size").clone();
                    let saved=marks::load(&root,&asset.id)?;
                    if (saved.preferred_width_mm/saved.preferred_height_mm/(saved.pixels.0 as f32/saved.pixels.1 as f32)-1.0).abs()>0.01 { bail!("Preferred size lost its aspect lock"); }
                    if let Some(path)=std::env::var_os("DPDF_SMOKE_MARKS_SNAPSHOT") { crate::snapshot::write(ui.window(),&PathBuf::from(path))?; }
                    state.clear(&ui); state.closed=false; state.refresh_needed=false;
                    state.open(&ui,root.join(&asset.id).join("original"),Some(asset)); phase=6;
                }
                6 if ui.get_can_save() && ui.get_width_mm().as_str()=="72" => {
                    println!("studio smoke: preferred millimetres and aspect lock survived library reopen");
                    state.document=Some(42);ui.set_can_place(true);ui.set_width_mm("84".into());place=true;phase=7;
                }
                7 if state.placement.is_some() => {
                    let mark=state.placement.take().unwrap();
                    let asset_use=mark.asset_use.as_ref().context("Saved-library placement lost its asset identity")?;
                    if marks::load(&asset_use.root,&asset_use.id)?.last_used_at.is_some() {bail!("Arming placement incorrectly counted asset usage");}
                    println!("studio smoke: arming a saved asset did not record page usage");
                    if mark.document!=42 || (mark.size.width_mm-84.0).abs()>0.001 {bail!("Studio placement lost its target or preferred size");}
                    let (pixels,_,_)=crate::imaging::decode(&mark.bytes)?;
                    if (mark.size.width_mm/mark.size.height_mm/(pixels.width() as f32/pixels.height() as f32)-1.0).abs()>0.001 {bail!("Fast placement lost the locked aspect ratio");}
                    println!("studio smoke: immutable cleaned placement payload and physical size passed");
                    state.clear(&ui);
                    if state.source.is_some() || ui.get_has_source() { bail!("Studio close retained working image state"); }
                    println!("studio smoke: working images released on close; complete");
                    let _=slint::quit_event_loop(); phase=8;
                }
                _=>{}
            }
            Ok(())
        })();
        drop(state);
        if let Err(error)=outcome { *failure.borrow_mut()=Some(format!("{error:#}")); let _=slint::quit_event_loop(); return; }
        if let Some((start,end))=drag {
            use slint::platform::{PointerEventButton,WindowEvent};
            ui.window().dispatch_event(WindowEvent::PointerMoved{position:start});
            ui.window().dispatch_event(WindowEvent::PointerPressed{position:start,button:PointerEventButton::Left});
            ui.window().dispatch_event(WindowEvent::PointerMoved{position:end});
            ui.window().dispatch_event(WindowEvent::PointerReleased{position:end,button:PointerEventButton::Left});
        }
        if save { ui.invoke_save_asset(); }
        if save_size { ui.invoke_save_size(); }
        if place { ui.invoke_place_mark(); }
    });
    session.ui.run()?;
    if let Some(error) = result.borrow_mut().take() {
        bail!(error);
    }
    Ok(())
}
