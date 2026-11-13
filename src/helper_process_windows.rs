//! Windows noninteractive helper supervision. Never used for a user-owned dialog.
use anyhow::{Context, Result, bail};
use std::{
    io::{self, Read, Write},
    os::windows::{
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    process::{Child, Command, Output, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{HANDLE, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
        },
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject,
        },
        Threading::{
            CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
        },
    },
};

pub(crate) struct Limits {
    pub timeout: Duration,
    pub stdout: usize,
    // Zero discards stderr, including anything echoed from private stdin.
    pub stderr: usize,
}

fn owned_handle(raw: HANDLE, action: &'static str) -> Result<OwnedHandle> {
    if raw.is_null() || raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error()).context(action);
    }
    // Each caller passes a newly created, uniquely owned Win32 handle.
    Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
}

struct OwnedProcess {
    child: Child,
    job: OwnedHandle,
}
impl Drop for OwnedProcess {
    fn drop(&mut self) {
        // The child was assigned before its primary thread could run. No
        // breakaway is permitted. Kill descendants before joining pipe tasks:
        // even an exited root can leave a descendant holding an inherited pipe.
        unsafe { TerminateJobObject(self.job.as_raw_handle(), 1) };
        let _ = self.child.kill();
        let _ = self.child.wait();
        // Closing our sole job handle also enforces KILL_ON_JOB_CLOSE.
    }
}

fn create_job() -> Result<OwnedHandle> {
    // Null security attributes create a noninheritable, unnamed job handle.
    let job = owned_handle(
        unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) },
        "Create helper process job",
    )?;
    let mut policy = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    policy.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // No memory/CPU/breakaway overrides: enclosing runner job limits still apply.
    if unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            (&policy as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&policy) as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error()).context("Configure helper process job");
    }
    Ok(job)
}

fn resume_child(pid: u32) -> Result<()> {
    // std::process::Child owns the process handle but not the primary thread
    // handle. The suspended process has not executed any user code. Enumerate
    // its initial thread through the documented ToolHelp interface.
    let snapshot = owned_handle(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) },
        "Inspect suspended helper thread",
    )?;
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut found = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) } != 0;
    while found {
        if entry.th32OwnerProcessID == pid {
            let thread = owned_handle(
                unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) },
                "Open suspended helper thread",
            )?;
            // CREATE_SUSPENDED establishes exactly one suspend count.
            if unsafe { ResumeThread(thread.as_raw_handle()) } != 1 {
                bail!("Cannot resume the owned helper thread");
            }
            return Ok(());
        }
        found = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) } != 0;
    }
    bail!("Cannot locate the suspended helper thread");
}

fn collect(mut pipe: impl Read, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0; 8192];
        let count = match pipe.read(&mut chunk) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result.context("Read helper output")?,
        };
        if count == 0 {
            return Ok(bytes);
        }
        if count > limit.saturating_sub(bytes.len()) {
            bail!("Helper output exceeds its byte limit");
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
}

enum PipeResult {
    Input(io::Result<()>),
    Stdout(Result<Vec<u8>>),
    Stderr(Result<Vec<u8>>),
}

