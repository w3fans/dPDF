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

fn image_pdf(pdfium: &Pdfium, bytes: &[u8], paper: (f32, f32), unit: f64) -> Result<Vec<u8>> {
    let physical = [f64::from(paper.0) * unit, f64::from(paper.1) * unit];
    if !physical
        .iter()
        .all(|v| v.is_finite() && *v > 0.0 && *v <= 14_400.0)
    {
        bail!("Unsupported physical image-page dimensions");
    }
    let (mut image, orientation, format) = decode(bytes)?;
    let (width, height, x, y) = fit((image.width(), image.height()), paper)?;
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
    let mut document = pdfium.create_new_pdf()?;
    {
        let mut page = document
            .pages_mut()
            .create_page_at_end(PdfPagePaperSize::new_custom(
                PdfPoints::new(paper.0),
                PdfPoints::new(paper.1),
            ))?;
        let mut object = if format == ImageFormat::Jpeg {
            if orientation == Orientation::NoTransforms && !resized {
                // Preserve original DCT data when no pixel transformation is required.
                PdfPageImageObject::new_from_jpeg_reader(&document, Cursor::new(bytes))?
            } else {
                let mut jpeg = Vec::new();
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 92)
                    .encode_image(&image.to_rgb8())?;
                PdfPageImageObject::new_from_jpeg_reader(&document, Cursor::new(jpeg))?
            }
        } else {
            PdfPageImageObject::new(&document, &DynamicImage::ImageRgba8(image.to_rgba8()))?
        };
        object.scale(width, height)?;
        object.translate(PdfPoints::new(x), PdfPoints::new(y))?;
        page.objects_mut().add_image_object(object)?;
    }
    crate::pdf_units::set_image_page_unit(
        document
            .save_to_bytes()
            .context("Create PDF page from image")?,
        unit,
    )
}

fn read_source(path: &Path) -> Result<(Vec<u8>, bool)> {
    let mut file = std::fs::File::open(path).with_context(|| format!("Read {}", path.display()))?;
    let mut header = [0; 16];
    let count = file.read(&mut header)?;
    let image = matches!(
        image::guess_format(&header[..count]),
        Ok(ImageFormat::Png | ImageFormat::Jpeg)
    );
    let mut bytes = header[..count].to_vec();
    if image {
        file.take(MAX_INPUT + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_INPUT {
            bail!("Page image exceeds 64 MiB; resize it first");
        }
    } else {
        file.read_to_end(&mut bytes)?;
    }
    Ok((bytes, image))
}

/// Called on the existing PDF worker, or by the standalone CLI before creating a UI.
/// The original image path remains attached to its PDF snapshot to prevent source overwrites.
pub fn prepare(
    pdfium: &Pdfium,
    primary: &PdfDocument<'_>,
    original: &SourceSnapshot,
    operation: &PageOperation,
    paths: &[PathBuf],
) -> Result<Vec<SourceSnapshot>> {
    let mut paper = (
        PdfPagePaperSize::a4().width().value,
        PdfPagePaperSize::a4().height().value,
    );
    let mut target = None;
    let mut units = None;
    if operation.kind == OperationKind::Replace && !paths.is_empty() {
        let selected = parse_selection(&operation.pages, primary.pages().len() as usize)?;
        if selected.len() != 1 {
            bail!("Choose one target page to replace");
        }
        let page = primary.pages().get(selected[0] as i32)?;
        paper = (page.width().value, page.height().value);
        target = Some(selected[0]);
    }
    paths
        .iter()
        .map(|path| {
            let (bytes, image) = read_source(path)?;
            let bytes = if image {
                let unit = if let Some(target) = target {
                    if units.is_none() {
                        let values = crate::pdf_units::load(original.bytes.as_slice())?;
                        if values.len() != primary.pages().len() as usize {
                            bail!("Replacement page metadata differs from the opened snapshot");
                        }
                        units = Some(values);
                    }
                    units
                        .as_ref()
                        .and_then(|values| values.get(target))
                        .copied()
                        .context("Missing replacement page unit")?
                } else {
                    1.0
                };
                image_pdf(pdfium, &bytes, paper, unit)
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
