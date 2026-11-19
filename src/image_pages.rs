//! Bounded scan/photo import. Physical page size comes from the document, never image DPI metadata.
use crate::imaging::{MAX_INPUT, decode};
use crate::{
    assembly::{OperationKind, PageOperation, SourceSnapshot},
    pages::parse_selection,
};
use anyhow::{Context, Result, bail};
use image::{DynamicImage, ImageFormat, metadata::Orientation};
use pdfium_render::prelude::*;
use std::{
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::Arc,
};

fn fit(image: (u32, u32), paper: (f32, f32)) -> Result<(f32, f32, f32, f32)> {
    let (width, height) = paper;
    if image.0 == 0
        || image.1 == 0
        || !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || width > 14_400.0
        || height > 14_400.0
    {
        bail!("Unsupported image or PDF page dimensions");
    }
    let scale = (width / image.0 as f32).min(height / image.1 as f32);
    let drawn_width = image.0 as f32 * scale;
    let drawn_height = image.1 as f32 * scale;
    Ok((
        drawn_width,
        drawn_height,
        (width - drawn_width) / 2.0,
        (height - drawn_height) / 2.0,
    ))
}

/// Native geometry reduced to values before leaving the PDF owner.
pub(crate) struct Target {
    paper: (f32, f32),
    replacement: Option<usize>,
    pages: usize,
}
impl Target {
    pub(crate) fn new(
        primary: &PdfDocument<'_>,
        operation: &PageOperation,
        has_inputs: bool,
    ) -> Result<Self> {
        let pages = primary.pages().len() as usize;
        let mut target = Self {
            paper: (
                PdfPagePaperSize::a4().width().value,
                PdfPagePaperSize::a4().height().value,
            ),
            replacement: None,
            pages,
        };
        if operation.kind == OperationKind::Replace && has_inputs {
            let selected = parse_selection(&operation.pages, pages)?;
            if selected.len() != 1 {
                bail!("Choose one target page to replace");
            }
            let page = primary.pages().get(selected[0] as i32)?;
            target.paper = (page.width().value, page.height().value);
            target.replacement = Some(selected[0]);
        }
        Ok(target)
    }
    pub(crate) fn unit(
        &self,
        original: &SourceSnapshot,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<f64> {
        let Some(target) = self.replacement else {
            return Ok(1.0);
        };
        let metadata = crate::assembly::inspect_snapshot_cancellable(&original.bytes, cancel)?;
        let values = crate::pdf_units::from_metadata(&metadata, self.pages)?;
        values
            .get(target)
            .copied()
            .context("Missing replacement page unit")
    }
    pub(crate) fn prepare(
        &self,
        bytes: Vec<u8>,
        unit: f64,
        cancel: Option<&std::sync::atomic::AtomicBool>,
    ) -> Result<PreparedImage> {
        crate::background_jobs::check_cancelled(cancel)?;
        let physical = [
            f64::from(self.paper.0) * unit,
            f64::from(self.paper.1) * unit,
        ];
        if !physical
            .iter()
            .all(|v| v.is_finite() && *v > 0.0 && *v <= 14_400.0)
        {
            bail!("Unsupported physical image-page dimensions");
        }
        let (mut image, orientation, format) = decode(&bytes)?;
        crate::background_jobs::check_cancelled(cancel)?;
        let (width, height, x, y) = fit((image.width(), image.height()), self.paper)?;
        let max_width = (f64::from(width) * unit * 300.0 / 72.0)
            .ceil()
            .clamp(1.0, 4096.0) as u32;
        let max_height = (f64::from(height) * unit * 300.0 / 72.0)
            .ceil()
            .clamp(1.0, 4096.0) as u32;
        let resized = image.width() > max_width || image.height() > max_height;
        if resized {
            image = image.resize(max_width, max_height, image::imageops::FilterType::Lanczos3);
        }
        crate::background_jobs::check_cancelled(cancel)?;
        let pixels = if format == ImageFormat::Jpeg {
            if orientation == Orientation::NoTransforms && !resized {
                Pixels::Jpeg(bytes) // Preserve original DCT data without a pixel transformation.
            } else {
                let mut jpeg = Vec::new();
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 92)
                    .encode_image(&image.to_rgb8())?;
                Pixels::Jpeg(jpeg)
            }
        } else {
            Pixels::Rgba(image.to_rgba8())
        };
        crate::background_jobs::check_cancelled(cancel)?;
        Ok(PreparedImage {
            paper: self.paper,
            geometry: (width, height, x, y),
            unit,
            pixels,
        })
    }
}

