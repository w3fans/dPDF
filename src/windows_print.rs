//! The Windows print dialog and GDI spooler run only in a separate print child.
//! PDFium is bound once in that child, never sharing the reader's live handles.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]
use crate::print_plan::{self, Layout, Selection};
use anyhow::{Context, Result, bail};
use pdfium_render::prelude::*;
use std::{
    ffi::{OsStr, OsString},
    io::Read,
    path::PathBuf,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::{GlobalFree, HWND},
    Graphics::Gdi::*,
    Storage::Xps::{AbortDoc, DOCINFOW, EndDoc, EndPage, StartDocW, StartPage},
    System::{
        Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize},
        Memory::*,
    },
    UI::{Controls::Dialogs::*, WindowsAndMessaging::*},
};

fn wide(value: &OsStr) -> Result<Vec<u16>> {
    #[cfg(target_os = "windows")]
    let mut result = {
        use std::os::windows::ffi::OsStrExt;
        value.encode_wide().collect::<Vec<_>>()
    };
    #[cfg(not(target_os = "windows"))]
    let mut result = value.to_string_lossy().encode_utf16().collect::<Vec<_>>();
    if result.contains(&0) {
        bail!("Native print text contains a NUL");
    }
    result.push(0);
    Ok(result)
}

struct Com;
impl Com {
    fn init() -> Result<Self> {
        let result = unsafe { CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32) };
        if result < 0 {
            bail!("Initialize native printing COM: HRESULT {result:#x}");
        }
        Ok(Self)
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

struct Owner(HWND);
impl Owner {
    fn new() -> Result<Self> {
        // PrintDlgEx requires a real owner HWND. This hidden top-level STATIC
        // belongs to the print child; the reader stays responsive and independent.
        let class = wide(OsStr::new("STATIC"))?;
        let title = wide(OsStr::new("dPDF print"))?;
        let window = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                400,
                200,
                null_mut(),
                null_mut(),
                null_mut(),
                null(),
            )
        };
        if window.is_null() {
            return Err(std::io::Error::last_os_error()).context("Create native print owner");
        }
        Ok(Self(window))
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.0);
        }
    }
}

