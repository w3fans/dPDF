//! Platform-independent print selection, physical geometry and bounded Unicode
//! printer-name buffers. The actual Windows driver/dialog still needs native tests.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]
use anyhow::{Context, Result, bail};

pub const MAX_RANGES: usize = 64;
pub const MAX_PIXELS: usize = 16_000_000;

pub fn ranges(text: &str, count: usize) -> Result<Vec<(u32, u32)>> {
    let valid = crate::platform::validate_range(text, count)?;
    if valid.is_empty() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for part in valid.split(',') {
        let (a, b) = part.split_once('-').unwrap_or((part, part));
        result.push((a.parse()?, b.parse()?));
    }
    if result.len() > MAX_RANGES {
        bail!("Native printing accepts up to {MAX_RANGES} page ranges");
    }
    Ok(result)
}

pub enum Selection<'a> {
    All,
    Current,
    Ranges(&'a [(u32, u32)]),
}

pub fn pages(count: usize, current: usize, selection: Selection<'_>) -> Result<Vec<usize>> {
    if count == 0 || count > 100_000 || current >= count {
        bail!("Invalid native print page selection");
    }
    match selection {
        Selection::All => Ok((0..count).collect()),
        Selection::Current => Ok(vec![current]),
        Selection::Ranges(ranges) => {
            if ranges.is_empty() || ranges.len() > MAX_RANGES {
                bail!("Invalid native print page ranges");
            }
            let mut selected = vec![false; count];
            for &(start, end) in ranges {
                if start == 0 || start > end || end as usize > count {
                    bail!("Printed page ranges must be within 1–{count}");
                }
                selected[start as usize - 1..end as usize].fill(true);
            }
            Ok(selected
                .iter()
                .enumerate()
                .filter_map(|(i, &yes)| yes.then_some(i))
                .collect())
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct Layout {
    pub source_width: f64,
    pub source_height: f64,
    pub printable_width: f64,
    pub printable_height: f64,
    pub draw_width: f64,
    pub draw_height: f64,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub raster_width: i32,
    pub raster_height: i32,
}

impl Layout {
    /// GDI MM_TEXT's origin is the printable area's origin. Center there, using
    /// separate device X/Y DPI so non-square device pixels do not distort paper size.
    pub fn new(
        page: (f64, f64),
        printable: (i32, i32),
        dpi: (i32, i32),
        fit: bool,
    ) -> Result<Self> {
        let (pw, ph) = page;
        let (w, h) = printable;
        let (dx, dy) = dpi;
        if ![pw, ph].iter().all(|v| v.is_finite() && *v > 0.0)
            || [w, h, dx, dy].iter().any(|v| *v <= 0)
        {
            bail!("Invalid native print geometry");
        }
        let (cw, ch) = (
            f64::from(w) * 72.0 / f64::from(dx),
            f64::from(h) * 72.0 / f64::from(dy),
        );
        let scale = if fit { (cw / pw).min(ch / ph) } else { 1.0 };
        let (dw, dh) = (pw * scale, ph * scale);
        let (width, height) = (
            (dw / 72.0 * f64::from(dx)).round().max(1.0),
            (dh / 72.0 * f64::from(dy)).round().max(1.0),
        );
        if ![dw, dh, width, height]
            .iter()
            .all(|v| v.is_finite() && *v > 0.0)
            || width > f64::from(i32::MAX)
            || height > f64::from(i32::MAX)
        {
            bail!("Page dimensions exceed native printing limits");
        }
        let density = (300.0 / 72.0f64)
            .min(4096.0 / dw)
            .min(8192.0 / dh)
            .min((MAX_PIXELS as f64 / dw / dh).sqrt());
        let (rw, rh) = (
            (dw * density).floor().max(1.0) as i32,
            (dh * density).floor().max(1.0) as i32,
        );
        Ok(Self {
            source_width: pw,
            source_height: ph,
            printable_width: cw,
            printable_height: ch,
            draw_width: dw,
            draw_height: dh,
            x: ((f64::from(w) - width) / 2.0).round() as i32,
            y: ((f64::from(h) - height) / 2.0).round() as i32,
            width: width as i32,
            height: height as i32,
            raster_width: rw,
            raster_height: rh,
        })
    }
}

/// DEVNAMES offsets are UTF-16 code units from the beginning of the allocation,
/// including its four-WORD header, not bytes and not Unicode scalar values.
pub fn printer_names(printer: &str) -> Result<Vec<u16>> {
    let mut data = vec![0u16; 4];
    for (index, value) in ["WINSPOOL", printer, ""].iter().enumerate() {
        if value.contains('\0') {
            bail!("Printer name contains a NUL");
        }
        data[index] = data.len().try_into().context("Printer name is too long")?;
        data.extend(value.encode_utf16());
        data.push(0);
    }
    if data.len() > u16::MAX as usize {
        bail!("Printer name is too long");
    }
    Ok(data)
}

pub fn printer_strings(data: &[u16]) -> Result<(String, String)> {
    if data.len() < 4 {
        bail!("Native printer names are truncated");
    }
    let get = |offset: u16| -> Result<String> {
        let start = usize::from(offset);
        if start < 4 {
            bail!("Native printer name offset is invalid");
        }
        let tail = data
            .get(start..)
            .context("Native printer name offset is outside its allocation")?;
        let end = tail
            .iter()
            .position(|v| *v == 0)
            .context("Native printer name is unterminated")?;
        String::from_utf16(&tail[..end]).context("Native printer name is invalid Unicode")
    };
    Ok((get(data[1])?, get(data[2])?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disjoint_ranges_current_and_overlaps_select_original_pages_once() {
        assert_eq!(pages(10, 6, Selection::Current).unwrap(), [6]);
        assert_eq!(
            pages(10, 0, Selection::All).unwrap(),
            (0..10).collect::<Vec<_>>()
        );
        let selected_ranges = ranges("7,1-3,2-5,10", 10).unwrap();
        assert_eq!(
            pages(10, 6, Selection::Ranges(&selected_ranges)).unwrap(),
            [0, 1, 2, 3, 4, 6, 9]
        );
        for invalid in [&[][..], &[(0, 1)], &[(4, 2)], &[(1, 11)]] {
            assert!(pages(10, 0, Selection::Ranges(invalid)).is_err());
        }
        assert!(pages(10, 10, Selection::Current).is_err());
        assert!(ranges("0", 10).is_err());
    }
    #[test]
    fn physical_size_survives_non_square_dpi_and_fit_centers_in_printable_area() {
        let actual = Layout::new((144.0, 72.0), (2400, 6000), (300, 600), false).unwrap();
        assert_eq!((actual.width, actual.height), (600, 600));
        assert_eq!((actual.x, actual.y), (900, 2700));
        let fit = Layout::new((144.0, 72.0), (2400, 6000), (300, 600), true).unwrap();
        assert_eq!((fit.width, fit.height, fit.x, fit.y), (2400, 2400, 0, 1800));
        let clipped = Layout::new((1440.0, 1440.0), (2400, 6000), (300, 600), false).unwrap();
        assert!(clipped.x < 0 && clipped.y < 0); // Actual size clips rather than silently shrinking.
        for page in [
            (595.0, 842.0),
            (842.0, 595.0),
            (1.0, 100_000.0),
            (100_000.0, 1.0),
        ] {
            for fit in [false, true] {
                let plan = Layout::new(page, (4800, 6000), (600, 600), fit).unwrap();
                assert!(plan.raster_width > 0 && plan.raster_width <= 4096);
                assert!(plan.raster_height > 0 && plan.raster_height <= 8192);
                assert!(plan.raster_width as usize * plan.raster_height as usize <= MAX_PIXELS);
            }
        }
        assert!(Layout::new((f64::NAN, 10.0), (100, 100), (300, 300), true).is_err());
        assert!(Layout::new((100.0, 100.0), (100, 100), (0, 300), true).is_err());
    }
    #[test]
    fn unicode_printer_buffers_validate_offsets_lengths_and_terminators() {
        let data = printer_names("Žig 🖨 Office").unwrap();
        assert_eq!(
            printer_strings(&data).unwrap(),
            ("Žig 🖨 Office".into(), "".into())
        );
        for offset in [0, 3, u16::MAX] {
            let mut broken = data.clone();
            broken[1] = offset;
            assert!(printer_strings(&broken).is_err());
        }
        let mut broken = data.clone();
        broken.pop();
        assert!(printer_strings(&broken).is_err());
        assert!(printer_names("Office\0ignored").is_err());
        assert!(printer_names(&"x".repeat(65535)).is_err());
    }
}
