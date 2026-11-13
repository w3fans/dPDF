//! Linux noninteractive helper supervision. No engine handles or diagnostic input logging.
use anyhow::{Context, Result, bail};
use std::{
    io::{self, Read, Write},
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

pub(crate) struct Limits {
    pub timeout: Duration,
    pub stdout: usize,
    // Zero discards stderr, including anything a helper echoes from private stdin.
    pub stderr: usize,
}

unsafe extern "C" {
    fn fcntl(fd: std::ffi::c_int, command: std::ffi::c_int, ...) -> std::ffi::c_int;
    fn kill(pid: std::ffi::c_int, signal: std::ffi::c_int) -> std::ffi::c_int;
    fn getpgrp() -> std::ffi::c_int;
}

// Set only by the internal native print CLI after the outer supervisor made
// this process a group leader. Nested helpers must stay in that owned group.
static OWNED_PRINT_HOST: AtomicBool = AtomicBool::new(false);
pub(crate) fn enable_owned_print_host() -> Result<()> {
    if unsafe { getpgrp() } != i32::try_from(std::process::id())? {
        bail!("Native print host requires an owned process group");
    }
    OWNED_PRINT_HOST.store(true, Ordering::Release);
    Ok(())
}

fn nonblocking(pipe: &impl AsRawFd) -> io::Result<()> {
    // Linux F_GETFL/F_SETFL and O_NONBLOCK. Preserve all existing descriptor flags.
    // Descriptors are borrowed from live owned child pipes throughout both calls.
    let flags = unsafe { fcntl(pipe.as_raw_fd(), 3) };
    if flags < 0 || unsafe { fcntl(pipe.as_raw_fd(), 4, flags | 0x800) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

struct OwnedProcess {
    child: Child,
    group: i32,
    inherited: bool,
    finished: bool,
}
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        // The outer owner creates a separate group before exec; native-host
        // helpers join that group. Never target the reader's process group.
        if !self.inherited || !self.finished {
            // Nested helper failure aborts this print host and its entire group.
            // The external owner reaps the host and removes its private scratch.
            // Successful nested work must never kill the host that owns it.
            unsafe { kill(-self.group, 9) };
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn drain<R: Read>(pipe: &mut Option<R>, bytes: &mut Vec<u8>, limit: usize) -> Result<bool> {
    let mut progressed = false;
    // Bound work per iteration so a continuously writing helper cannot starve
    // deadline/cancel checks or the other pipe.
    for _ in 0..8 {
        let Some(reader) = pipe.as_mut() else { break };
        let mut chunk = [0; 8192];
        match reader.read(&mut chunk) {
            Ok(0) => {
                *pipe = None;
                break;
            }
            Ok(count) => {
                if count > limit.saturating_sub(bytes.len()) {
                    bail!("Helper output exceeds its byte limit");
                }
                bytes.extend_from_slice(&chunk[..count]);
                progressed = true;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error).context("Read helper output"),
        }
    }
    Ok(progressed)
}

pub(crate) fn run(
    command: &mut Command,
    input: &[u8],
    limits: Limits,
    cancel: Option<&AtomicBool>,
) -> Result<Output> {
    let timeout = Some(limits.timeout);
    run_inner(command, input, limits, timeout, cancel)
}

/// Owned native dialog: no user-interaction deadline; shutdown still cancels.
pub(crate) fn run_dialog(
    command: &mut Command,
    input: &[u8],
    cancel: &AtomicBool,
) -> Result<Output> {
    run_inner(
        command,
        input,
        Limits {
            timeout: Duration::ZERO,
            stdout: 64 * 1024,
            stderr: 64 * 1024,
        },
        None,
        Some(cancel),
    )
}

fn run_inner(
    command: &mut Command,
    input: &[u8],
    limits: Limits,
    timeout: Option<Duration>,
    cancel: Option<&AtomicBool>,
) -> Result<Output> {
    let started = Instant::now();
    let cancelled = || cancel.is_some_and(|value| value.load(Ordering::Relaxed));
    if cancelled() {
        bail!("Helper operation cancelled");
    }
    let inherited = timeout.is_some() && OWNED_PRINT_HOST.load(Ordering::Acquire);
    command.process_group(if inherited { unsafe { getpgrp() } } else { 0 });
    let child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(if limits.stderr == 0 {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .spawn()
        .context("Start helper process")?;
    // Linux process identifiers fit in signed pid_t; reject before negating.
    let group = match i32::try_from(if inherited {
        std::process::id()
    } else {
        child.id()
    }) {
        Ok(id) if id > 0 => id,
        _ => {
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
            bail!("Invalid helper process identity");
        }
    };
    let mut process = OwnedProcess {
        child,
        group,
        inherited,
        finished: false,
    };
    let mut stdin = process.child.stdin.take();
    let mut stdout = process.child.stdout.take();
    let mut stderr = process.child.stderr.take();
    if let Some(pipe) = &stdin {
        nonblocking(pipe)?;
    }
    if let Some(pipe) = &stdout {
        nonblocking(pipe)?;
    }
    if let Some(pipe) = &stderr {
        nonblocking(pipe)?;
    }
    let mut sent = 0;
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let mut status = None;
    loop {
        if cancelled() {
            bail!("Helper operation cancelled");
        }
        if timeout.is_some_and(|limit| started.elapsed() >= limit) {
            bail!("Helper operation exceeded its deadline");
        }
        let mut progressed = drain(&mut stdout, &mut output, limits.stdout)?;
        progressed |= drain(&mut stderr, &mut diagnostics, limits.stderr)?;
        if sent == input.len() {
            stdin = None;
        } else if let Some(pipe) = stdin.as_mut() {
            match pipe.write(&input[sent..input.len().min(sent.saturating_add(8192))]) {
                Ok(0) => bail!("Helper input pipe closed before input completed"),
                Ok(count) => {
                    sent += count;
                    progressed = true;
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => return Err(error).context("Write helper input"),
            }
        }
        if status.is_none() {
            status = process.child.try_wait().context("Inspect helper exit")?;
        }
        if let Some(status) = status
            && stdout.is_none()
            && stderr.is_none()
            && stdin.is_none()
        {
            process.finished = true;
            return Ok(Output {
                status,
                stdout: output,
                stderr: diagnostics,
            });
        }
        if !progressed {
            thread::sleep(Duration::from_millis(5));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn limits() -> Limits {
        Limits {
            timeout: Duration::from_millis(150),
            stdout: 4096,
            stderr: 4096,
        }
    }
    #[test]
    fn success_and_warning_status_are_preserved() {
        let mut policy = limits();
        policy.timeout = Duration::from_secs(3);
        let result = run(
            Command::new("sh").args(["-c", "cat; printf warning >&2; exit 3"]),
            b"fixture",
            policy,
            None,
        )
        .unwrap();
        assert_eq!(result.stdout, b"fixture");
        assert_eq!(result.stderr, b"warning");
        assert_eq!(result.status.code(), Some(3));
    }
    #[test]
    fn deadline_covers_child_exit_with_descendant_holding_pipes() {
        let start = Instant::now();
        let error = run(
            Command::new("sh").args(["-c", "sleep 30 & exit 0"]),
            b"",
            limits(),
            None,
        )
        .unwrap_err();
        assert!(error.to_string().contains("deadline"));
        assert!(start.elapsed() < Duration::from_secs(3));
    }
    #[test]
    fn blocked_stdin_and_flooded_output_are_bounded() {
        let start = Instant::now();
        let error = run(
            Command::new("sh").args(["-c", "sleep 30"]),
            &vec![0; 1024 * 1024],
            limits(),
            None,
        )
        .unwrap_err();
        assert!(error.to_string().contains("deadline"));
        assert!(start.elapsed() < Duration::from_secs(3));
        for stream in ["cat /dev/zero", "cat /dev/zero >&2"] {
            let error =
                run(Command::new("sh").args(["-c", stream]), b"", limits(), None).unwrap_err();
            assert!(error.to_string().contains("byte limit"));
        }
    }
    #[test]
    fn live_cancellation_stops_a_blocked_helper() {
        let cancelled = AtomicBool::new(false);
        let start = Instant::now();
        thread::scope(|scope| {
            scope.spawn(|| {
                thread::sleep(Duration::from_millis(50));
                cancelled.store(true, Ordering::Relaxed);
            });
            let mut policy = limits();
            policy.timeout = Duration::from_secs(3);
            let error = run(
                Command::new("sh").args(["-c", "sleep 30"]),
                b"",
                policy,
                Some(&cancelled),
            )
            .unwrap_err();
            assert!(error.to_string().contains("cancelled"));
        });
        assert!(start.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn cancellation_and_private_diagnostics() {
        let cancelled = AtomicBool::new(true);
        assert!(
            run(
                Command::new("sh").args(["-c", "exit 0"]),
                b"",
                limits(),
                Some(&cancelled)
            )
            .unwrap_err()
            .to_string()
            .contains("cancelled")
        );
        let mut policy = limits();
        policy.timeout = Duration::from_secs(3);
        policy.stderr = 0;
        let result = run(
            Command::new("sh").args(["-c", "cat >&2; exit 1"]),
            b"private fixture password",
            policy,
            None,
        )
        .unwrap();
        assert!(result.stderr.is_empty());
        assert!(!result.status.success());
    }
    #[test]
    fn dialog_has_no_interaction_deadline_but_cancels_blocked_io_and_bounds_output() {
        let cancel = AtomicBool::new(false);
        let start = Instant::now();
        assert!(
            run_dialog(Command::new("sh").args(["-c", "sleep .3"]), b"", &cancel)
                .unwrap()
                .status
                .success()
        );
        assert!(start.elapsed() >= Duration::from_millis(250));
        thread::scope(|scope| {
            scope.spawn(|| {
                thread::sleep(Duration::from_millis(80));
                cancel.store(true, Ordering::Release);
            });
            let start = Instant::now();
            let error = run_dialog(
                Command::new("sh").args(["-c", "sleep 30"]),
                &vec![0; 1024 * 1024],
                &cancel,
            )
            .unwrap_err();
            assert!(error.to_string().contains("cancelled"));
            assert!(start.elapsed() < Duration::from_secs(3));
        });
        cancel.store(false, Ordering::Release);
        for stream in ["cat /dev/zero", "cat /dev/zero >&2"] {
            let error =
                run_dialog(Command::new("sh").args(["-c", stream]), b"", &cancel).unwrap_err();
            assert!(error.to_string().contains("byte limit"));
        }
    }

    #[test]
    #[ignore = "owned host subprocess only; never enable group inheritance in the test runner"]
    fn owned_print_host_fixture() {
        enable_owned_print_host().unwrap();
        let mut policy = limits();
        policy.timeout = Duration::from_millis(200);
        if std::env::var("DPDF_TEST_PRINT_HOST_MODE").as_deref() == Ok("success") {
            policy.timeout = Duration::from_secs(3);
            let output = run(
                Command::new("sh").args(["-c", "printf nested-ready"]),
                b"",
                policy,
                None,
            )
            .unwrap();
            assert_eq!(output.stdout, b"nested-ready");
            println!("host survived successful nested work");
        } else {
            let pid = std::env::var_os("DPDF_TEST_PRINT_DESCENDANT").unwrap();
            let _ = run(
                Command::new("sh")
                    .args(["-c", "sleep 30 & echo $! > \"$1\"; wait", "fixture"])
                    .arg(pid),
                b"",
                policy,
                None,
            );
            panic!("A failed nested helper must abort its externally owned host group");
        }
    }

    #[test]
    fn successful_nested_helper_keeps_host_alive_and_failed_nested_helper_cannot_escape() {
        let fixture = |mode: &str| {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "helper_process::tests::owned_print_host_fixture",
                    "--ignored",
                    "--nocapture",
                ])
                .env("DPDF_TEST_PRINT_HOST_MODE", mode);
            command
        };
        let cancel = AtomicBool::new(false);
        let result = run_dialog(&mut fixture("success"), b"", &cancel).unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("host survived"));
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("descendant.pid");
        let start = Instant::now();
        let result = run_dialog(
            fixture("stall").env("DPDF_TEST_PRINT_DESCENDANT", &path),
            b"",
            &cancel,
        )
        .unwrap();
        assert!(!result.status.success());
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(result.status.signal(), Some(9));
        assert!(start.elapsed() < Duration::from_secs(3));
        let pid = std::fs::read_to_string(path).unwrap();
        let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid.trim()));
        assert!(
            stat.is_err()
                || stat
                    .unwrap()
                    .rsplit(')')
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .next()
                    == Some("Z")
        );
    }
}