struct Dialog {
    raw: PRINTDLGEXW,
    ranges: Vec<PRINTPAGERANGE>,
}
impl Drop for Dialog {
    fn drop(&mut self) {
        unsafe {
            if !self.raw.hDC.is_null() {
                DeleteDC(self.raw.hDC);
            }
            if !self.raw.hDevMode.is_null() {
                GlobalFree(self.raw.hDevMode);
            }
            if !self.raw.hDevNames.is_null() {
                GlobalFree(self.raw.hDevNames);
            }
        }
    }
}
impl Dialog {
    fn new(
        owner: HWND,
        count: usize,
        initial: &[(u32, u32)],
        printer: &str,
        current: usize,
    ) -> Result<Self> {
        let mut ranges = vec![PRINTPAGERANGE::default(); print_plan::MAX_RANGES];
        for (slot, &(a, b)) in ranges.iter_mut().zip(initial) {
            slot.nFromPage = a;
            slot.nToPage = b;
        }
        let mut flags = PD_RETURNDC | PD_USEDEVMODECOPIESANDCOLLATE | PD_NOSELECTION;
        if initial == [(current as u32 + 1, current as u32 + 1)] {
            flags |= PD_CURRENTPAGE;
        } else if !initial.is_empty() {
            flags |= PD_PAGENUMS;
        }
        let mut dialog = Self {
            raw: PRINTDLGEXW {
                lStructSize: std::mem::size_of::<PRINTDLGEXW>() as u32,
                hwndOwner: owner,
                Flags: flags,
                nPageRanges: initial.len() as u32,
                nMaxPageRanges: ranges.len() as u32,
                lpPageRanges: ranges.as_mut_ptr(),
                nMinPage: 1,
                nMaxPage: count.try_into()?,
                nCopies: 1,
                nStartPage: START_PAGE_GENERAL,
                ..Default::default()
            },
            ranges,
        };
        if !printer.is_empty() {
            let data = print_plan::printer_names(printer)?;
            unsafe {
                dialog.raw.hDevNames = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, data.len() * 2);
                if dialog.raw.hDevNames.is_null() {
                    bail!("Allocate native printer names");
                }
                let address = GlobalLock(dialog.raw.hDevNames);
                if address.is_null() {
                    bail!("Lock native printer names");
                }
                std::ptr::copy_nonoverlapping(data.as_ptr(), address.cast::<u16>(), data.len());
                GlobalUnlock(dialog.raw.hDevNames);
            }
        }
        Ok(dialog)
    }
    fn show(&mut self) -> Result<bool> {
        let result = unsafe { PrintDlgExW(&mut self.raw) };
        if result < 0 {
            bail!("Open Windows print dialog: HRESULT {result:#x}");
        }
        match self.raw.dwResultAction {
            PD_RESULT_PRINT => {
                if self.raw.hDC.is_null() {
                    bail!("The selected printer did not return a device context");
                }
                Ok(true)
            }
            PD_RESULT_CANCEL | PD_RESULT_APPLY => Ok(false),
            _ => bail!("Windows print dialog returned an unknown action"),
        }
    }
    fn names(&self) -> Result<(String, String)> {
        unsafe {
            let handle = self.raw.hDevNames;
            if handle.is_null() {
                bail!("The printer returned no device names");
            }
            let size = GlobalSize(handle);
            if !(8..=1024 * 1024).contains(&size) || !size.is_multiple_of(2) {
                bail!("Invalid native printer name buffer");
            }
            let address = GlobalLock(handle);
            if address.is_null() {
                bail!("Cannot read selected printer names");
            }
            let result = print_plan::printer_strings(std::slice::from_raw_parts(
                address.cast::<u16>(),
                size / 2,
            ));
            GlobalUnlock(handle);
            result
        }
    }
    fn pages(&self, count: usize, current: usize) -> Result<Vec<usize>> {
        if self.raw.Flags & PD_SELECTION != 0 {
            bail!("Native print selection is unavailable; choose pages instead");
        }
        if self.raw.Flags & PD_CURRENTPAGE != 0 {
            return print_plan::pages(count, current, Selection::Current);
        }
        if self.raw.Flags & PD_PAGENUMS == 0 {
            return print_plan::pages(count, current, Selection::All);
        }
        let selected = self
            .ranges
            .get(..self.raw.nPageRanges as usize)
            .context("Native print range buffer overflow")?
            .iter()
            .map(|r| (r.nFromPage, r.nToPage))
            .collect::<Vec<_>>();
        print_plan::pages(count, current, Selection::Ranges(&selected))
    }
}

