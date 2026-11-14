//! Local visual-signature/stamp assets. These images are not cryptographic signatures.
use anyhow::{Context, Result, bail};
use image::{DynamicImage, ImageEncoder, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Crop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cleanup {
    pub crop: Option<Crop>,
    pub deskew_degrees: f32,
    pub remove_white: bool,
    pub white_level: u8,
    pub black_level: u8,
}

impl Default for Cleanup {
    fn default() -> Self {
        Self {
            crop: None,
            deskew_degrees: 0.0,
            remove_white: true,
            white_level: 245,
            black_level: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarkKind {
    Signature,
    Stamp,
    Initials,
    Other,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Asset {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub kind: MarkKind,
    pub notes: String,
    pub source_format: String,
    pub pixels: (u32, u32),
    pub preferred_width_mm: f32,
    pub preferred_height_mm: f32,
    pub aspect_locked: bool,
    pub cleanup: Cleanup,
    pub imported_at: u64,
    pub last_used_at: Option<u64>,
}

fn validate_size(width: f32, height: f32) -> Result<()> {
    if !width.is_finite()
        || !height.is_finite()
        || !(0.1..=300.0).contains(&width)
        || !(0.1..=300.0).contains(&height)
    {
        bail!("Mark dimensions must be between 0.1 and 300 mm");
    }
    Ok(())
}

fn validate_id(id: &str) -> Result<()> {
    if !id.starts_with("asset-")
        || id.len() > 64
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        bail!("Invalid local asset ID");
    }
    Ok(())
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// White-matte removal uses the darkest channel as opacity, preserving colored ink
/// and removing its white fringe. Existing source alpha is multiplied, never made opaque.
pub fn cleanup(source: &DynamicImage, settings: &Cleanup) -> Result<RgbaImage> {
    if !settings.deskew_degrees.is_finite() || settings.deskew_degrees.abs() > 15.0 {
        bail!("Deskew must be between -15 and 15 degrees");
    }
    if settings.white_level <= settings.black_level {
        bail!("White level must exceed black level");
    }
    let mut image = if let Some(crop) = settings.crop {
        if crop.width == 0
            || crop.height == 0
            || crop
                .x
                .checked_add(crop.width)
                .is_none_or(|end| end > source.width())
            || crop
                .y
                .checked_add(crop.height)
                .is_none_or(|end| end > source.height())
        {
            bail!("Crop must lie inside the source image");
        }
        source
            .crop_imm(crop.x, crop.y, crop.width, crop.height)
            .to_rgba8()
    } else {
        source.to_rgba8()
    };
    if settings.remove_white {
        let white = settings.white_level as f32;
        let span = (settings.white_level - settings.black_level) as f32;
        for pixel in image.pixels_mut() {
            let minimum = pixel.0[..3].iter().copied().min().unwrap_or(255) as f32;
            let alpha = ((white - minimum) / span).clamp(0.0, 1.0);
            if alpha == 0.0 {
                *pixel = Rgba([0, 0, 0, 0]);
                continue;
            }
            for channel in &mut pixel.0[..3] {
                *channel = ((*channel as f32 - 255.0 * (1.0 - alpha)) / alpha)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            pixel.0[3] = (pixel.0[3] as f32 * alpha).round() as u8;
            if pixel.0[3] == 0 {
                *pixel = Rgba([0, 0, 0, 0]);
            }
        }
    }
    if settings.deskew_degrees.abs() > 0.001 {
        image = deskew(&image, settings.deskew_degrees)?;
    }
    if !image.pixels().any(|p| p.0[3] != 0) {
        bail!("Cleanup removed all ink; adjust crop or white level");
    }
    Ok(image)
}

fn deskew(source: &RgbaImage, degrees: f32) -> Result<RgbaImage> {
    let radians = degrees.to_radians();
    let (sin, cos) = radians.sin_cos();
    let (width, height) = (source.width() as f32, source.height() as f32);
    let out_width = (width * cos.abs() + height * sin.abs()).ceil() as u32;
    let out_height = (width * sin.abs() + height * cos.abs()).ceil() as u32;
    if u64::from(out_width) * u64::from(out_height) > crate::imaging::MAX_PIXELS {
        bail!("Deskewed image exceeds 32 megapixels; crop or resize the source first");
    }
    let mut output = RgbaImage::new(out_width, out_height);
    for (x, y, pixel) in output.enumerate_pixels_mut() {
        let dx = x as f32 + 0.5 - out_width as f32 / 2.0;
        let dy = y as f32 + 0.5 - out_height as f32 / 2.0;
        let sx = cos * dx + sin * dy + width / 2.0 - 0.5;
        let sy = -sin * dx + cos * dy + height / 2.0 - 0.5;
        let (x0, y0) = (sx.floor() as i64, sy.floor() as i64);
        let (fx, fy) = (sx - sx.floor(), sy - sy.floor());
        let mut alpha = 0.0;
        let mut premultiplied = [0.0f32; 3];
        for (px, py, weight) in [
            (x0, y0, (1.0 - fx) * (1.0 - fy)),
            (x0 + 1, y0, fx * (1.0 - fy)),
            (x0, y0 + 1, (1.0 - fx) * fy),
            (x0 + 1, y0 + 1, fx * fy),
        ] {
            if px < 0 || py < 0 || px >= source.width() as i64 || py >= source.height() as i64 {
                continue;
            }
            let source = source.get_pixel(px as u32, py as u32);
            let contribution = source.0[3] as f32 / 255.0 * weight;
            alpha += contribution;
            for (channel, value) in premultiplied.iter_mut().zip(source.0[..3].iter()) {
                *channel += *value as f32 * contribution;
            }
        }
        if alpha > 0.0 {
            *pixel = Rgba([
                (premultiplied[0] / alpha).round() as u8,
                (premultiplied[1] / alpha).round() as u8,
                (premultiplied[2] / alpha).round() as u8,
                (alpha * 255.0).round() as u8,
            ]);
        }
    }
    Ok(output)
}

pub struct MarkSource {
    pub original: Vec<u8>,
    pub image: DynamicImage,
    pub format: String,
}

#[derive(Clone, Copy)]
pub struct PreferredSize {
    pub width_mm: f32,
    pub height_mm: f32,
    pub aspect_locked: bool,
}

pub struct ImportOptions {
    pub name: String,
    pub kind: MarkKind,
    pub notes: String,
    pub preferred_size: Option<PreferredSize>,
    pub cleanup: Cleanup,
}

pub fn source_from_rgba(width: u32, height: u32, bytes: Vec<u8>) -> Result<MarkSource> {
    if width == 0
        || height == 0
        || u64::from(width) * u64::from(height) > crate::imaging::MAX_PIXELS
    {
        bail!("Clipboard image must contain between 1 and 32 million pixels");
    }
    if bytes.len() as u64 != u64::from(width) * u64::from(height) * 4 {
        bail!("Clipboard image has invalid RGBA dimensions");
    }
    let image = RgbaImage::from_raw(width, height, bytes)
        .context("Clipboard image has invalid RGBA dimensions")?;
    let original = png_ref(&image)?;
    if original.len() as u64 > crate::imaging::MAX_INPUT {
        bail!("Clipboard PNG exceeds 64 MiB; crop or resize the source first");
    }
    Ok(MarkSource {
        original,
        image: DynamicImage::ImageRgba8(image),
        format: "PNG".into(),
    })
}

pub fn read_image(path: &Path) -> Result<MarkSource> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(crate::imaging::MAX_INPUT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > crate::imaging::MAX_INPUT {
        bail!("Signature/stamp image exceeds 64 MiB");
    }
    let (image, _, format) = crate::imaging::decode(&bytes)?;
    Ok(MarkSource {
        original: bytes,
        image,
        format: format!("{format:?}"),
    })
}

fn png(image: RgbaImage) -> Result<Vec<u8>> {
    png_ref(&image)
}

pub fn png_ref(image: &RgbaImage) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes).write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(bytes)
}

pub fn clean_copy(source: &Path, destination: &Path, settings: &Cleanup) -> Result<()> {
    crate::storage::ensure_separate_output(destination, &[source])?;
    let source = read_image(source)?;
    crate::storage::atomic_write(&png(cleanup(&source.image, settings)?)?, destination)
}

pub fn import(
    root: &Path,
    source: &Path,
    name: String,
    kind: MarkKind,
    settings: Cleanup,
) -> Result<Asset> {
    import_snapshot(
        root,
        &read_image(source)?,
        ImportOptions {
            name,
            kind,
            notes: String::new(),
            preferred_size: None,
            cleanup: settings,
        },
    )
}

/// Preview and save share the same original snapshot, even if its disk file changes.
pub fn import_snapshot(root: &Path, source: &MarkSource, options: ImportOptions) -> Result<Asset> {
    if options.name.trim().is_empty() || options.name.len() > 256 {
        bail!("Enter an asset name of 1–256 bytes");
    }
    if options.notes.len() > 4096 {
        bail!("Asset notes must be at most 4096 bytes");
    }
    let cleaned = cleanup(&source.image, &options.cleanup)?;
    let pixels = cleaned.dimensions();
    let default_width = 45.0f32.min(100.0 * pixels.0 as f32 / pixels.1 as f32);
    let preferred = options.preferred_size.unwrap_or(PreferredSize {
        width_mm: default_width,
        height_mm: default_width * pixels.1 as f32 / pixels.0 as f32,
        aspect_locked: true,
    });
    validate_preferred(pixels, preferred)?;
    std::fs::create_dir_all(root)?;
    let folder = tempfile::Builder::new()
        .prefix(".import-")
        .tempdir_in(root)?;
    let id = format!(
        "asset-{}",
        folder
            .path()
            .file_name()
            .context("Asset folder name")?
            .to_string_lossy()
            .trim_start_matches(".import-")
    );
    let asset = Asset {
        schema_version: 1,
        id,
        name: options.name.trim().into(),
        kind: options.kind,
        notes: options.notes,
        source_format: source.format.clone(),
        pixels,
        preferred_width_mm: preferred.width_mm,
        preferred_height_mm: preferred.height_mm,
        aspect_locked: preferred.aspect_locked,
        cleanup: options.cleanup,
        imported_at: timestamp(),
        last_used_at: None,
    };
    let cleaned = DynamicImage::ImageRgba8(cleaned);
    let thumbnail = cleaned.thumbnail(160, 100).to_rgba8();
    std::fs::write(folder.path().join("original"), &source.original)?;
    std::fs::write(
        folder.path().join("cleaned.png"),
        png(cleaned.into_rgba8())?,
    )?;
    std::fs::write(folder.path().join("thumbnail.png"), png(thumbnail)?)?;
    crate::storage::atomic_write(
        &serde_json::to_vec_pretty(&asset)?,
        &folder.path().join("metadata.json"),
    )?;
    // Publish a complete asset directory in one rename. In-progress imports are not listed.
    let destination = root.join(&asset.id);
    if destination.exists() {
        bail!("Asset ID collision; retry the import");
    }
    std::fs::rename(folder.path(), destination)?;
    let _ = folder.keep();
    Ok(asset)
}

pub fn load(root: &Path, id: &str) -> Result<Asset> {
    validate_id(id)?;
    let path = root.join(id).join("metadata.json");
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(65_537)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 65_536 {
        bail!("Asset metadata is too large");
    }
    let asset: Asset = serde_json::from_slice(&bytes)?;
    if asset.schema_version != 1
        || asset.id != id
        || asset.name.len() > 256
        || asset.notes.len() > 4096
        || asset.pixels.0 == 0
        || asset.pixels.1 == 0
        || u64::from(asset.pixels.0) * u64::from(asset.pixels.1) > crate::imaging::MAX_PIXELS
    {
        bail!("Unsupported or invalid asset metadata");
    }
    validate_size(asset.preferred_width_mm, asset.preferred_height_mm)?;
    Ok(asset)
}

#[derive(Clone, Debug)]
pub struct AssetUse {
    pub root: PathBuf,
    pub id: String,
}

/// One bounded I/O worker; bookkeeping never blocks page input/rendering.
pub struct UsageWorker {
    requests: std::sync::mpsc::SyncSender<(AssetUse, u64)>,
    replies: std::sync::mpsc::Receiver<Result<(), String>>,
}
impl UsageWorker {
    pub fn start() -> Option<Self> {
        let (requests, pending) = std::sync::mpsc::sync_channel::<(AssetUse, u64)>(64);
        let (completed, replies) = std::sync::mpsc::sync_channel(64);
        std::thread::Builder::new()
            .name("dpdf-asset-use".into())
            .spawn(move || {
                while let Ok((asset, used_at)) = pending.recv() {
                    let result = record_use_at(&asset.root, &asset.id, used_at)
                        .map(|_| ())
                        .map_err(|error| format!("{error:#}"));
                    if completed.send(result).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Self { requests, replies })
    }

    pub fn record(&self, asset: AssetUse) -> Result<()> {
        self.requests
            .try_send((asset, timestamp()))
            .context("Asset usage worker is busy or unavailable")
    }

    pub fn poll_error(&self) -> Option<String> {
        while let Ok(result) = self.replies.try_recv() {
            if let Err(error) = result {
                return Some(error);
            }
        }
        None
    }
}

fn metadata_lock(root: &Path, id: &str) -> Result<std::fs::File> {
    validate_id(id)?;
    let lock = std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(id).join("metadata.lock"))
        .context("Open asset metadata lock")?;
    // Metadata flushes may briefly contend on Windows. These writes run on I/O
    // workers; retain a bounded wait without rejecting a legitimate slow writer.
    let started = std::time::Instant::now();
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(lock),
            Err(std::fs::TryLockError::WouldBlock)
                if started.elapsed() < std::time::Duration::from_secs(5) =>
            {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(error) => {
                #[cfg(test)]
                eprintln!(
                    "metadata lock failed after {} ms: {error:?}",
                    started.elapsed().as_millis()
                );
                return Err(anyhow::anyhow!(
                    "Asset metadata is locked or unavailable: {error}"
                ));
            }
        }
    }
}

fn record_use_at(root: &Path, id: &str, used_at: u64) -> Result<Asset> {
    let _lock = metadata_lock(root, id)?;
    let mut asset = load(root, id)?;
    // Concurrent/queued updates cannot move usage backwards or overwrite a new size.
    asset.last_used_at = Some(asset.last_used_at.unwrap_or(0).max(used_at));
    crate::storage::atomic_write(
        &serde_json::to_vec_pretty(&asset)?,
        &root.join(id).join("metadata.json"),
    )?;
    Ok(asset)
}

pub fn save_size(
    root: &Path,
    id: &str,
    width: f32,
    height: f32,
    aspect_locked: bool,
) -> Result<Asset> {
    let _lock = metadata_lock(root, id)?;
    let mut asset = load(root, id)?;
    validate_preferred(
        asset.pixels,
        PreferredSize {
            width_mm: width,
            height_mm: height,
            aspect_locked,
        },
    )?;
    asset.preferred_width_mm = width;
    asset.preferred_height_mm = height;
    asset.aspect_locked = aspect_locked;
    crate::storage::atomic_write(
        &serde_json::to_vec_pretty(&asset)?,
        &root.join(id).join("metadata.json"),
    )?;
    Ok(asset)
}

fn validate_preferred(pixels: (u32, u32), size: PreferredSize) -> Result<()> {
    validate_size(size.width_mm, size.height_mm)?;
    if size.aspect_locked {
        let ratio = pixels.0 as f32 / pixels.1 as f32;
        if (size.width_mm / size.height_mm / ratio - 1.0).abs() > 0.01 {
            bail!("Preferred size must preserve the image aspect ratio while locked");
        }
    }
    Ok(())
}

pub struct LibraryScan {
    pub assets: Vec<Asset>,
    pub warnings: Vec<String>,
}

pub fn scan(root: &Path) -> Result<LibraryScan> {
    if !root.exists() {
        return Ok(LibraryScan {
            assets: vec![],
            warnings: vec![],
        });
    }
    let mut assets = Vec::new();
    let mut warnings = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let id = entry.file_name().to_string_lossy().into_owned();
        if !entry.file_type()?.is_dir() || validate_id(&id).is_err() {
            continue;
        }
        // Broken or future-format assets must be reported, never silently overwritten.
        match load(root, &id) {
            Ok(asset) => assets.push(asset),
            Err(error) => warnings.push(format!("Read local asset {id}: {error:#}")),
        }
    }
    assets.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(LibraryScan { assets, warnings })
}

pub fn list(root: &Path) -> Result<Vec<Asset>> {
    let scan = scan(root)?;
    if !scan.warnings.is_empty() {
        bail!(
            "{}",
            scan.warnings
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    Ok(scan.assets)
}

pub fn default_root() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("DPDF_DATA_DIR") {
        return Some(PathBuf::from(path).join("marks"));
    }
    #[cfg(target_os = "windows")]
    let directory = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let directory = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    directory.map(|p| p.join("dPDF/marks"))
}

/// Development interface for exercising assets before the cleanup/placement UI is complete.
pub fn run_cli(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    let action = args.next().context("--marks list|import|size|clean")?;
    let mut next = || args.next().context("Missing mark argument");
    let result = match action.to_string_lossy().as_ref() {
        "clean" => {
            let source = PathBuf::from(next()?);
            let destination = PathBuf::from(next()?);
            let settings = read_settings(args.next().map(PathBuf::from))?;
            clean_copy(&source, &destination, &settings)?;
            serde_json::json!({"cleaned": destination})
        }
        "list" => {
            let root = args
                .next()
                .map(PathBuf::from)
                .or_else(default_root)
                .context("Choose a local library directory")?;
            serde_json::to_value(list(&root)?)?
        }
        "import" => {
            let source = PathBuf::from(next()?);
            let name = next()?.to_string_lossy().into_owned();
            let kind = match next()?.to_string_lossy().as_ref() {
                "signature" => MarkKind::Signature,
                "stamp" => MarkKind::Stamp,
                "initials" => MarkKind::Initials,
                "other" => MarkKind::Other,
                _ => bail!("Mark type must be signature, stamp, initials or other"),
            };
            let root = args
                .next()
                .map(PathBuf::from)
                .or_else(default_root)
                .context("Choose a local library directory")?;
            let settings = read_settings(args.next().map(PathBuf::from))?;
            serde_json::to_value(import(&root, &source, name, kind, settings)?)?
        }
        "size" => {
            let id = next()?.to_string_lossy().into_owned();
            let width = next()?.to_string_lossy().parse()?;
            let height = next()?.to_string_lossy().parse()?;
            let root = args
                .next()
                .map(PathBuf::from)
                .or_else(default_root)
                .context("Choose a local library directory")?;
            let locked = match args.next().as_deref().and_then(|s| s.to_str()) {
                None | Some("locked") => true,
                Some("free") => false,
                _ => bail!("Aspect option must be locked or free"),
            };
            serde_json::to_value(save_size(&root, &id, width, height, locked)?)?
        }
        _ => bail!(
            "--marks list [LIBRARY] | import IMAGE NAME TYPE [LIBRARY] [SETTINGS.json] | size ID WIDTH_MM HEIGHT_MM [LIBRARY] [locked|free] | clean IMAGE OUTPUT.png [SETTINGS.json]"
        ),
    };
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn read_settings(path: Option<PathBuf>) -> Result<Cleanup> {
    let Some(path) = path else {
        return Ok(Cleanup::default());
    };
    if std::fs::metadata(&path)?.len() > 65_536 {
        bail!("Cleanup settings are too large");
    }
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_usage_and_size_updates_preserve_assets_and_future_metadata() {
        use std::sync::{Arc, Barrier};
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("library");
        let source = source_from_rgba(8, 4, vec![80; 8 * 4 * 4]).unwrap();
        let asset = import_snapshot(
            &root,
            &source,
            ImportOptions {
                name: "Usage fixture".into(),
                kind: MarkKind::Stamp,
                notes: "Retained notes".into(),
                preferred_size: Some(PreferredSize {
                    width_mm: 30.0,
                    height_mm: 15.0,
                    aspect_locked: true,
                }),
                cleanup: Cleanup::default(),
            },
        )
        .unwrap();
        let files = ["original", "cleaned.png", "thumbnail.png"];
        let before: Vec<_> = files
            .iter()
            .map(|name| std::fs::read(root.join(&asset.id).join(name)).unwrap())
            .collect();
        assert!(load(&root, &asset.id).unwrap().last_used_at.is_none());
        let barrier = Arc::new(Barrier::new(2));
        let now = timestamp();
        let started = std::time::Instant::now();
        let size_thread = {
            let (root, id, barrier) = (root.clone(), asset.id.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                let mut trace = Vec::new();
                for step in 1..=16 {
                    let begin = std::time::Instant::now();
                    let width = 30.0 + step as f32;
                    let result = save_size(&root, &id, width, width / 2.0, true);
                    trace.push((
                        step,
                        begin.duration_since(started).as_millis(),
                        begin.elapsed().as_millis(),
                    ));
                    if let Err(error) = result {
                        return (trace, Err(format!("{error:#}")));
                    }
                }
                (trace, Ok(()))
            })
        };
        let usage_thread = {
            let (root, id) = (root.clone(), asset.id.clone());
            std::thread::spawn(move || {
                barrier.wait();
                let mut trace = Vec::new();
                for step in 1..=16 {
                    let begin = std::time::Instant::now();
                    let result = record_use_at(&root, &id, now + step);
                    trace.push((
                        step,
                        begin.duration_since(started).as_millis(),
                        begin.elapsed().as_millis(),
                    ));
                    if let Err(error) = result {
                        return (trace, Err(format!("{error:#}")));
                    }
                }
                (trace, Ok(()))
            })
        };
        let size = size_thread.join().unwrap();
        let usage = usage_thread.join().unwrap();
        eprintln!("metadata writers (step, start_ms, duration_ms): size={size:?}; usage={usage:?}");
        assert!(
            size.1.is_ok() && usage.1.is_ok(),
            "metadata writers (step, start_ms, duration_ms): size={size:?}; usage={usage:?}"
        );
        // An older queued event cannot roll back a newer usage time.
        record_use_at(&root, &asset.id, now + 1).unwrap();
        let stored = load(&root, &asset.id).unwrap();
        assert_eq!(stored.last_used_at, Some(now + 16));
        assert_eq!(
            (stored.preferred_width_mm, stored.preferred_height_mm),
            (46.0, 23.0)
        );
        assert_eq!(stored.notes, "Retained notes");
        for (name, bytes) in files.iter().zip(before) {
            assert_eq!(
                std::fs::read(root.join(&asset.id).join(name)).unwrap(),
                bytes
            );
        }
        let metadata = root.join(&asset.id).join("metadata.json");
        let mut future: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&metadata).unwrap()).unwrap();
        future["schema_version"] = 999.into();
        let bytes = serde_json::to_vec(&future).unwrap();
        std::fs::write(&metadata, &bytes).unwrap();
        assert!(record_use_at(&root, &asset.id, now + 100).is_err());
        assert_eq!(std::fs::read(metadata).unwrap(), bytes);
    }

    #[test]
    fn metadata_lock_waits_for_brief_contention_and_refuses_persistent_locks() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let id = "asset-lock-fixture";
        std::fs::create_dir(root.join(id)).unwrap();
        let held = metadata_lock(root, id).unwrap();
        let (ready, waiting) = std::sync::mpsc::channel();
        let waiter_root = root.to_path_buf();
        let waiter = std::thread::spawn(move || {
            ready.send(()).unwrap();
            metadata_lock(&waiter_root, id)
        });
        waiting.recv().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1300));
        drop(held);
        let acquired = waiter.join().unwrap().unwrap();
        // The lock is still held: a second writer must eventually return an error.
        assert!(metadata_lock(root, id).is_err());
        drop(acquired);
        assert!(metadata_lock(root, id).is_ok());
    }

    #[test]
    fn clipboard_snapshot_is_lossless_rgba_and_rejects_invalid_buffer_dimensions() {
        let pixels = vec![0, 0, 255, 128, 255, 0, 0, 255];
        let source = source_from_rgba(2, 1, pixels.clone()).unwrap();
        assert_eq!(source.image.to_rgba8().as_raw(), &pixels);
        assert_eq!(
            crate::imaging::decode(&source.original)
                .unwrap()
                .0
                .to_rgba8()
                .into_raw(),
            pixels
        );
        assert!(source_from_rgba(0, 1, vec![]).is_err());
        assert!(source_from_rgba(1, 1, vec![0; 5]).is_err());
        assert!(source_from_rgba(10_000, 10_000, vec![]).is_err());
    }

    #[test]
    fn preview_snapshot_survives_disk_changes_and_imports_its_selected_physical_size() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("source.png");
        let original = png(RgbaImage::from_pixel(8, 4, Rgba([10, 30, 150, 255]))).unwrap();
        std::fs::write(&path, &original).unwrap();
        let snapshot = read_image(&path).unwrap();
        std::fs::write(&path, b"changed after preview").unwrap();
        let root = temporary.path().join("library");
        let asset = import_snapshot(
            &root,
            &snapshot,
            ImportOptions {
                name: "Snapshot stamp".into(),
                kind: MarkKind::Stamp,
                notes: "Fixture notes".into(),
                preferred_size: Some(PreferredSize {
                    width_mm: 30.0,
                    height_mm: 15.0,
                    aspect_locked: true,
                }),
                cleanup: Cleanup::default(),
            },
        )
        .unwrap();
        assert_eq!(
            std::fs::read(root.join(&asset.id).join("original")).unwrap(),
            original
        );
        let restored = load(&root, &asset.id).unwrap();
        assert_eq!(restored.preferred_width_mm, 30.0);
        assert_eq!(restored.preferred_height_mm, 15.0);
        assert_eq!(restored.notes, "Fixture notes");
        assert_eq!(std::fs::read(path).unwrap(), b"changed after preview");
    }

    #[test]
    fn white_cleanup_preserves_color_and_existing_alpha_without_a_white_fringe() {
        let image = RgbaImage::from_fn(4, 1, |x, _| {
            Rgba(match x {
                0 => [255, 255, 255, 255],
                1 => [0, 0, 0, 80],
                2 => [128, 128, 255, 255],
                _ => [200, 40, 40, 255],
            })
        });
        let result = cleanup(
            &DynamicImage::ImageRgba8(image.clone()),
            &Cleanup {
                white_level: 255,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.get_pixel(0, 0).0, [0, 0, 0, 0]);
        assert_eq!(result.get_pixel(1, 0).0, [0, 0, 0, 80]);
        assert_eq!(result.get_pixel(2, 0).0, [0, 0, 255, 127]);
        for x in 2..4 {
            let alpha = result.get_pixel(x, 0).0[3] as f32 / 255.0;
            for channel in 0..3 {
                let recomposited =
                    result.get_pixel(x, 0).0[channel] as f32 * alpha + 255.0 * (1.0 - alpha);
                assert!((recomposited - image.get_pixel(x, 0).0[channel] as f32).abs() <= 1.0);
            }
        }
    }

    #[test]
    fn crop_and_deskew_validate_bounds_and_keep_color_on_transparent_edges() {
        let source = DynamicImage::ImageRgba8(RgbaImage::from_pixel(10, 6, Rgba([255, 0, 0, 255])));
        let output = cleanup(
            &source,
            &Cleanup {
                deskew_degrees: 10.0,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(output.width() > 10 && output.height() > 6);
        assert!(output.pixels().any(|p| p.0[3] > 0 && p.0[3] < 255));
        for pixel in output.pixels().filter(|p| p.0[3] > 0) {
            assert_eq!(&pixel.0[..3], &[255, 0, 0]);
        }
        assert!(
            cleanup(
                &source,
                &Cleanup {
                    crop: Some(Crop {
                        x: u32::MAX,
                        y: 0,
                        width: 2,
                        height: 1
                    }),
                    ..Default::default()
                }
            )
            .is_err()
        );
        assert!(
            cleanup(
                &source,
                &Cleanup {
                    deskew_degrees: f32::NAN,
                    ..Default::default()
                }
            )
            .is_err()
        );
        let white =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255])));
        assert!(cleanup(&white, &Cleanup::default()).is_err());
    }

    #[test]
    fn library_preserves_original_and_persists_physical_size_without_pixel_assumptions() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("source.png");
        let bytes = png(RgbaImage::from_pixel(8, 4, Rgba([10, 30, 150, 255]))).unwrap();
        std::fs::write(&source, &bytes).unwrap();
        let root = temporary.path().join("library");
        let asset = import(
            &root,
            &source,
            "Test stamp".into(),
            MarkKind::Stamp,
            Cleanup::default(),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(root.join(&asset.id).join("original")).unwrap(),
            bytes
        );
        assert_eq!(std::fs::read(&source).unwrap(), bytes);
        assert_eq!(asset.preferred_width_mm, 45.0);
        assert_eq!(asset.preferred_height_mm, 22.5);
        save_size(&root, &asset.id, 30.0, 15.0, true).unwrap();
        assert_eq!(load(&root, &asset.id).unwrap().preferred_width_mm, 30.0);
        assert_eq!(list(&root).unwrap().len(), 1);
        assert!(save_size(&root, &asset.id, f32::INFINITY, 15.0, true).is_err());
        assert!(load(&root, "asset-../escape").is_err());
        assert!(clean_copy(&source, &source, &Cleanup::default()).is_err());
        let metadata = root.join(&asset.id).join("metadata.json");
        assert!(save_size(&root, &asset.id, 30.0, 30.0, true).is_err());
        let mut future = serde_json::to_value(&asset).unwrap();
        future["schema_version"] = serde_json::json!(99);
        let future = serde_json::to_vec(&future).unwrap();
        std::fs::write(&metadata, &future).unwrap();
        assert!(load(&root, &asset.id).is_err());
        assert_eq!(std::fs::read(&metadata).unwrap(), future);
        let good = import(
            &root,
            &source,
            "Valid alongside future asset".into(),
            MarkKind::Other,
            Cleanup::default(),
        )
        .unwrap();
        let scan = scan(&root).unwrap();
        assert_eq!(scan.assets.len(), 1);
        assert_eq!(scan.assets[0].id, good.id);
        assert_eq!(scan.warnings.len(), 1);
        assert!(list(&root).is_err());
        assert_eq!(std::fs::read(metadata).unwrap(), future);
    }
}