enum Pixels {
    Jpeg(Vec<u8>),
    Rgba(image::RgbaImage),
}
pub(crate) struct PreparedImage {
    paper: (f32, f32),
    geometry: (f32, f32, f32, f32),
    unit: f64,
    pixels: Pixels,
}
impl PreparedImage {
    /// Native object construction only; no file reads, image decoding or helpers.
    pub(crate) fn create(self, pdfium: &Pdfium) -> Result<(Vec<u8>, f64)> {
        let mut document = pdfium.create_new_pdf()?;
        {
            let mut page =
                document
                    .pages_mut()
                    .create_page_at_end(PdfPagePaperSize::new_custom(
                        PdfPoints::new(self.paper.0),
                        PdfPoints::new(self.paper.1),
                    ))?;
            let mut object = match self.pixels {
                Pixels::Jpeg(jpeg) => {
                    PdfPageImageObject::new_from_jpeg_reader(&document, Cursor::new(jpeg))?
                }
                Pixels::Rgba(image) => {
                    PdfPageImageObject::new(&document, &DynamicImage::ImageRgba8(image))?
                }
            };
            let (width, height, x, y) = self.geometry;
            object.scale(width, height)?;
            object.translate(PdfPoints::new(x), PdfPoints::new(y))?;
            page.objects_mut().add_image_object(object)?;
        }
        Ok((
            document
                .save_to_bytes()
                .context("Create PDF page from image")?,
            self.unit,
        ))
    }
}

pub(crate) fn read_source(
    path: &Path,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<(Vec<u8>, bool)> {
    crate::background_jobs::check_cancelled(cancel)?;
    let metadata = std::fs::metadata(path).with_context(|| format!("Read {}", path.display()))?;
    if !metadata.is_file() {
        bail!("Page input must be a regular file");
    }
    let mut file = std::fs::File::open(path).with_context(|| format!("Read {}", path.display()))?;
    let length = file.metadata()?.len();
    let mut header = [0; 16];
    let count = file.read(&mut header)?;
    let image = matches!(
        image::guess_format(&header[..count]),
        Ok(ImageFormat::Png | ImageFormat::Jpeg)
    );
    if image && length > MAX_INPUT {
        bail!("Page image exceeds 64 MiB; resize it first");
    }
    let mut bytes = header[..count].to_vec();
    loop {
        crate::background_jobs::check_cancelled(cancel)?;
        let mut chunk = [0; 64 * 1024];
        let count = file.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        let new_length = bytes
            .len()
            .checked_add(count)
            .context("Page input size overflow")?;
        if new_length as u64 > length {
            bail!("Page input changed while reading");
        }
        bytes
            .try_reserve(count)
            .context("Reserve page input snapshot memory")?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    if bytes.len() as u64 != length {
        bail!("Page input changed while reading");
    }
    Ok((bytes, image))
}

/// Standalone CLI path; the GUI uses the same phases across its owned workers.
/// The original image path remains attached to its PDF snapshot for alias protection.
pub fn prepare(
    pdfium: &Pdfium,
    primary: &PdfDocument<'_>,
    original: &SourceSnapshot,
    operation: &PageOperation,
    paths: &[PathBuf],
) -> Result<Vec<SourceSnapshot>> {
    let target = Target::new(primary, operation, !paths.is_empty())?;
    let mut unit = None;
    paths
        .iter()
        .map(|path| {
            let (bytes, image) = read_source(path, None)?;
            let bytes = if image {
                let value = match unit {
                    Some(unit) => unit,
                    None => {
                        let value = target.unit(original, None)?;
                        unit = Some(value);
                        value
                    }
                };
                let (bytes, unit) = target.prepare(bytes, value, None)?.create(pdfium)?;
                crate::pdf_units::set_image_page_unit(bytes, unit)
                    .with_context(|| format!("Import page image {}", path.display()))?
            } else {
                bytes
            };
            Ok(SourceSnapshot {
                path: path.clone(),
                bytes: Arc::new(bytes),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_fit_preserves_aspect_and_centers_without_using_pixel_dpi() {
        assert_eq!(
            fit((400, 200), (600.0, 800.0)).unwrap(),
            (600.0, 300.0, 0.0, 250.0)
        );
        assert_eq!(
            fit((4000, 2000), (600.0, 800.0)).unwrap(),
            fit((400, 200), (600.0, 800.0)).unwrap()
        );
        assert!(fit((0, 200), (600.0, 800.0)).is_err());
        assert!(fit((200, 200), (f32::NAN, 800.0)).is_err());
    }

    #[test]
    fn jpeg_phone_orientation_is_applied_before_sizing() {
        let original = image::RgbImage::from_pixel(40, 20, image::Rgb([40, 80, 120]));
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
            .encode_image(&original)
            .unwrap();
        // EXIF little-endian TIFF, IFD0 orientation=6 (90 degrees clockwise).
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut oriented = vec![0xff, 0xd8, 0xff, 0xe1];
        oriented.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        oriented.extend_from_slice(exif);
        oriented.extend_from_slice(&jpeg[2..]);
        let (image, orientation, _) = decode(&oriented).unwrap();
        assert_eq!(orientation, Orientation::Rotate90);
        assert_eq!((image.width(), image.height()), (20, 40));
    }
}
