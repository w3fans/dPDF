//! Linux's native GTK print operation, isolated in a child process. GTK loads only
//! when printing, so reader startup and its Slint event loop remain independent.
use anyhow::{Context, Result, bail};
use libloading::Library;
use pdfium_render::prelude::*;
use std::{
    ffi::{CStr, CString, OsString, c_char, c_int, c_void},
    io::Read,
    path::{Path, PathBuf},
};
mod direct_pdf;

type Obj = *mut c_void;
type Callback = unsafe extern "C" fn();

macro_rules! api {
    ($name:ident,$soname:literal,{$($symbol:ident:$signature:ty),+$(,)?})=>{
        struct $name {$($symbol:$signature,)+ _library:std::mem::ManuallyDrop<Library>}
        impl $name {fn load()->Result<Self>{
            // These stable platform ABI signatures are confined to this module.
            // GTK registers global GObject types: retain the libraries until this child exits.
            let library=unsafe{Library::new($soname)}.context(concat!("Load native printing support: ",$soname))?;
            Ok(Self{$($symbol:unsafe{*library.get::<$signature>(concat!(stringify!($symbol),"\0").as_bytes())?},)+_library:std::mem::ManuallyDrop::new(library)})
        }}
    }
}
api!(Gtk,"libgtk-3.so.0",{
    gtk_init_check:unsafe extern "C" fn(*mut c_int,*mut *mut *mut c_char)->c_int,
    gtk_settings_get_default:unsafe extern "C" fn()->Obj,
    gtk_settings_set_string_property:unsafe extern "C" fn(Obj,*const c_char,*const c_char,*const c_char),
    gtk_print_unix_dialog_new:unsafe extern "C" fn(*const c_char,Obj)->Obj,
    gtk_print_unix_dialog_set_settings:unsafe extern "C" fn(Obj,Obj),
    gtk_print_unix_dialog_get_settings:unsafe extern "C" fn(Obj)->Obj,
    gtk_print_unix_dialog_set_current_page:unsafe extern "C" fn(Obj,c_int),
    gtk_print_unix_dialog_set_embed_page_setup:unsafe extern "C" fn(Obj,c_int),
    gtk_print_unix_dialog_set_manual_capabilities:unsafe extern "C" fn(Obj,c_int),
    gtk_print_unix_dialog_set_page_setup:unsafe extern "C" fn(Obj,Obj),
    gtk_print_unix_dialog_get_page_setup:unsafe extern "C" fn(Obj)->Obj,
    gtk_print_unix_dialog_get_selected_printer:unsafe extern "C" fn(Obj)->Obj,
    gtk_dialog_run:unsafe extern "C" fn(Obj)->c_int,
    gtk_widget_hide:unsafe extern "C" fn(Obj),
    gtk_widget_destroy:unsafe extern "C" fn(Obj),
    gtk_printer_accepts_pdf:unsafe extern "C" fn(Obj)->c_int,
    gtk_printer_get_name:unsafe extern "C" fn(Obj)->*const c_char,
    gtk_print_job_new:unsafe extern "C" fn(*const c_char,Obj,Obj,Obj)->Obj,
    gtk_print_job_get_settings:unsafe extern "C" fn(Obj)->Obj,
    gtk_print_job_set_track_print_status:unsafe extern "C" fn(Obj,c_int),
    gtk_print_job_get_status:unsafe extern "C" fn(Obj)->c_int,
    gtk_print_job_get_scale:unsafe extern "C" fn(Obj)->f64,
    gtk_print_job_get_num_copies:unsafe extern "C" fn(Obj)->c_int,
    gtk_print_job_get_n_up:unsafe extern "C" fn(Obj)->u32,
    gtk_print_job_get_page_set:unsafe extern "C" fn(Obj)->c_int,
    gtk_print_job_get_collate:unsafe extern "C" fn(Obj)->c_int,
    gtk_print_settings_get_n_copies:unsafe extern "C" fn(Obj)->c_int,
    gtk_print_job_get_reverse:unsafe extern "C" fn(Obj)->c_int,
    gtk_print_settings_unset:unsafe extern "C" fn(Obj,*const c_char),
    gtk_print_job_set_source_fd:unsafe extern "C" fn(Obj,c_int,*mut *mut GError)->c_int,
    gtk_print_job_send:unsafe extern "C" fn(Obj,unsafe extern "C" fn(Obj,Obj,*const GError),Obj,Option<unsafe extern "C" fn(Obj)>),
    gtk_page_setup_get_orientation:unsafe extern "C" fn(Obj)->c_int,
    gtk_print_operation_get_default_page_setup:unsafe extern "C" fn(Obj)->Obj,
    gtk_print_operation_new:unsafe extern "C" fn()->Obj,
    gtk_print_settings_new:unsafe extern "C" fn()->Obj,
    gtk_print_settings_set:unsafe extern "C" fn(Obj,*const c_char,*const c_char),
    gtk_print_settings_get:unsafe extern "C" fn(Obj,*const c_char)->*const c_char,
    gtk_print_operation_set_print_settings:unsafe extern "C" fn(Obj,Obj),
    gtk_print_operation_get_print_settings:unsafe extern "C" fn(Obj)->Obj,
    gtk_print_operation_set_n_pages:unsafe extern "C" fn(Obj,c_int),
    gtk_print_operation_set_current_page:unsafe extern "C" fn(Obj,c_int),
    gtk_print_operation_set_unit:unsafe extern "C" fn(Obj,c_int),
    gtk_print_operation_set_job_name:unsafe extern "C" fn(Obj,*const c_char),
    gtk_print_operation_set_embed_page_setup:unsafe extern "C" fn(Obj,c_int),
    gtk_print_operation_set_allow_async:unsafe extern "C" fn(Obj,c_int),
    gtk_page_setup_new:unsafe extern "C" fn()->Obj,
    gtk_paper_size_new:unsafe extern "C" fn(*const c_char)->Obj,
    gtk_paper_size_free:unsafe extern "C" fn(Obj),
    gtk_page_setup_set_paper_size:unsafe extern "C" fn(Obj,Obj),
    gtk_page_setup_set_top_margin:unsafe extern "C" fn(Obj,f64,c_int),
    gtk_page_setup_set_bottom_margin:unsafe extern "C" fn(Obj,f64,c_int),
    gtk_page_setup_set_left_margin:unsafe extern "C" fn(Obj,f64,c_int),
    gtk_page_setup_set_right_margin:unsafe extern "C" fn(Obj,f64,c_int),
    gtk_print_operation_set_default_page_setup:unsafe extern "C" fn(Obj,Obj),
    gtk_print_operation_run:unsafe extern "C" fn(Obj,c_int,Obj,*mut *mut GError)->c_int,
    gtk_print_operation_cancel:unsafe extern "C" fn(Obj),
    gtk_print_context_get_cairo_context:unsafe extern "C" fn(Obj)->Obj,
    gtk_print_context_get_width:unsafe extern "C" fn(Obj)->f64,
    gtk_print_context_get_height:unsafe extern "C" fn(Obj)->f64,
});
api!(Gobject,"libgobject-2.0.so.0",{
    g_signal_connect_data:unsafe extern "C" fn(Obj,*const c_char,Callback,Obj,Option<unsafe extern "C" fn(Obj,Obj)>,u32)->std::ffi::c_ulong,
    g_signal_handler_disconnect:unsafe extern "C" fn(Obj,std::ffi::c_ulong),
    g_type_name_from_instance:unsafe extern "C" fn(Obj)->*const c_char,
    g_object_unref:unsafe extern "C" fn(Obj),
});
api!(Glib,"libglib-2.0.so.0",{
    g_set_prgname:unsafe extern "C" fn(*const c_char),
    g_set_application_name:unsafe extern "C" fn(*const c_char),
    g_filename_from_uri:unsafe extern "C" fn(*const c_char,*mut *mut c_char,*mut *mut GError)->*mut c_char,
    g_free:unsafe extern "C" fn(Obj),
    g_main_context_iteration:unsafe extern "C" fn(Obj,c_int)->c_int,
    g_error_free:unsafe extern "C" fn(*mut GError),
});
api!(Cairo,"libcairo.so.2",{
    cairo_image_surface_create:unsafe extern "C" fn(c_int,c_int,c_int)->Obj,
    cairo_image_surface_get_data:unsafe extern "C" fn(Obj)->*mut u8,
    cairo_image_surface_get_stride:unsafe extern "C" fn(Obj)->c_int,
    cairo_surface_mark_dirty:unsafe extern "C" fn(Obj),
    cairo_surface_status:unsafe extern "C" fn(Obj)->c_int,
    cairo_surface_destroy:unsafe extern "C" fn(Obj),
    cairo_save:unsafe extern "C" fn(Obj),
    cairo_restore:unsafe extern "C" fn(Obj),
    cairo_translate:unsafe extern "C" fn(Obj,f64,f64),
    cairo_scale:unsafe extern "C" fn(Obj,f64,f64),
    cairo_set_source_surface:unsafe extern "C" fn(Obj,Obj,f64,f64),
    cairo_paint:unsafe extern "C" fn(Obj),
    cairo_status:unsafe extern "C" fn(Obj)->c_int,
});
#[repr(C)]
struct GError {
    domain: u32,
    code: c_int,
    message: *mut c_char,
}

