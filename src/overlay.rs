//! Ordinary visual-mark overlays. Edited pages are assembled with untouched source pages.
use crate::{
    assembly::{self, ExportResult, SourceSnapshot},
    document::NormalizedRect,
    imaging,
    pages::PagePlan,
};
use anyhow::{Context, Result, bail, ensure};
use image::{DynamicImage, RgbaImage};
use pdfium_render::prelude::*;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    pub x_mm: f32,
    pub y_mm: f32,
    pub width_mm: f32,
    pub height_mm: f32,
    #[serde(default)]
    pub quarter_turns: u8,
}

impl Geometry {
    pub fn normalized(
        &self,
        page_width_points: f32,
        page_height_points: f32,
    ) -> Result<NormalizedRect> {
        let page_width = page_width_points * 25.4 / 72.0;
        let page_height = page_height_points * 25.4 / 72.0;
        let (width, height) = if self.quarter_turns.is_multiple_of(2) {
            (self.width_mm, self.height_mm)
        } else {
            (self.height_mm, self.width_mm)
        };
        if ![
            self.x_mm,
            self.y_mm,
            self.width_mm,
            self.height_mm,
            page_width,
            page_height,
        ]
        .iter()
        .all(|v| v.is_finite())
            || self.quarter_turns > 3
            || self.x_mm < 0.0
            || self.y_mm < 0.0
            || !(0.1..=300.0).contains(&self.width_mm)
            || !(0.1..=300.0).contains(&self.height_mm)
            || page_width <= 0.0
            || page_height <= 0.0
            || self.x_mm + width > page_width + 0.001
            || self.y_mm + height > page_height + 0.001
        {
            bail!(
                "Placed mark must fit inside the page and have dimensions between 0.1 and 300 mm"
            );
        }
        Ok(NormalizedRect {
            x: self.x_mm / page_width,
            y: self.y_mm / page_height,
            width: width / page_width,
            height: height / page_height,
        })
    }
}

#[derive(Clone)]
pub struct Placement {
    pub page: usize,
    pub geometry: Geometry,
    pub opacity: f32,
    pub image: Arc<Vec<u8>>,
}