pub(crate) fn run(
    command: &mut Command,
    input: &[u8],
    limits: Limits,
    cancel: Option<&AtomicBool>,
) -> Result<Output> {
    let started = Instant::now();
    let cancelled = || cancel.is_some_and(|value| value.load(Ordering::Relaxed));
    if cancelled() {
        bail!("Helper operation cancelled");
    }
    // The process guard MUST live inside this closure. Every return/unwind
    // drops it (kills the job and waits for the root) before scope joins pipe
    // threads. No writer borrows input beyond this call; no threads detach.
    thread::scope(|scope| -> Result<Output> {
        let job = create_job()?;
        let child = command
            .creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(if limits.stderr == 0 {
                Stdio::null()
            } else {
                Stdio::piped()
            })
            .spawn()
            .context("Start helper process")?;
        let mut process = OwnedProcess { child, job };
        if unsafe {
            AssignProcessToJobObject(process.job.as_raw_handle(), process.child.as_raw_handle())
        } == 0
        {
            return Err(io::Error::last_os_error()).context("Own helper process tree");
        }
        resume_child(process.child.id())?;

        let (sender, receiver) = mpsc::channel();
        let stdout = process.child.stdout.take().context("Open helper output")?;
        let reply = sender.clone();
        thread::Builder::new()
            .name("dpdf-helper-output".into())
            .spawn_scoped(scope, move || {
                let _ = reply.send(PipeResult::Stdout(collect(stdout, limits.stdout)));
            })
            .context("Start helper output reader")?;
        let mut diagnostics = if limits.stderr == 0 {
            Some(Vec::new())
        } else {
            let stderr = process
                .child
                .stderr
                .take()
                .context("Open helper diagnostics")?;
            let reply = sender.clone();
            thread::Builder::new()
                .name("dpdf-helper-errors".into())
                .spawn_scoped(scope, move || {
                    let _ = reply.send(PipeResult::Stderr(collect(stderr, limits.stderr)));
                })
                .context("Start helper diagnostic reader")?;
            None
        };
        let mut stdin = process.child.stdin.take().context("Open helper input")?;
        thread::Builder::new()
            .name("dpdf-helper-input".into())
            .spawn_scoped(scope, move || {
                let result = stdin.write_all(input);
                drop(stdin);
                let _ = sender.send(PipeResult::Input(result));
            })
            .context("Start helper input writer")?;
        // Each task sends exactly one message, containing at most its byte cap
        // plus one. There is no channel of unbounded streaming output chunks.
        let mut output = None;
        let mut input_done = false;
        let mut status = None;
        loop {
            if cancelled() {
                bail!("Helper operation cancelled");
            }
            if started.elapsed() >= limits.timeout {
                bail!("Helper operation exceeded its deadline");
            }
            while let Ok(message) = receiver.try_recv() {
                match message {
                    PipeResult::Input(result) => {
                        result.context("Write helper input")?;
                        input_done = true;
                    }
                    PipeResult::Stdout(result) => output = Some(result?),
                    PipeResult::Stderr(result) => diagnostics = Some(result?),
                }
            }
            if status.is_none() {
                status = process.child.try_wait().context("Inspect helper exit")?;
            }
            if input_done
                && let Some(status) = status
                && let (Some(stdout), Some(stderr)) = (&mut output, &mut diagnostics)
            {
                return Ok(Output {
                    status,
                    stdout: std::mem::take(stdout),
                    stderr: std::mem::take(stderr),
                });
            }
            thread::sleep(Duration::from_millis(5));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "helper_process::tests::fixture_process",
                "--ignored",
                "--nocapture",
            ])
            .env("DPDF_TEST_HELPER_MODE", mode);
        command
    }

    fn limits() -> Limits {
        Limits {
            timeout: Duration::from_secs(3),
            stdout: 4096,
            stderr: 4096,
        }
    }

    // Only a generated test subprocess enters this function. This avoids an
    // installed shell, PowerShell startup/profile, or external test binary.
    #[test]
    #[ignore = "subprocess fixture invoked explicitly by supervisor tests"]
    fn fixture_process() {
        let mode = std::env::var("DPDF_TEST_HELPER_MODE").unwrap();
        match mode.as_str() {
            "finite" => thread::sleep(Duration::from_millis(250)),
            "sleep" => thread::sleep(Duration::from_secs(30)),
            "stdout" | "stderr" => {
                let mut writer: Box<dyn Write> = if mode == "stdout" {
                    Box::new(io::stdout())
                } else {
                    Box::new(io::stderr())
                };
                loop {
                    writer.write_all(&[b'x'; 8192]).unwrap();
                    writer.flush().unwrap();
                }
            }
            "private" => {
                let mut input = Vec::new();
                io::stdin().read_to_end(&mut input).unwrap();
                io::stderr().write_all(&input).unwrap();
                std::process::exit(1);
            }
            "warning" => {
                let mut input = Vec::new();
                io::stdin().read_to_end(&mut input).unwrap();
                io::stdout().write_all(&input).unwrap();
                io::stdout().flush().unwrap();
                io::stderr().write_all(b"warning fixture").unwrap();
                std::process::exit(3);
            }
            "descendant" => {
                let child = fixture("sleep")
                    .stdin(Stdio::null())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .unwrap();
                std::fs::write(
                    std::env::var_os("DPDF_TEST_HELPER_PID").unwrap(),
                    child.id().to_string(),
                )
                .unwrap();
                std::process::exit(0);
            }
            _ => panic!("Unknown generated helper mode"),
        }
    }

    #[test]
    fn warning_input_and_private_diagnostics_are_preserved() {
        let result = run(&mut fixture("warning"), b"fixture input", limits(), None).unwrap();
        assert_eq!(result.status.code(), Some(3));
        assert!(result.stdout.ends_with(b"fixture input"));
        assert_eq!(result.stderr, b"warning fixture");
        let mut policy = limits();
        policy.stderr = 0;
        let result = run(
            &mut fixture("private"),
            b"private fixture password",
            policy,
            None,
        )
        .unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn stalled_input_and_output_floods_are_bounded() {
        let start = Instant::now();
        assert!(fixture("finite").output().unwrap().status.success());
        assert!(start.elapsed() >= Duration::from_millis(200));
        let start = Instant::now();
        let error = run(&mut fixture("sleep"), &vec![0; 1024 * 1024], limits(), None).unwrap_err();
        assert!(error.to_string().contains("deadline"));
        assert!(start.elapsed() < Duration::from_secs(8));
        for mode in ["stdout", "stderr"] {
            let error = run(&mut fixture(mode), b"", limits(), None).unwrap_err();
            assert!(error.to_string().contains("byte limit"));
        }
        assert!(
            run(&mut fixture("finite"), b"", limits(), None)
                .unwrap()
                .status
                .success()
        );
    }

    #[test]
    fn exited_root_cannot_leave_a_pipe_holding_descendant() {
        use windows_sys::Win32::{
            Foundation::{ERROR_INVALID_PARAMETER, WAIT_OBJECT_0},
            System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
        };
        let folder = tempfile::tempdir().unwrap();
        let pid_path = folder.path().join("descendant.pid");
        let start = Instant::now();
        let error = run(
            fixture("descendant").env("DPDF_TEST_HELPER_PID", &pid_path),
            b"",
            limits(),
            None,
        )
        .unwrap_err();
        assert!(error.to_string().contains("deadline"));
        assert!(start.elapsed() < Duration::from_secs(8));
        let pid = std::fs::read_to_string(pid_path).unwrap().parse().unwrap();
        let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        if raw.is_null() {
            assert_eq!(
                io::Error::last_os_error().raw_os_error(),
                Some(ERROR_INVALID_PARAMETER as i32)
            );
        } else {
            let process = owned_handle(raw, "Inspect fixture descendant").unwrap();
            assert_eq!(
                unsafe { WaitForSingleObject(process.as_raw_handle(), 3000) },
                WAIT_OBJECT_0
            );
        }
    }

    #[test]
    fn prestart_and_live_cancellation_are_bounded() {
        let cancelled = AtomicBool::new(true);
        let error = run(&mut fixture("sleep"), b"", limits(), Some(&cancelled)).unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        cancelled.store(false, Ordering::Relaxed);
        let start = Instant::now();
        thread::scope(|scope| {
            scope.spawn(|| {
                thread::sleep(Duration::from_millis(150));
                cancelled.store(true, Ordering::Relaxed);
            });
            let error = run(&mut fixture("sleep"), b"", limits(), Some(&cancelled)).unwrap_err();
            assert!(error.to_string().contains("cancelled"));
        });
        assert!(start.elapsed() < Duration::from_secs(8));
    }
}
