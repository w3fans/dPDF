//! Compact PDF submission through GTK's native CUPS job API. The selected
//! printer's authentication and asynchronous completion remain owned by GTK.
use super::{GError, Glib, Gobject, Gtk, Obj};
use anyhow::{Context, Result, bail};
use std::{
    ffi::{CStr, CString},
    fs::File,
    io::{Seek, SeekFrom, Write},
    os::fd::AsRawFd,
    process::Command,
    sync::Mutex,
};

/// GTK's page-ranges setting is zero-based; CUPS page-ranges is one-based.
/// Bound/check the complete selection before sending any job.
fn selection(mode: &str, ranges: &str, count: usize, current: usize) -> Result<String> {
    if count == 0 || count > 100_000 || current >= count {
        bail!("Invalid direct PDF page selection");
    }
    match mode {
        "all" | "" => Ok(String::new()),
        "current" => Ok((current + 1).to_string()),
        "ranges" => {
            let mut selected = vec![false; count];
            for part in ranges.split(',') {
                let (start, end) = part.split_once('-').unwrap_or((part, part));
                let start: usize = start.parse().context("Invalid GTK print range")?;
                let end: usize = end.parse().context("Invalid GTK print range")?;
                if start > end || end >= count {
                    bail!("Printed page range is outside the document");
                }
                selected[start..=end].fill(true);
            }
            // Print each selected source page once in document order, matching
            // the shared native print plan. Compress runs to keep tickets small.
            let mut groups: Vec<(usize, usize)> = Vec::new();
            for (page, yes) in selected.into_iter().enumerate() {
                if !yes {
                    continue;
                }
                if let Some((_, last)) = groups.last_mut()
                    && page == *last + 1
                {
                    *last = page;
                } else {
                    groups.push((page, page));
                }
            }
            if groups.is_empty() {
                bail!("No pages selected for direct PDF printing");
            }
            Ok(groups
                .into_iter()
                .map(|(first, last)| {
                    if first == last {
                        (first + 1).to_string()
                    } else {
                        format!("{}-{}", first + 1, last + 1)
                    }
                })
                .collect::<Vec<_>>()
                .join(","))
        }
        _ => bail!("Unsupported direct PDF page selection"),
    }
}

fn opened_snapshot(bytes: &[u8]) -> Result<File> {
    use std::os::unix::fs::PermissionsExt;
    // No user pathname is reopened and no named print snapshot is left behind.
    let mut file = tempfile::tempfile().context("Create private print snapshot")?;
    // Shared build/desktop temp directories can supply a permissive default ACL.
    // Set the mask before writing confidential document bytes.
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    file.write_all(bytes)?;
    file.flush()?;
    file.seek(SeekFrom::Start(0))?;
    Ok(file)
}

/// GTK's CUPS encoder cannot forward disjoint ranges. Select them locally as
/// PDF pages, preserving streams instead of rendering. Everything stays inside
/// a private workspace; only the unlinked output descriptor escapes it.
fn selected_snapshot(bytes: &[u8], selected: &str, count: usize) -> Result<File> {
    use std::os::unix::fs::PermissionsExt;
    let pages = crate::pages::parse_selection(selected, count)?;
    let directory = tempfile::Builder::new().prefix("dpdf-print-").tempdir()?;
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))?;
    let mut input = tempfile::NamedTempFile::new_in(directory.path())?;
    input
        .as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    input.write_all(bytes)?;
    input.flush()?;
    let output = tempfile::NamedTempFile::new_in(directory.path())?.into_temp_path();
    let mut job = tempfile::NamedTempFile::new_in(directory.path())?;
    job.as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    serde_json::to_writer(
        &mut job,
        &serde_json::json!({
        "empty": "", "outputFile": output.to_path_buf(),
            "pages": [{"file": input.path(), "range": selected}],
            "streamData": "preserve", "objectStreams": "preserve"
        }),
    )?;
    job.flush()?;
    let executable = crate::assembly::qpdf_executable();
    // A JSON job avoids command-line length limits for a large page selection.
    // No source pathname from the user's filesystem is reopened.
    crate::assembly::run_qpdf(
        Command::new(&executable).arg(format!("--job-json-file={}", job.path().display())),
        "Prepare compact print pages",
    )?;
    let file = File::open(&output)?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    if file.metadata()?.len() > 256 * 1024 * 1024
        || crate::assembly::page_count(&executable, &output)? != pages.len()
    {
        bail!("Compact print snapshot has an unexpected size or page count");
    }
    drop(output);
    Ok(file)
}

type Completion = Mutex<Option<std::result::Result<(), String>>>;

