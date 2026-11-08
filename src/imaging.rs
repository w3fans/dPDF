//! Shared bounded PNG/JPEG decoding and phone-photo orientation.
use anyhow::{Context, Result, bail};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, metadata::Orientation};
use std::io::Cursor;

pub const MAX_INPUT: u64 = 64 * 1024 * 1024;
pub const MAX_PIXELS: u64 = 32_000_000;

pub fn decode(bytes: &[u8]) -> Result<(DynamicImage, Orientation, ImageFormat)> {
    let format = image::guess_format(bytes).context("Recognize PNG or JPEG page image")?;
    if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
        bail!("Page images must be PNG or JPEG");
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(16_000);
    limits.max_image_height = Some(16_000);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().context("Read page image header")?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > MAX_PIXELS
        || decoder.total_bytes() > 128 * 1024 * 1024
    {
        bail!("Page image exceeds the 32-megapixel / 128-MiB decoded limit; resize it first");
    }
    let orientation = decoder.orientation().context("Read image orientation")?;
    let mut image = DynamicImage::from_decoder(decoder).context("Decode page image")?;
    image.apply_orientation(orientation);
    Ok((image, orientation, format))
}
