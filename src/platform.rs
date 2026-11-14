use anyhow::{Context, Result, bail};
use std::{path::Path, process::Command, sync::atomic::AtomicBool};

/// Called only for fixed public About links after the user activates a button.
pub fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let status = Command::new("xdg-open")
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .context("Start the default browser with xdg-open")?;
        if !status.success() {
            bail!("The default browser could not open this link");
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        open_windows_url(url)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = url;
        bail!("External links are supported on Fedora and Windows");
    }
}

#[cfg(any(target_os = "windows", feature = "check-windows-print"))]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn open_windows_url(url: &str) -> Result<()> {
    use windows_sys::Win32::{
        System::Com::{
            COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
        },
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    };
    let initialized = unsafe {
        CoInitializeEx(
            std::ptr::null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        )
    };
    if initialized < 0 {
        bail!("Cannot initialize the Windows shell (error {initialized})");
    }
    struct ComApartment;
    impl Drop for ComApartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _apartment = ComApartment;
    let text = url.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let verb = "open".encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            text.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    if result <= 32 {
        bail!("The system could not open this location (Windows error {result})");
    }
    Ok(())
}

/// A sharing action refers to the saved file on disk, never unsaved page drafts.
pub fn saved_file_path(path: &Path) -> Result<String> {
    let path = path
        .canonicalize()
        .context("Saved PDF is unavailable; save a copy first")?;
    if !path.is_file() {
        bail!("Choose a saved PDF file before sharing");
    }
    crate::assembly::path_string(&path)
        .map(|path| path.into_owned())
        .map_err(|_| {
            anyhow::anyhow!("Save a copy to a regular Unicode drive or network path before sharing")
        })
}

/// Opens the saved file's folder. This does not claim email attachment support.
pub fn open_file_folder(path: &Path) -> Result<()> {
    let saved = saved_file_path(path)?;
    let folder = Path::new(&saved)
        .parent()
        .context("Saved file has no folder")?;
    #[cfg(target_os = "linux")]
    {
        open_folder_with(Path::new("xdg-open"), folder)
    }
    #[cfg(target_os = "windows")]
    {
        open_windows_url(folder.to_str().context("Folder path is not Unicode")?)
            .context("Open the saved PDF folder")
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = folder;
        bail!("Opening folders is supported on Fedora and Windows");
    }
}

#[cfg(target_os = "linux")]
fn open_folder_with(executable: &Path, folder: &Path) -> Result<()> {
    let status = Command::new(executable)
        .arg(folder)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("Start the desktop file manager")?;
    if !status.success() {
        bail!("The file manager could not open the folder; copy the saved file path instead");
    }
    Ok(())
}

/// Validates page ranges before passing them to a spooler; page numbering is one-based.
pub fn validate_range(range: &str, page_count: usize) -> Result<String> {
    let range = range.trim();
    if range.is_empty() {
        return Ok(String::new());
    }
    let mut normalized = Vec::new();
    for part in range.split(',') {
        let bounds: Vec<_> = part.trim().split('-').collect();
        if bounds.is_empty() || bounds.len() > 2 {
            bail!("Use page ranges such as 1-3,7");
        }
        let start: usize = bounds[0].trim().parse().context("Invalid page number")?;
        let end = if bounds.len() == 2 {
            bounds[1].trim().parse().context("Invalid range end")?
        } else {
            start
        };
        if start == 0 || start > end || end > page_count {
            bail!("Page range must be between 1 and {page_count}, in ascending order");
        }
        normalized.push(if start == end {
            start.to_string()
        } else {
            format!("{start}-{end}")
        });
    }
    Ok(normalized.join(","))
}

pub fn discover_printers(cancel: Option<&AtomicBool>) -> Result<Vec<String>> {
    #[cfg(target_os = "linux")]
    {
        discover_cups_printers(Path::new("lpstat"), cancel)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = cancel;
        bail!(
            "Printer discovery is not implemented on this platform; use the default or enter a printer name"
        )
    }
}