struct SpoolJob {
    dc: HDC,
    active: bool,
}
impl Drop for SpoolJob {
    fn drop(&mut self) {
        if self.active {
            unsafe {
                AbortDoc(self.dc);
            }
        }
    }
}
impl SpoolJob {
    fn start(dc: HDC, name: &[u16], output: Option<&[u16]>) -> Result<Self> {
        let info = DOCINFOW {
            cbSize: std::mem::size_of::<DOCINFOW>() as i32,
            lpszDocName: name.as_ptr(),
            lpszOutput: output.map_or(null(), |v| v.as_ptr()),
            lpszDatatype: null(),
            fwType: 0,
        };
        if unsafe { StartDocW(dc, &info) } <= 0 {
            return Err(std::io::Error::last_os_error()).context("Start Windows print job");
        }
        Ok(Self { dc, active: true })
    }
    fn draw(&self, page: &PdfPage<'_>, fit: bool, unit: f64) -> Result<serde_json::Value> {
        let dc = self.dc;
        let plan = unsafe {
            Layout::new(
                (
                    f64::from(page.width().value) * unit,
                    f64::from(page.height().value) * unit,
                ),
                (
                    GetDeviceCaps(dc, HORZRES as i32),
                    GetDeviceCaps(dc, VERTRES as i32),
                ),
                (
                    GetDeviceCaps(dc, LOGPIXELSX as i32),
                    GetDeviceCaps(dc, LOGPIXELSY as i32),
                ),
                fit,
            )?
        };
        // Explicit dimensions keep even extremely tall/wide pages inside both
        // allocation caps. A sub-pixel aspect rounding is below print resolution.
        let bitmap = page.render_with_config(
            &PdfRenderConfig::new()
                .set_target_size(plan.raster_width, plan.raster_height)
                .use_print_quality(true)
                .render_annotations(true)
                .render_form_data(true),
        )?;
        let (width, height) = (bitmap.width(), bitmap.height());
        if width <= 0 || height <= 0 || width as usize * height as usize > print_plan::MAX_PIXELS {
            bail!("Native print raster exceeds its allocation budget");
        }
        let mut pixels = bitmap.as_rgba_bytes();
        if pixels.len() != width as usize * height as usize * 4 {
            bail!("Invalid native print raster");
        }
        for pixel in pixels.as_chunks_mut::<4>().0 {
            let [r, g, b, a] = *pixel;
            let white = |v: u8| {
                ((u32::from(v) * u32::from(a) + 255 * (255 - u32::from(a)) + 127) / 255) as u8
            };
            *pixel = [white(b), white(g), white(r), 0]; // Top-down 32-bit BI_RGB BGRX.
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: pixels.len() as u32,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            if StartPage(dc) <= 0 {
                bail!("Start native printed page");
            }
            if SetMapMode(dc, MM_TEXT) == 0
                || SetStretchBltMode(dc, HALFTONE) == 0
                || SetBrushOrgEx(dc, 0, 0, null_mut()) == 0
            {
                bail!("Configure native printed page");
            }
            let lines = StretchDIBits(
                dc,
                plan.x,
                plan.y,
                plan.width,
                plan.height,
                0,
                0,
                width,
                height,
                pixels.as_ptr().cast(),
                &info,
                DIB_RGB_COLORS,
                SRCCOPY,
            );
            if lines == 0 || lines == GDI_ERROR {
                bail!("Render native printed page");
            }
            if EndPage(dc) <= 0 {
                bail!("Finish native printed page");
            }
        }
        let mut metric = serde_json::to_value(plan)?;
        unsafe {
            for (name, cap) in [
                ("dpi_x", LOGPIXELSX),
                ("dpi_y", LOGPIXELSY),
                ("physical_width", PHYSICALWIDTH),
                ("physical_height", PHYSICALHEIGHT),
                ("offset_x", PHYSICALOFFSETX),
                ("offset_y", PHYSICALOFFSETY),
            ] {
                metric[name] = GetDeviceCaps(dc, cap as i32).into();
            }
        }
        Ok(metric)
    }
    fn finish(mut self) -> Result<()> {
        if unsafe { EndDoc(self.dc) } <= 0 {
            bail!("Finish Windows print job");
        }
        self.active = false;
        Ok(())
    }
}

fn cancelled() {
    println!(
        "{}",
        serde_json::json!({"status":"cancelled","pages_drawn":[],"metrics":[]})
    );
}