unsafe extern "C" fn complete(_job: Obj, data: Obj, error: *const GError) {
    let result = if error.is_null() {
        Ok(())
    } else {
        // GTK owns GError and frees it after returning from this callback.
        let message = unsafe { (*error).message };
        Err(if message.is_null() {
            "Native PDF submission failed".into()
        } else {
            unsafe { CStr::from_ptr(message) }
                .to_string_lossy()
                .into_owned()
        })
    };
    let state = unsafe { &*data.cast::<Completion>() };
    if let Ok(mut state) = state.lock() {
        *state = Some(result);
    }
}

struct JobRef<'a> {
    object: Obj,
    api: &'a Gobject,
}
impl Drop for JobRef<'_> {
    fn drop(&mut self) {
        unsafe { (self.api.g_object_unref)(self.object) };
    }
}

pub(super) struct Request<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) title: &'a str,
    pub(super) printer: Obj,
    pub(super) settings: Obj,
    pub(super) setup: Obj,
    pub(super) count: usize,
    pub(super) current: usize,
    pub(super) fit: bool,
}

pub(super) fn submit(
    gtk: &Gtk,
    gobject: &Gobject,
    glib: &Glib,
    request: Request<'_>,
) -> Result<Option<serde_json::Value>> {
    let Request {
        bytes,
        title,
        printer,
        settings,
        setup,
        count,
        current,
        fit,
    } = request;
    if printer.is_null() {
        return Ok(None);
    }
    unsafe {
        let kind = (gobject.g_type_name_from_instance)(printer);
        if kind.is_null()
            || CStr::from_ptr(kind).to_bytes() != b"GtkPrinterCups"
            || (gtk.gtk_printer_accepts_pdf)(printer) == 0
        {
            return Ok(None);
        }
        let name = CStr::from_ptr((gtk.gtk_printer_get_name)(printer)).to_str()?;
        // Restrict server-side layout to configured CUPS queues. Discovered
        // direct IPP destinations need not implement the same layout options.
        if !crate::helper_process::run(
            Command::new("lpstat").args(["-p", name]),
            b"",
            crate::helper_process::Limits {
                timeout: std::time::Duration::from_secs(5),
                stdout: 64 * 1024,
                stderr: 0,
            },
            None,
        )
        .is_ok_and(|o| o.status.success())
        {
            return Ok(None);
        }
        // Declare the snapshot first: GTK borrows its fd, so the job must be
        // destroyed before the Rust File closes the descriptor.
        let snapshot: File;
        let raw = (gtk.gtk_print_job_new)(CString::new(title)?.as_ptr(), printer, settings, setup);
        if raw.is_null() {
            bail!("Cannot prepare native CUPS print settings");
        }
        let prepared = JobRef {
            object: raw,
            api: gobject,
        };
        // GTK maps printer-supported controls to cups-* options. If it leaves
        // client-side layout work, use the existing raster path without sending.
        let eligible = (gtk.gtk_print_job_get_scale)(raw) == 1.0
            && (gtk.gtk_print_job_get_num_copies)(raw) == 1
            && (gtk.gtk_print_job_get_n_up)(raw) == 1
            && (gtk.gtk_print_job_get_page_set)(raw) == 0
            && (gtk.gtk_print_job_get_reverse)(raw) == 0
            && ((gtk.gtk_print_job_get_collate)(raw) == 0
                || (gtk.gtk_print_settings_get_n_copies)(settings) <= 1);
        if !eligible {
            return Ok(None);
        }
        let settings = (gtk.gtk_print_job_get_settings)(raw);
        let get = |key: &CStr| {
            let value = (gtk.gtk_print_settings_get)(settings, key.as_ptr());
            if value.is_null() {
                String::new()
            } else {
                CStr::from_ptr(value).to_string_lossy().into_owned()
            }
        };
        let n_up = get(c"number-up");
        if !n_up.is_empty() && n_up != "1" && get(c"cups-number-up") != n_up {
            return Ok(None);
        }
        let selected = selection(&get(c"print-pages"), &get(c"page-ranges"), count, current)?;
        let subset = selected.contains(',');
        snapshot = if subset {
            match selected_snapshot(bytes, &selected, count) {
                Ok(snapshot) => snapshot,
                // No job has been sent. Preserve the established renderer if
                // qpdf is missing or refuses a document PDFium can render.
                Err(_) => return Ok(None),
            }
        } else {
            opened_snapshot(bytes)?
        };
        let set = |key: &CStr, value: &str| -> Result<()> {
            (gtk.gtk_print_settings_set)(settings, key.as_ptr(), CString::new(value)?.as_ptr());
            Ok(())
        };
        if selected.is_empty() || subset {
            (gtk.gtk_print_settings_unset)(settings, c"cups-page-ranges".as_ptr());
            if subset {
                // The snapshot already contains exactly the selected pages;
                // never apply the original page numbers a second time.
                set(c"print-pages", "all")?;
                (gtk.gtk_print_settings_unset)(settings, c"page-ranges".as_ptr());
            }
        } else {
            set(c"cups-page-ranges", &selected)?;
        }
        set(c"cups-print-scaling", if fit { "fit" } else { "none" })?;
        set(c"cups-fit-to-page", if fit { "true" } else { "false" })?;
        let orientation = match (gtk.gtk_page_setup_get_orientation)(setup) {
            0 => "3",
            1 => "4",
            2 => "6",
            3 => "5",
            _ => "3",
        };
        set(c"cups-orientation-requested", orientation)?;
        let mut error = std::ptr::null_mut();
        if (gtk.gtk_print_job_set_source_fd)(raw, snapshot.as_raw_fd(), &mut error) == 0 {
            bail!(
                "{}",
                super::glib_error(glib, error, "Cannot attach opened PDF snapshot")
            );
        }
        // GTK treats a syntactically valid IPP error reply as transport success.
        // Tracking makes its CUPS backend enter PENDING only with a server job-id;
        // missing acknowledgements otherwise appear immediately FINISHED.
        (gtk.gtk_print_job_set_track_print_status)(raw, 1);
        let completion: Completion = Mutex::new(None);
        (gtk.gtk_print_job_send)(
            raw,
            complete,
            (&completion as *const Completion).cast_mut().cast(),
            None,
        );
        let result = loop {
            let result = completion
                .lock()
                .map_err(|_| anyhow::anyhow!("Native print completion state failed"))?
                .take();
            if let Some(result) = result {
                break result;
            }
            // Keep native authentication/dialog events and socket I/O moving.
            // Never sleep between ready I/O events or impose an auth deadline.
            if (glib.g_main_context_iteration)(std::ptr::null_mut(), 0) == 0 {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        };
        // Submission errors do not trigger raster retries: the server may have
        // accepted data before an acknowledgement was lost, risking duplicates.
        result
            .map_err(anyhow::Error::msg)
            .context("CUPS PDF submission failed")?;
        // The callback's source dispatch has finished, but we have not iterated
        // again to poll print completion. An acknowledged job must be pending
        // (4/5) or printing (6), rather than GTK's no-job-id FINISHED shortcut.
        if !matches!((gtk.gtk_print_job_get_status)(raw), 4..=6) {
            bail!(
                "CUPS PDF submission failed: server did not acknowledge a queued job; do not retry automatically"
            );
        }
        drop(prepared);
        Ok(Some(
            serde_json::json!({"status":"submitted", "transport":"cups-pdf", "source_bytes":bytes.len(), "spool_bytes":snapshot.metadata()?.len(), "subset_pages":if subset { &selected } else { "" }, "pages_drawn":[], "metrics":[]}),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selections_are_checked_and_converted_before_submission() {
        assert_eq!(selection("all", "", 50, 6).unwrap(), "");
        assert_eq!(selection("current", "", 50, 6).unwrap(), "7");
        assert_eq!(selection("ranges", "0-2,6", 50, 6).unwrap(), "1-3,7");
        assert_eq!(selection("ranges", "6,0-2,1-3,6", 50, 6).unwrap(), "1-4,7");
        assert_eq!(
            selection("ranges", "0-99999", 100_000, 0).unwrap(),
            "1-100000"
        );
        for (mode, range) in [
            ("ranges", ""),
            ("ranges", "50"),
            ("ranges", "9-1"),
            ("ranges", "-1"),
            ("ranges", "18446744073709551615"),
            ("selection", ""),
        ] {
            assert!(selection(mode, range, 50, 6).is_err(), "{mode}/{range}");
        }
    }
    #[test]
    fn opened_snapshot_is_anonymous_private_and_contains_the_exact_bytes() {
        use std::{
            io::Read,
            os::unix::fs::{MetadataExt, PermissionsExt},
        };
        let bytes = b"%PDF-1.7\nimmutable generated opened snapshot\n";
        let mut file = opened_snapshot(bytes).unwrap();
        let metadata = file.metadata().unwrap();
        assert_eq!(metadata.permissions().mode() & 0o077, 0);
        assert_eq!(metadata.nlink(), 0);
        let mut received = Vec::new();
        file.read_to_end(&mut received).unwrap();
        assert_eq!(received, bytes);
    }
    #[test]
    fn completion_copies_errors_without_taking_gtk_error_ownership() {
        let state: Completion = Mutex::new(None);
        let data = (&state as *const Completion).cast_mut().cast();
        unsafe {
            complete(std::ptr::null_mut(), data, std::ptr::null());
        }
        assert_eq!(state.lock().unwrap().take(), Some(Ok(())));
        let message = CString::new("controlled submission refusal").unwrap();
        let error = GError {
            domain: 0,
            code: 0,
            message: message.as_ptr().cast_mut(),
        };
        unsafe {
            complete(std::ptr::null_mut(), data, &error);
        }
        drop(message);
        assert_eq!(
            state.lock().unwrap().take(),
            Some(Err("controlled submission refusal".into()))
        );
    }
}