#[cfg(target_os = "linux")]
fn discover_cups_printers(executable: &Path, cancel: Option<&AtomicBool>) -> Result<Vec<String>> {
    let output = cups_output_cancellable(
        Command::new(executable).arg("-e"),
        std::time::Duration::from_secs(5),
        false,
        cancel,
    )?;
    if !output.status.success() {
        bail!(
            "Printer discovery failed: {}",
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(2000)
                .collect::<String>()
                .trim()
        );
    }
    let mut names: Vec<_> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    Ok(names)
}

#[cfg(all(target_os = "linux", test))]
fn cups_output(
    command: &mut Command,
    timeout: std::time::Duration,
    submission: bool,
) -> Result<std::process::Output> {
    cups_output_cancellable(command, timeout, submission, None)
}

#[cfg(target_os = "linux")]
fn cups_output_cancellable(
    command: &mut Command,
    timeout: std::time::Duration,
    submission: bool,
    cancel: Option<&AtomicBool>,
) -> Result<std::process::Output> {
    crate::helper_process::run(
        command,
        b"",
        crate::helper_process::Limits {
            timeout,
            stdout: 64 * 1024,
            stderr: 64 * 1024,
        },
        cancel,
    )
    .map_err(|error| {
        if submission {
            anyhow::anyhow!(
                "CUPS did not confirm submission: {error}. The job may already be queued; check the print queue before submitting again"
            )
        } else {
            anyhow::anyhow!("Discover system printers with lpstat: {error}")
        }
    })
}

#[cfg(target_os = "linux")]
fn submit_cups(
    executable: &Path,
    path: &Path,
    range: &str,
    printer: &str,
    fit: bool,
    cancel: Option<&AtomicBool>,
) -> Result<String> {
    submit_cups_with_command_cancellable(
        &mut Command::new(executable),
        path,
        range,
        printer,
        fit,
        cancel,
    )
}

#[cfg(all(target_os = "linux", test))]
fn submit_cups_with_command(
    command: &mut Command,
    path: &Path,
    range: &str,
    printer: &str,
    fit: bool,
) -> Result<String> {
    submit_cups_with_command_cancellable(command, path, range, printer, fit, None)
}