struct Job<'a, 'pdf> {
    gtk: &'a Gtk,
    glib: &'a Glib,
    cairo: &'a Cairo,
    pdf: &'a PdfDocument<'pdf>,
    source: &'a Path,
    fit: bool,
    units: Option<Vec<f64>>,
    error: Option<String>,
    drawn: Vec<usize>,
    metrics: Vec<serde_json::Value>,
}

impl Job<'_, '_> {
    fn protect_output(&self, operation: Obj) -> Result<()> {
        use std::os::unix::ffi::OsStrExt;
        unsafe {
            let settings = (self.gtk.gtk_print_operation_get_print_settings)(operation);
            if settings.is_null() {
                return Ok(());
            }
            let uri = (self.gtk.gtk_print_settings_get)(settings, c"output-uri".as_ptr());
            if uri.is_null() {
                return Ok(());
            }
            let mut error = std::ptr::null_mut();
            let filename = (self.glib.g_filename_from_uri)(uri, std::ptr::null_mut(), &mut error);
            if filename.is_null() {
                let message = glib_error(
                    self.glib,
                    error,
                    "Native print-to-file needs a local file URI",
                );
                bail!("{message}");
            }
            let path = PathBuf::from(std::ffi::OsStr::from_bytes(
                CStr::from_ptr(filename).to_bytes(),
            ));
            (self.glib.g_free)(filename.cast());
            crate::storage::ensure_separate_output(&path, &[self.source])
        }
    }
    fn draw(&mut self, context: Obj, index: c_int) -> Result<()> {
        let page = self.pdf.pages().get(index)?;
        let unit = self
            .units
            .as_ref()
            .map_or(1.0, |units| units[index as usize]);
        let (pw, ph) = (
            f64::from(page.width().value) * unit,
            f64::from(page.height().value) * unit,
        );
        let (w, h) = unsafe {
            (
                (self.gtk.gtk_print_context_get_width)(context),
                (self.gtk.gtk_print_context_get_height)(context),
            )
        };
        if ![pw, ph, w, h].iter().all(|v| v.is_finite() && *v > 0.0) {
            bail!("Invalid printed page dimensions");
        }
        let scale = if self.fit { (w / pw).min(h / ph) } else { 1.0 };
        let (dw, dh) = (pw * scale, ph * scale);
        // Around 300 DPI at the actual printed size, bounded to 16M pixels.
        let raster_scale = (300.0 / 72.0f64)
            .min(4096.0 / dw)
            .min(8192.0 / dh)
            .min((16_000_000.0 / (dw * dh)).sqrt());
        let bitmap = page.render_with_config(
            &PdfRenderConfig::new()
                .set_target_width((dw * raster_scale).floor().max(1.0) as i32)
                .set_maximum_height(8192)
                .use_print_quality(true)
                .render_annotations(true)
                .render_form_data(true),
        )?;
        let (bw, bh) = (bitmap.width(), bitmap.height());
        let mut pixels = bitmap.as_rgba_bytes();
        if pixels.len() != bw as usize * bh as usize * 4 {
            bail!("Native print image has invalid dimensions");
        }
        // Cairo ARGB32 uses native-endian premultiplied channels.
        for pixel in pixels.as_chunks_mut::<4>().0 {
            let a = u32::from(pixel[3]);
            let r = (u32::from(pixel[0]) * a + 127) / 255;
            let g = (u32::from(pixel[1]) * a + 127) / 255;
            let b = (u32::from(pixel[2]) * a + 127) / 255;
            pixel.copy_from_slice(&((a << 24) | (r << 16) | (g << 8) | b).to_ne_bytes());
        }
        unsafe {
            let surface = (self.cairo.cairo_image_surface_create)(0, bw, bh);
            if (self.cairo.cairo_surface_status)(surface) != 0 {
                (self.cairo.cairo_surface_destroy)(surface);
                bail!("Cannot allocate native print image surface");
            }
            let target = (self.cairo.cairo_image_surface_get_data)(surface);
            let stride = (self.cairo.cairo_image_surface_get_stride)(surface) as usize;
            // Cairo owns this allocation and may retain it until the PDF page is flushed.
            // Never lend a temporary Rust Vec to the PDF surface.
            for row in 0..bh as usize {
                std::ptr::copy_nonoverlapping(
                    pixels.as_ptr().add(row * bw as usize * 4),
                    target.add(row * stride),
                    bw as usize * 4,
                );
            }
            (self.cairo.cairo_surface_mark_dirty)(surface);
            let cr = (self.gtk.gtk_print_context_get_cairo_context)(context);
            (self.cairo.cairo_save)(cr);
            (self.cairo.cairo_translate)(cr, (w - dw) / 2.0, (h - dh) / 2.0);
            (self.cairo.cairo_scale)(cr, dw / f64::from(bw), dh / f64::from(bh));
            (self.cairo.cairo_set_source_surface)(cr, surface, 0.0, 0.0);
            (self.cairo.cairo_paint)(cr);
            // Restore the caller's graphics state; Cairo retains any needed image references.
            (self.cairo.cairo_restore)(cr);
            (self.cairo.cairo_surface_destroy)(surface);
            if (self.cairo.cairo_status)(cr) != 0 {
                bail!("Native print drawing failed");
            }
        }
        self.drawn.push(index as usize);
        self.metrics.push(serde_json::json!({"source_width":pw,"source_height":ph,"printable_width":w,"printable_height":h,"draw_width":dw,"draw_height":dh,"raster_width":bw,"raster_height":bh}));
        Ok(())
    }
}

