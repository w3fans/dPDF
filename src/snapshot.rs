//! Optional diagnostics capture only the application's own window.
use std::{io::Write, path::Path};

pub fn write(window: &slint::Window, path: &Path) -> anyhow::Result<()> {
    let pixels = window.take_snapshot()?;
    let mut output = std::io::BufWriter::new(std::fs::File::create(path)?);
    write!(output, "P6\n{} {}\n255\n", pixels.width(), pixels.height())?;
    for pixel in pixels.as_slice() {
        output.write_all(&[pixel.r, pixel.g, pixel.b])?;
    }
    output.flush()?;
    Ok(())
}