#[cfg(target_os = "linux")]
fn submit_cups_with_command_cancellable(
    command: &mut Command,
    path: &Path,
    range: &str,
    printer: &str,
    fit: bool,
    cancel: Option<&AtomicBool>,
) -> Result<String> {
    if !printer.trim().is_empty() {
        command.args(["-d", printer.trim()]);
    }
    if !range.is_empty() {
        command.args(["-P", range]);
    }
    command.args([
        "-o",
        if fit {
            "fit-to-page"
        } else {
            "print-scaling=none"
        },
    ]);
    let output = cups_output_cancellable(
        command.arg("--").arg(path),
        std::time::Duration::from_secs(30),
        true,
        cancel,
    )?;
    if !output.status.success() {
        bail!(
            "CUPS did not confirm submission: {}. The job may already be queued; check the print queue before submitting again",
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(2000)
                .collect::<String>()
                .trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub fn print_pdf(
    path: &Path,
    range: &str,
    printer: &str,
    fit: bool,
    cancel: Option<&AtomicBool>,
) -> Result<String> {
    #[cfg(target_os = "linux")]
    {
        submit_cups(Path::new("lp"), path, range, printer, fit, cancel)
    }
    #[cfg(target_os = "windows")]
    {
        // Quick Print remains an explicit helper; the toolbar and Ctrl+P use
        // the separate native print child and the standard Windows dialog.
        let helper = std::env::var_os("DPDF_PRINT_HELPER").context("Quick Print requires DPDF_PRINT_HELPER pointing to SumatraPDF.exe; use System dialog for native Windows printing")?;
        let mut command = Command::new(helper);
        if printer.trim().is_empty() {
            command.arg("-print-to-default");
        } else {
            command.args(["-print-to", printer.trim()]);
        }
        let settings = if range.is_empty() {
            if fit { "fit".into() } else { "noscale".into() }
        } else {
            format!("{range},{}", if fit { "fit" } else { "noscale" })
        };
        let output = crate::helper_process::run(
            command
                .args(["-silent", "-print-settings", &settings])
                .arg(path),
            b"",
            crate::helper_process::Limits {
                timeout: std::time::Duration::from_secs(60),
                stdout: 64 * 1024,
                stderr: 64 * 1024,
            },
            cancel,
        )
        .map_err(|error| anyhow::anyhow!("Windows print helper did not confirm submission: {error}. The job may already be queued; check the print queue before submitting again"))?;
        if !output.status.success() {
            bail!(
                "Windows print helper did not confirm submission: {}. The job may already be queued; check the print queue before submitting again",
                String::from_utf8_lossy(&output.stderr)
                    .chars()
                    .take(2000)
                    .collect::<String>()
            );
        }
        Ok("Sent to configured Windows print helper".into())
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (path, range, printer, fit, cancel);
        bail!("Printing is supported on Linux and Windows only")
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub fn native_print(
    bytes: &[u8],
    source: &Path,
    current: usize,
    range: &str,
    printer: &str,
    fit: bool,
    cancel: &AtomicBool,
) -> Result<String> {
    if bytes.len() > 256 * 1024 * 1024 {
        bail!(
            "Native printing supports PDF snapshots up to 256 MiB; use Quick Print for this file"
        );
    }
    // This owner retains scratch across child/group/job cleanup, including a
    // forced abort of a nested native-host helper. No private temp files detach.
    let scratch = tempfile::tempdir()?;
    let output = crate::helper_process::run_dialog(
        Command::new(std::env::current_exe()?)
            .env("GTK_USE_PORTAL", "0")
            .env("TMPDIR", scratch.path()).env("TMP", scratch.path()).env("TEMP", scratch.path())
            .arg("--native-print").arg(source).arg(current.to_string()).arg(range)
            .arg(if fit { "fit" } else { "actual" }).arg(printer),
        bytes,
        cancel,
    ).map_err(|error| anyhow::anyhow!("Native printing did not confirm completion: {error}. The job may already be queued; check the print queue before submitting again"))?;
    if !output.status.success() {
        bail!(
            "Native printing did not confirm completion: {}. The job may already be queued; check the print queue before submitting again",
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(2000)
                .collect::<String>()
                .trim()
        );
    }
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("Native printing returned an invalid result; the job may already be queued, check the print queue before submitting again")?;
    match result["status"].as_str() {
        Some("cancelled") => Ok("Printing cancelled".into()),
        Some("submitted") => Ok("Sent to the native print system".into()),
        _ => bail!("Unknown native printing result; check the print queue before submitting again"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sharing_requires_an_existing_file_and_preserves_unicode_names() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("Pogodba ŽČŠ 100% #contract.pdf");
        assert!(saved_file_path(&path).is_err());
        assert!(saved_file_path(folder.path()).is_err());
        std::fs::write(&path, b"opened PDF bytes").unwrap();
        let shared = saved_file_path(&path).unwrap();
        assert!(shared.ends_with("Pogodba ŽČŠ 100% #contract.pdf"));
        assert_eq!(std::fs::read(&shared).unwrap(), b"opened PDF bytes");
        std::fs::remove_file(path).unwrap();
        assert!(saved_file_path(Path::new(&shared)).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn folder_open_passes_one_literal_argument_and_reports_failure() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("folder Ž $value ; 100% #quote'");
        std::fs::create_dir(&folder).unwrap();
        let opener = root.path().join("file-manager-fixture");
        std::fs::write(&opener, "#!/bin/sh\n[ \"$#\" -eq 1 ] && [ -d \"$1\" ] || exit 3\nprintf '%s' \"$1\" > \"$1/received.txt\"\n").unwrap();
        std::fs::set_permissions(&opener, std::fs::Permissions::from_mode(0o700)).unwrap();
        open_folder_with(&opener, &folder).unwrap();
        assert_eq!(
            std::fs::read_to_string(folder.join("received.txt")).unwrap(),
            folder.to_str().unwrap()
        );
        std::fs::write(&opener, "#!/bin/sh\nexit 7\n").unwrap();
        assert!(open_folder_with(&opener, &folder).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn cups_process_receives_literal_ranges_printer_and_pdf_bytes() {
        use std::os::unix::fs::PermissionsExt;
        let folder = tempfile::tempdir().unwrap();
        let mock = folder.path().join("lp-mock");
        let source = folder.path().join("signed contract.pdf");
        let capture = folder.path().join("arguments");
        let captured_pdf = folder.path().join("printed.pdf");
        let quote = |path: &Path| format!("'{}'", path.to_string_lossy().replace('\'', "'\"'\"'"));
        std::fs::write(&source, b"%PDF-1.7 original signed bytes").unwrap();
        std::fs::write(&mock, format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\nfor last do :; done\ncat \"$last\" > {}\nprintf 'request id is mock-7\\n'\n", quote(&capture), quote(&captured_pdf))).unwrap();
        std::fs::set_permissions(&mock, std::fs::Permissions::from_mode(0o700)).unwrap();
        // Invoke the generated fixture through an existing interpreter. A
        // concurrent fork can transiently inherit a newly written script's fd
        // before exec closes it, making direct execution fail with ETXTBSY.
        // Production submission still executes lp directly, without a retry.
        let writable = std::fs::OpenOptions::new().write(true).open(&mock).unwrap();
        let direct_error = Command::new(&mock).spawn().unwrap_err();
        assert_eq!(direct_error.raw_os_error(), Some(26));
        let printer = "Office$(not-a-command)";
        assert_eq!(
            submit_cups_with_command(
                Command::new("/bin/sh").arg(&mock),
                &source,
                "1-3,7",
                printer,
                false
            )
            .unwrap(),
            "request id is mock-7"
        );
        let arguments = std::fs::read_to_string(&capture).unwrap();
        assert_eq!(
            arguments.lines().collect::<Vec<_>>(),
            vec![
                "-d",
                printer,
                "-P",
                "1-3,7",
                "-o",
                "print-scaling=none",
                "--",
                source.to_str().unwrap()
            ]
        );
        assert_eq!(
            std::fs::read(&captured_pdf).unwrap(),
            std::fs::read(&source).unwrap()
        );
        submit_cups_with_command(Command::new("/bin/sh").arg(&mock), &source, "", "", true)
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&capture)
                .unwrap()
                .lines()
                .take(3)
                .collect::<Vec<_>>(),
            vec!["-o", "fit-to-page", "--"]
        );
        std::fs::write(&mock, "#!/bin/sh\necho 'printer offline' >&2\nexit 42\n").unwrap();
        assert!(
            submit_cups_with_command(Command::new("/bin/sh").arg(&mock), &source, "7", "", true)
                .unwrap_err()
                .to_string()
                .contains("printer offline")
        );
        drop(writable);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cups_stalls_and_floods_report_ambiguity_without_resubmitting() {
        use std::time::{Duration, Instant};
        let folder = tempfile::tempdir().unwrap();
        let attempts = folder.path().join("attempts");
        let source = folder.path().join("source.pdf");
        std::fs::write(&source, b"%PDF unchanged source").unwrap();
        let command = |delay: &str| {
            let mut command = Command::new("/bin/sh");
            command
                .args([
                    "-c",
                    "printf 'submitted\\n' >> \"$1\"; sleep \"$2\"",
                    "fixture",
                ])
                .arg(&attempts)
                .arg(delay);
            command
        };
        // Characterize the previous Command::output boundary with a finite
        // stall before exercising the replacement against the same fixture.
        let start = Instant::now();
        assert!(command("0.25").output().unwrap().status.success());
        assert!(start.elapsed() >= Duration::from_millis(200));
        assert_eq!(std::fs::read_to_string(&attempts).unwrap(), "submitted\n");
        std::fs::write(&attempts, b"").unwrap();
        let start = Instant::now();
        let error = cups_output(&mut command("30"), Duration::from_millis(150), true)
            .unwrap_err()
            .to_string();
        assert!(error.contains("deadline") && error.contains("may already be queued"));
        assert!(error.contains("check the print queue"));
        assert!(start.elapsed() < Duration::from_secs(3));
        assert_eq!(std::fs::read_to_string(&attempts).unwrap(), "submitted\n");
        for stream in ["cat /dev/zero", "cat /dev/zero >&2"] {
            let error = cups_output(
                Command::new("/bin/sh").args(["-c", stream]),
                Duration::from_secs(3),
                true,
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains("byte limit") && error.contains("may already be queued"));
        }
        let error = cups_output(
            Command::new("/bin/sh").args(["-c", "sleep 30"]),
            Duration::from_millis(150),
            false,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("deadline") && !error.contains("queued"));
        let retry = cups_output(
            Command::new("/bin/sh").args(["-c", "printf 'request id is retry-1'"]),
            Duration::from_secs(3),
            true,
        )
        .unwrap();
        assert_eq!(retry.stdout, b"request id is retry-1");
        assert_eq!(std::fs::read(&source).unwrap(), b"%PDF unchanged source");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cups_discovery_filters_blank_and_duplicate_names() {
        use std::os::unix::fs::PermissionsExt;
        let folder = tempfile::tempdir().unwrap();
        let mock = folder.path().join("lpstat-mock");
        std::fs::write(
            &mock,
            "#!/bin/sh\n[ \"$1\" = -e ] || exit 2\nprintf 'West\\nEast\\nWest\\n\\n'\n",
        )
        .unwrap();
        std::fs::set_permissions(&mock, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            discover_cups_printers(&mock, None).unwrap(),
            vec!["East", "West"]
        );
    }

    #[test]
    fn ranges_are_bounded_and_no_spooler_options_can_be_injected() {
        assert_eq!(validate_range(" 1 - 3, 7 ", 10).unwrap(), "1-3,7");
        assert_eq!(validate_range("", 10).unwrap(), "");
        for invalid in ["0", "11", "3-1", "1-", "-d evil", "1,,2", "1-2-3"] {
            assert!(validate_range(invalid, 10).is_err(), "{invalid}");
        }
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn cancelled_submission_is_owned_and_reports_ambiguity_without_retry() {
        use std::{
            sync::atomic::Ordering,
            thread,
            time::{Duration, Instant},
        };
        let folder = tempfile::tempdir().unwrap();
        let attempts = folder.path().join("attempts");
        let cancel = AtomicBool::new(false);
        let start = Instant::now();
        thread::scope(|scope| {
            scope.spawn(|| {
                let until = Instant::now() + Duration::from_secs(2);
                while !attempts.exists() && Instant::now() < until {
                    thread::sleep(Duration::from_millis(5));
                }
                cancel.store(true, Ordering::Release);
            });
            let error = cups_output_cancellable(
                Command::new("sh")
                    .args(["-c", "echo submitted >> \"$1\"; sleep 30", "fixture"])
                    .arg(&attempts),
                Duration::from_secs(30),
                true,
                Some(&cancel),
            )
            .unwrap_err()
            .to_string();
            assert!(
                error.contains("cancelled")
                    && error.contains("may already be queued")
                    && error.contains("check the print queue")
            );
        });
        assert!(start.elapsed() < Duration::from_secs(3));
        assert_eq!(std::fs::read_to_string(attempts).unwrap(), "submitted\n");
    }
}