pub struct PreparedExport {
    sources: Vec<SourceSnapshot>,
    replacements: Vec<usize>,
    units: Vec<f64>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PlacementSpec {
    page: usize,
    image: PathBuf,
    geometry: Geometry,
    #[serde(default = "opaque")]
    opacity: f32,
}
fn opaque() -> f32 {
    1.0
}

/// Diagnostic copy-export entry point; UI placement will use the same worker API.
pub fn run_cli(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<()> {
    let input = PathBuf::from(
        args.next()
            .context("--overlay INPUT.pdf OUTPUT.pdf PLACEMENTS.json")?,
    );
    let output = PathBuf::from(args.next().context("Missing output PDF")?);
    let recipe = PathBuf::from(args.next().context("Missing placement JSON")?).canonicalize()?;
    if args.next().is_some() {
        bail!("--overlay accepts exactly three paths");
    }
    let output_parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()?;
    let output_path = output_parent.join(output.file_name().context("Choose output filename")?);
    crate::storage::ensure_separate_output(&output_path, &[&recipe, &input])?;
    let mut json = Vec::new();
    std::fs::File::open(&recipe)?
        .take(256 * 1024 + 1)
        .read_to_end(&mut json)?;
    if json.len() > 256 * 1024 {
        bail!("Placement JSON exceeds 256 KiB");
    }
    let specs: Vec<PlacementSpec> =
        serde_json::from_slice(&json).context("Read placement array")?;
    if specs.is_empty() || specs.len() > 256 {
        bail!("Specify between 1 and 256 marks");
    }
    let mut images = HashMap::new();
    let mut placements = Vec::new();
    let mut total = 0usize;
    for spec in specs {
        let path = recipe.parent().unwrap().join(&spec.image).canonicalize()?;
        crate::storage::ensure_separate_output(&output_path, &[&path])?;
        let image = if let Some(image) = images.get(&path) {
            Arc::clone(image)
        } else {
            let mut bytes = Vec::new();
            std::fs::File::open(&path)?
                .take(imaging::MAX_INPUT + 1)
                .read_to_end(&mut bytes)?;
            total = total
                .checked_add(bytes.len())
                .context("Image size overflow")?;
            if total > imaging::MAX_INPUT as usize {
                bail!("Mark images exceed 64 MiB");
            }
            let image = Arc::new(bytes);
            images.insert(path, Arc::clone(&image));
            image
        };
        placements.push(Placement {
            page: spec
                .page
                .checked_sub(1)
                .context("Page numbers start at 1")?,
            geometry: spec.geometry,
            opacity: spec.opacity,
            image,
        });
    }
    let original = SourceSnapshot {
        bytes: Arc::new(std::fs::read(&input)?),
        path: input,
    };
    let pdfium = crate::pdf::bind_pdfium()?;
    let document = pdfium.load_pdf_from_byte_slice(&original.bytes, None)?;
    let prepared = prepare(&pdfium, &document, original.clone(), &placements)?;
    eprintln!(
        "Simple overlay: editable image objects, not secure baking. Modified copies invalidate existing digital signatures."
    );
    let mut protected: Vec<_> = images.keys().map(|path| path.as_path()).collect();
    protected.push(&recipe);
    protected.push(&original.path);
    crate::storage::ensure_separate_output(&output_path, &protected)?;
    let result = prepared.save(&output)?;
    println!(
        "saved_pages={} output={}",
        result.pages,
        result.path.display()
    );
    Ok(())
}

impl PreparedExport {
    pub fn save(self, destination: &Path) -> Result<ExportResult> {
        self.save_cancellable(destination, None)
    }

    pub(crate) fn save_cancellable(
        self,
        destination: &Path,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<ExportResult> {
        crate::background_jobs::check_cancelled(cancel)?;
        // Serialized-page verification is byte-only helper work. It must not
        // run while the reader owns the native document being rendered.
        for (source, &unit) in self.sources.iter().skip(1).zip(&self.units) {
            if unit != 1.0 {
                let metadata = assembly::inspect_snapshot_cancellable(&source.bytes, cancel)?;
                let saved = crate::pdf_units::from_metadata(&metadata, 1)?;
                ensure!(
                    (saved[0] - unit).abs() <= unit * 1e-6,
                    "Modified page did not preserve its physical unit scale"
                );
            }
        }
        assembly::export_plan_cancellable(
            &self.sources,
            destination,
            true,
            |counts| {
                let mut plan = PagePlan::new(counts)?;
                for (index, page) in self.replacements.iter().enumerate() {
                    plan.replace(*page, index + 1, 0)?;
                }
                Ok(plan)
            },
            cancel,
        )
    }
}

fn matrix(page: &PdfPage<'_>, geometry: Geometry, unit: f64) -> Result<PdfMatrix> {
    let width = (f64::from(page.width().value) * unit) as f32;
    let height = (f64::from(page.height().value) * unit) as f32;
    let rect = geometry.normalized(width, height)?;
    let (left, right, top, bottom) = (rect.x, rect.x + rect.width, rect.y, rect.y + rect.height);
    // Image unit coordinates have a bottom-left origin. Map its three corners
    // through native DeviceToPage, including crop offsets and intrinsic rotation.
    let corners = match geometry.quarter_turns {
        1 => [(left, top), (left, bottom), (right, top)],
        2 => [(right, top), (left, top), (right, bottom)],
        3 => [(right, bottom), (right, top), (left, bottom)],
        _ => [(left, bottom), (right, bottom), (left, top)],
    };
    let config = PdfRenderConfig::new().set_fixed_size(1_000_000, 1_000_000);
    for (x, y) in corners {
        let dx = ((x * 1_000_000.0).round() / 1_000_000.0 - x).abs() * width * 25.4 / 72.0;
        let dy = ((y * 1_000_000.0).round() / 1_000_000.0 - y).abs() * height * 25.4 / 72.0;
        ensure!(
            dx <= 0.005 && dy <= 0.005,
            "Physical mark position exceeds supported coordinate precision"
        );
    }
    let points = corners.map(|(x, y)| {
        page.pixels_to_points(
            (x * 1_000_000.0).round() as i32,
            (y * 1_000_000.0).round() as i32,
            &config,
        )
    });
    let [origin, right, top] = points;
    let (origin, right, top) = (origin?, right?, top?);
    let physical_width =
        f64::from((right.0.value - origin.0.value).hypot(right.1.value - origin.1.value))
            * unit
            * 25.4
            / 72.0;
    let physical_height =
        f64::from((top.0.value - origin.0.value).hypot(top.1.value - origin.1.value)) * unit * 25.4
            / 72.0;
    ensure!(
        (physical_width - f64::from(geometry.width_mm)).abs() <= 0.01
            && (physical_height - f64::from(geometry.height_mm)).abs() <= 0.01,
        "Physical mark size exceeds supported coordinate precision"
    );
    Ok(PdfMatrix::new(
        right.0.value - origin.0.value,
        right.1.value - origin.1.value,
        top.0.value - origin.0.value,
        top.1.value - origin.1.value,
        origin.0.value,
        origin.1.value,
    ))
}

fn raster(placement: &Placement) -> Result<RgbaImage> {
    let (image, _, _) = imaging::decode(&placement.image)?;
    let mut image = image.into_rgba8();
    let max_width = (placement.geometry.width_mm / 25.4 * 400.0)
        .ceil()
        .clamp(1.0, 4096.0);
    let max_height = (placement.geometry.height_mm / 25.4 * 400.0)
        .ceil()
        .clamp(1.0, 4096.0);
    let scale = (max_width / image.width() as f32)
        .min(max_height / image.height() as f32)
        .min(1.0);
    if scale < 1.0 {
        // Resample premultiplied pixels to keep transparent colored edges free of dark fringes.
        for pixel in image.pixels_mut() {
            let alpha = pixel.0[3] as u16;
            for channel in &mut pixel.0[..3] {
                *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
            }
        }
        let width = (image.width() as f32 * scale).floor().max(1.0) as u32;
        let height = (image.height() as f32 * scale).floor().max(1.0) as u32;
        image =
            image::imageops::resize(&image, width, height, image::imageops::FilterType::Lanczos3);
        for pixel in image.pixels_mut() {
            let alpha = pixel.0[3] as u32;
            for channel in &mut pixel.0[..3] {
                *channel = (u32::from(*channel) * 255 + alpha / 2)
                    .checked_div(alpha)
                    .unwrap_or(0)
                    .min(255) as u8;
            }
        }
    }
    for pixel in image.pixels_mut() {
        pixel.0[3] = (pixel.0[3] as f32 * placement.opacity).round() as u8;
        if pixel.0[3] == 0 {
            pixel.0 = [0, 0, 0, 0];
        }
    }
    Ok(image)
}

/// Runs only on the existing PDF worker (or standalone CLI engine). No handles escape.
pub fn prepare(
    pdfium: &Pdfium,
    document: &PdfDocument<'_>,
    original: SourceSnapshot,
    placements: &[Placement],
) -> Result<PreparedExport> {
    if placements.is_empty() || placements.len() > 256 {
        bail!("Place between 1 and 256 marks before exporting");
    }
    let units = crate::pdf_units::mark_units(&original, document.pages().len() as usize)?;
    prepare_with_units(pdfium, document, original, placements, &units)
}

pub(crate) fn prepare_with_units(
    pdfium: &Pdfium,
    document: &PdfDocument<'_>,
    original: SourceSnapshot,
    placements: &[Placement],
    units: &[f64],
) -> Result<PreparedExport> {
    ensure!(
        units.len() == document.pages().len() as usize
            && units.iter().all(|unit| unit.is_finite() && *unit > 0.0),
        "PDF page metadata differs from the opened document"
    );
    if placements.is_empty() || placements.len() > 256 {
        bail!("Place between 1 and 256 marks before exporting");
    }
    let mut unique = HashSet::new();
    let mut bytes = 0usize;
    let mut pages: BTreeMap<usize, Vec<&Placement>> = BTreeMap::new();
    for placement in placements {
        if placement.page >= document.pages().len() as usize
            || !placement.opacity.is_finite()
            || !(0.0..=1.0).contains(&placement.opacity)
        {
            bail!("Invalid mark page or opacity");
        }
        if unique.insert(Arc::as_ptr(&placement.image) as usize) {
            bytes = bytes
                .checked_add(placement.image.len())
                .context("Mark image size overflow")?;
        }
        if bytes > 64 * 1024 * 1024 {
            bail!("Placed mark images exceed the 64-MiB budget");
        }
        pages.entry(placement.page).or_default().push(placement);
    }
    let mut sources = vec![original];
    let mut replacements = Vec::new();
    let mut expected_units = Vec::new();
    for (index, placements) in pages {
        let mut edited = pdfium.create_new_pdf()?;
        edited
            .pages_mut()
            .copy_page_from_document(document, index as i32, 0)?;
        {
            let mut page = edited.pages().get(0)?;
            for placement in placements {
                let transform = matrix(&page, placement.geometry, units[index])?;
                let image = DynamicImage::ImageRgba8(raster(placement)?);
                let mut object = PdfPageImageObject::new(&edited, &image)?;
                object.apply_matrix(transform)?;
                page.objects_mut().add_image_object(object)?;
            }
        }
        let bytes = edited.save_to_bytes()?;
        sources.push(SourceSnapshot {
            path: sources[0].path.clone(),
            bytes: Arc::new(bytes),
        });
        replacements.push(index);
        expected_units.push(units[index]);
    }
    Ok(PreparedExport {
        sources,
        replacements,
        units: expected_units,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn millimetre_bounds_follow_local_image_rotation_and_reject_invalid_placement() {
        let mut geometry = Geometry {
            x_mm: 180.0,
            y_mm: 200.0,
            width_mm: 45.0,
            height_mm: 20.0,
            quarter_turns: 1,
        };
        let rect = geometry
            .normalized(210.0 * 72.0 / 25.4, 297.0 * 72.0 / 25.4)
            .unwrap();
        assert!((rect.width - 20.0 / 210.0).abs() < 0.0001);
        assert!((rect.height - 45.0 / 297.0).abs() < 0.0001);
        geometry.quarter_turns = 0;
        assert!(
            geometry
                .normalized(210.0 * 72.0 / 25.4, 297.0 * 72.0 / 25.4)
                .is_err()
        );
        geometry.x_mm = f32::NAN;
        assert!(geometry.normalized(600.0, 800.0).is_err());
    }
}