pub fn run_cli(mut args: impl Iterator<Item = OsString>, diagnostic: bool) -> Result<()> {
    let source = PathBuf::from(args.next().context("Missing original path")?);
    let diagnostic_output = if diagnostic {
        Some(PathBuf::from(
            args.next().context("Missing diagnostic output PDF")?,
        ))
    } else {
        None
    };
    let current: usize = args
        .next()
        .context("Missing current page")?
        .to_string_lossy()
        .parse()?;
    let range = args
        .next()
        .context("Missing print range")?
        .into_string()
        .map_err(|_| anyhow::anyhow!("Invalid range"))?;
    let fit = match args.next().as_deref() {
        Some(v) if v == "fit" => true,
        Some(v) if v == "actual" => false,
        _ => bail!("Choose fit or actual scaling"),
    };
    let printer = args
        .next()
        .unwrap_or_default()
        .into_string()
        .map_err(|_| anyhow::anyhow!("Invalid printer name"))?;
    if args.next().is_some() {
        bail!("Too many native print arguments");
    }
    if diagnostic && std::env::var("DPDF_TEST_NATIVE_PRINT").as_deref() != Ok("1") {
        bail!(
            "Diagnostic Windows printing requires DPDF_TEST_NATIVE_PRINT=1 and only permits Microsoft Print to PDF"
        );
    }
    if let Some(output) = &diagnostic_output {
        crate::storage::ensure_separate_output(output, &[&source])?;
    }
    let mut bytes = Vec::new();
    let mut input: Box<dyn Read> = if diagnostic {
        Box::new(std::fs::File::open(&source)?)
    } else {
        Box::new(std::io::stdin())
    };
    input
        .by_ref()
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 * 1024 {
        bail!("Native printing supports PDF snapshots up to 256 MiB");
    }
    let units = if fit {
        None
    } else {
        Some(crate::pdf_units::load(&bytes)?)
    };
    let pdfium = crate::pdf::bind_pdfium()?;
    let pdf = pdfium.load_pdf_from_byte_vec(bytes, None)?;
    let count = pdf.pages().len() as usize;
    if units.as_ref().is_some_and(|units| units.len() != count) {
        bail!("Physical page metadata differs from the printed snapshot");
    }
    print_plan::pages(count, current, Selection::All)?;
    let ranges = print_plan::ranges(&range, count)?;
    let _com = Com::init()?;
    let owner = Owner::new()?;
    let mut dialog = Dialog::new(
        owner.0,
        count,
        &ranges,
        if diagnostic {
            "Microsoft Print to PDF"
        } else {
            printer.trim()
        },
        current,
    )?;
    if !dialog.show()? {
        cancelled();
        return Ok(());
    }
    let selected = dialog.pages(count, current)?;
    let (device, port) = dialog.names()?;
    // This check precedes StartDoc: a diagnostic can never submit to a physical
    // printer, even if the user changes the selection in the native dialog.
    if diagnostic && !device.eq_ignore_ascii_case("Microsoft Print to PDF") {
        bail!("Diagnostic printing refused the selected device; choose Microsoft Print to PDF");
    }
    let output = if diagnostic_output.is_some() {
        diagnostic_output
    } else if dialog.raw.Flags & PD_PRINTTOFILE != 0
        || device.eq_ignore_ascii_case("Microsoft Print to PDF")
        || port.eq_ignore_ascii_case("PORTPROMPT:")
        || port.eq_ignore_ascii_case("FILE:")
    {
        let extension = if device.eq_ignore_ascii_case("Microsoft Print to PDF") {
            "pdf"
        } else {
            "prn"
        };
        let name = source
            .file_stem()
            .unwrap_or(OsStr::new("document"))
            .to_string_lossy();
        let selected = rfd::FileDialog::new()
            .set_title("Save printed copy")
            .set_file_name(format!("{name}-printed.{extension}"))
            .save_file();
        if selected.is_none() {
            cancelled();
            return Ok(());
        }
        selected
    } else {
        None
    };
    if let Some(output) = &output {
        crate::storage::ensure_separate_output(output, &[&source])?;
    }
    let output_text = output.as_deref().map(|p| wide(p.as_os_str())).transpose()?;
    let name = wide(source.file_name().unwrap_or(OsStr::new("dPDF document")))?;
    // Recheck immediately before handing a file path to the spooler.
    if let Some(output) = &output {
        crate::storage::ensure_separate_output(output, &[&source])?;
    }
    let job = SpoolJob::start(dialog.raw.hDC, &name, output_text.as_deref())?;
    let mut metrics = Vec::with_capacity(selected.len());
    for &index in &selected {
        let unit = units.as_ref().map_or(1.0, |units| units[index]);
        metrics.push(job.draw(&pdf.pages().get(index as i32)?, fit, unit)?);
    }
    job.finish()?;
    println!(
        "{}",
        serde_json::json!({"status":"submitted","pages_drawn":selected,"metrics":metrics,"printer":device})
    );
    Ok(())
}