unsafe extern "C" fn begin(operation: Obj, _context: Obj, data: Obj) {
    let job = unsafe { &mut *data.cast::<Job<'_, '_>>() };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        job.protect_output(operation)
    }));
    callback_result(job, operation, result);
}
unsafe extern "C" fn draw(operation: Obj, context: Obj, page: c_int, data: Obj) {
    let job = unsafe { &mut *data.cast::<Job<'_, '_>>() };
    if job.error.is_some() {
        return;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job.draw(context, page)));
    callback_result(job, operation, result);
}
fn callback_result(job: &mut Job<'_, '_>, operation: Obj, result: std::thread::Result<Result<()>>) {
    let error = match result {
        Ok(Ok(())) => return,
        Ok(Err(e)) => format!("{e:#}"),
        Err(_) => "Native print callback failed".into(),
    };
    job.error = Some(error);
    unsafe {
        (job.gtk.gtk_print_operation_cancel)(operation);
    }
}
unsafe fn glib_error(glib: &Glib, error: *mut GError, fallback: &str) -> String {
    if error.is_null() {
        return fallback.into();
    }
    let message = unsafe { CStr::from_ptr((*error).message) }
        .to_string_lossy()
        .into_owned();
    unsafe {
        (glib.g_error_free)(error);
    }
    message
}

fn zero_ranges(range: &str, count: usize) -> Result<String> {
    let ranges = crate::print_plan::ranges(range, count)?;
    if ranges.is_empty() {
        return Ok(String::new());
    }
    let pages = crate::print_plan::pages(count, 0, crate::print_plan::Selection::Ranges(&ranges))?;
    let mut groups: Vec<(usize, usize)> = Vec::new();
    for page in pages {
        if let Some((_, last)) = groups.last_mut()
            && page == *last + 1
        {
            *last = page;
        } else {
            groups.push((page, page));
        }
    }
    // GTK's range entry expects ascending, non-overlapping ranges. Normalize
    // before showing it, rather than losing ranges while parsing the dialog.
    Ok(groups
        .into_iter()
        .map(|(first, last)| {
            if first == last {
                first.to_string()
            } else {
                format!("{first}-{last}")
            }
        })
        .collect::<Vec<_>>()
        .join(","))
}

pub fn run_cli(mut args: impl Iterator<Item = OsString>, diagnostic: bool) -> Result<()> {
    if !diagnostic {
        crate::helper_process::enable_owned_print_host()?;
    }
    let source = PathBuf::from(args.next().context("Missing original path")?);
    let output = if diagnostic {
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
    let fit = args.next().context("Missing scaling")? == "fit";
    let printer = args.next().unwrap_or_default();
    if args.next().is_some() {
        bail!("Too many native print arguments");
    }
    if diagnostic && std::env::var("GTK_PRINT_BACKENDS").as_deref() != Ok("file") {
        bail!("Diagnostic printing requires GTK_PRINT_BACKENDS=file to exclude physical printers");
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
    // Fit-to-paper is scale invariant. Actual size requires the PDF's physical
    // unit multiplier, which PDFium's page-width/height APIs do not include.
    let units = if fit {
        None
    } else {
        Some(crate::pdf_units::load(&bytes)?)
    };
    let pdfium = crate::pdf::bind_pdfium()?;
    let pdf = pdfium.load_pdf_from_byte_slice(&bytes, None)?;
    if units
        .as_ref()
        .is_some_and(|units| units.len() != pdf.pages().len() as usize)
    {
        bail!("Physical page metadata differs from the printed snapshot");
    }
    if pdf.pages().is_empty() || current >= pdf.pages().len() as usize {
        bail!("Invalid print page");
    }
    let ranges = zero_ranges(&range, pdf.pages().len() as usize)?;
    let gtk = Gtk::load()?;
    let gobject = Gobject::load()?;
    let glib = Glib::load()?;
    let cairo = Cairo::load()?;
    let mut job = Job {
        gtk: &gtk,
        glib: &glib,
        cairo: &cairo,
        pdf: &pdf,
        source: &source,
        fit,
        units,
        error: None,
        drawn: Vec::new(),
        metrics: Vec::new(),
    };
    unsafe {
        (glib.g_set_prgname)(c"net.dpdf.dPDF".as_ptr());
        (glib.g_set_application_name)(c"dPDF".as_ptr());
        if (gtk.gtk_init_check)(std::ptr::null_mut(), std::ptr::null_mut()) == 0 {
            bail!("Cannot connect native printing to the desktop display");
        }
        if std::env::var("GTK_PRINT_BACKENDS").as_deref() == Ok("file") {
            // GTK reads this property, not the diagnostic opt-in variable.
            (gtk.gtk_settings_set_string_property)(
                (gtk.gtk_settings_get_default)(),
                c"gtk-print-backends".as_ptr(),
                c"file".as_ptr(),
                c"dPDF isolated print diagnostic".as_ptr(),
            );
        }
        let operation = (gtk.gtk_print_operation_new)();
        let settings = (gtk.gtk_print_settings_new)();
        let set = |key: &str, value: &str| -> Result<()> {
            (gtk.gtk_print_settings_set)(
                settings,
                CString::new(key)?.as_ptr(),
                CString::new(value)?.as_ptr(),
            );
            Ok(())
        };
        if !printer.is_empty() {
            set("printer", &printer.to_string_lossy())?;
        }
        if !ranges.is_empty() {
            set("print-pages", "ranges")?;
            set("page-ranges", &ranges)?;
        }
        if let Some(output) = &output {
            crate::storage::ensure_separate_output(output, &[&source])?;
            set("printer", "Print to File")?;
            set("output-file-format", "pdf")?;
            let uri = output
                .canonicalize()
                .unwrap_or(std::path::absolute(output)?);
            // GLib handles URI escaping, including spaces and non-ASCII filenames.
            let encoded = uri
                .as_os_str()
                .as_encoded_bytes()
                .iter()
                .map(|b| {
                    if b.is_ascii_alphanumeric() || b"/-._~".contains(b) {
                        (*b as char).to_string()
                    } else {
                        format!("%{b:02X}")
                    }
                })
                .collect::<String>();
            set("output-uri", &format!("file://{encoded}"))?;
            let setup = (gtk.gtk_page_setup_new)();
            let paper = (gtk.gtk_paper_size_new)(c"iso_a4".as_ptr());
            (gtk.gtk_page_setup_set_paper_size)(setup, paper);
            (gtk.gtk_paper_size_free)(paper);
            (gtk.gtk_page_setup_set_top_margin)(setup, 36.0, 1);
            (gtk.gtk_page_setup_set_bottom_margin)(setup, 36.0, 1);
            (gtk.gtk_page_setup_set_left_margin)(setup, 36.0, 1);
            (gtk.gtk_page_setup_set_right_margin)(setup, 36.0, 1);
            (gtk.gtk_print_operation_set_default_page_setup)(operation, setup);
            (gobject.g_object_unref)(setup);
        }
        (gtk.gtk_print_operation_set_print_settings)(operation, settings);
        (gobject.g_object_unref)(settings);
        (gtk.gtk_print_operation_set_n_pages)(operation, pdf.pages().len() as c_int);
        (gtk.gtk_print_operation_set_current_page)(operation, current as c_int);
        (gtk.gtk_print_operation_set_unit)(operation, 1);
        let title = format!(
            "dPDF — {}",
            source.file_name().unwrap_or_default().to_string_lossy()
        );
        (gtk.gtk_print_operation_set_job_name)(operation, CString::new(title.as_str())?.as_ptr());
        (gtk.gtk_print_operation_set_embed_page_setup)(operation, 1);
        (gtk.gtk_print_operation_set_allow_async)(operation, 0);
        let data = (&mut job as *mut Job<'_, '_>).cast();
        let begin_id = (gobject.g_signal_connect_data)(
            operation,
            c"begin-print".as_ptr(),
            std::mem::transmute::<unsafe extern "C" fn(Obj, Obj, Obj), Callback>(begin),
            data,
            None,
            0,
        );
        let draw_id = (gobject.g_signal_connect_data)(
            operation,
            c"draw-page".as_ptr(),
            std::mem::transmute::<unsafe extern "C" fn(Obj, Obj, c_int, Obj), Callback>(draw),
            data,
            None,
            0,
        );
        let mut error = std::ptr::null_mut();
        let result = if std::env::var("GTK_PRINT_BACKENDS").as_deref() == Ok("file") {
            // Keep GTK's original one-dialog virtual-file path. The no-dialog
            // API excludes virtual printers, even when explicitly selected.
            (gtk.gtk_print_operation_run)(operation, 0, std::ptr::null_mut(), &mut error)
        } else {
            // Configured PDF queues can submit without rasterizing. Preserve selected
            // settings when native raster printing needs a final confirmation.
            let dialog = (gtk.gtk_print_unix_dialog_new)(c"Print".as_ptr(), std::ptr::null_mut());
            (gtk.gtk_print_unix_dialog_set_settings)(
                dialog,
                (gtk.gtk_print_operation_get_print_settings)(operation),
            );
            (gtk.gtk_print_unix_dialog_set_current_page)(dialog, current as c_int);
            (gtk.gtk_print_unix_dialog_set_embed_page_setup)(dialog, 1);
            // Application can handle page sets, copies, collation, reversal, scaling,
            // PDF/PS and n-up. Preview is deliberately not offered by this adapter.
            (gtk.gtk_print_unix_dialog_set_manual_capabilities)(dialog, 895);
            let setup = (gtk.gtk_print_operation_get_default_page_setup)(operation);
            if !setup.is_null() {
                (gtk.gtk_print_unix_dialog_set_page_setup)(dialog, setup);
            }
            let response = (gtk.gtk_dialog_run)(dialog);
            let result = if response == -5 {
                // GTK_RESPONSE_OK
                let chosen = (gtk.gtk_print_unix_dialog_get_settings)(dialog);
                let setup = (gtk.gtk_print_unix_dialog_get_page_setup)(dialog);
                (gtk.gtk_widget_hide)(dialog);
                (gtk.gtk_print_operation_set_print_settings)(operation, chosen);
                (gtk.gtk_print_operation_set_default_page_setup)(operation, setup);
                (gobject.g_object_unref)(chosen);
                let attempt = job.protect_output(operation).and_then(|()| {
                    let printer = (gtk.gtk_print_unix_dialog_get_selected_printer)(dialog);
                    if printer.is_null() {
                        bail!("No printer selected");
                    }
                    direct_pdf::submit(
                        &gtk,
                        &gobject,
                        &glib,
                        direct_pdf::Request {
                            bytes: &bytes,
                            title: &title,
                            printer,
                            settings: (gtk.gtk_print_operation_get_print_settings)(operation),
                            setup,
                            count: pdf.pages().len() as usize,
                            current,
                            fit,
                        },
                    )
                });
                match attempt {
                    Ok(Some(receipt)) => {
                        (gtk.gtk_widget_destroy)(dialog);
                        (gobject.g_signal_handler_disconnect)(operation, begin_id);
                        (gobject.g_signal_handler_disconnect)(operation, draw_id);
                        (gobject.g_object_unref)(operation);
                        println!("{receipt}");
                        return Ok(());
                    }
                    Ok(None) => {
                        // GTK's no-dialog printer finder skips virtual printers and can
                        // silently choose a different default. Require confirmation.
                        (gtk.gtk_print_operation_run)(
                            operation,
                            0,
                            std::ptr::null_mut(),
                            &mut error,
                        )
                    }
                    Err(problem) => {
                        job.error = Some(format!("{problem:#}"));
                        0
                    }
                }
            } else {
                2
            };
            (gtk.gtk_widget_destroy)(dialog);
            result
        };
        (gobject.g_signal_handler_disconnect)(operation, begin_id);
        (gobject.g_signal_handler_disconnect)(operation, draw_id);
        (gobject.g_object_unref)(operation);
        if let Some(message) = job.error.take() {
            bail!("{message}");
        }
        if result == 0 {
            bail!("{}", glib_error(&glib, error, "Native printing failed"));
        }
        if result != 1 && result != 2 {
            bail!("Native printing returned an unexpected asynchronous result");
        }
        if !error.is_null() {
            (glib.g_error_free)(error);
        }
        println!(
            "{}",
            serde_json::json!({"status":if result==2{"cancelled"}else{"submitted"},"pages_drawn":job.drawn,"metrics":job.metrics})
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_zero_based_ranges_preserve_current_and_disjoint_pages() {
        assert_eq!(super::zero_ranges("1-3,7", 10).unwrap(), "0-2,6");
        assert_eq!(super::zero_ranges("7", 10).unwrap(), "6");
        assert_eq!(super::zero_ranges("50,7-9,1,7", 50).unwrap(), "0,6-8,49");
        assert_eq!(super::zero_ranges("", 10).unwrap(), "");
        assert!(super::zero_ranges("11", 10).is_err());
    }
}
