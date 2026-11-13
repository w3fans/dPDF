//! Passwords travel in process memory / private stdin pipes, never command arguments.
use anyhow::{Context, Result, bail};
#[cfg(not(target_os = "linux"))]
use std::{
    io::{Read, Write},
    process::Stdio,
};
use std::{process::Command, sync::Arc};

pub fn validate_password(password: &str) -> Result<()> {
    if password.len() > 4096 || password.chars().any(|c| matches!(c, '\0' | '\r' | '\n')) {
        bail!("Use a single-line password of at most 4096 UTF-8 bytes");
    }
    Ok(())
}

pub fn decrypt(bytes: &[u8], password: &str) -> Result<Arc<Vec<u8>>> {
    decrypt_with(bytes, password, &crate::assembly::qpdf_executable())
}
fn decrypt_with(
    bytes: &[u8],
    password: &str,
    executable: &std::path::Path,
) -> Result<Arc<Vec<u8>>> {
    validate_password(password)?;
    if bytes.len() > 256 * 1024 * 1024 {
        bail!("Encrypted editing/printing supports PDF snapshots up to 256 MiB");
    }
    let scratch = tempfile::tempdir().context("Create private encrypted-PDF workspace")?;
    let input = scratch.path().join("encrypted.pdf");
    std::fs::write(&input, bytes)?;
    #[cfg(target_os = "linux")]
    let (data, status) = {
        let mut private_input = password.as_bytes().to_vec();
        private_input.push(b'\n');
        let result = crate::helper_process::run(
            Command::new(executable)
                .arg("--password-file=-")
                .arg("--decrypt")
                .arg("--stream-data=preserve")
                .arg(crate::assembly::path_string(&input)?.as_ref())
                .arg("-"),
            &private_input,
            crate::helper_process::Limits {
                timeout: std::time::Duration::from_secs(30),
                stdout: 256 * 1024 * 1024,
                stderr: 0,
            },
            None,
        )?;
        (result.stdout, result.status)
    };
    #[cfg(not(target_os = "linux"))]
    let (data, status) = {
        let mut child = Command::new(executable)
            .arg("--password-file=-")
            .arg("--decrypt")
            .arg("--stream-data=preserve")
            .arg(crate::assembly::path_string(&input)?.as_ref())
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("Start qpdf for encrypted editing/printing")?;
        let written = (|| -> Result<()> {
            let mut stdin = child.stdin.take().context("Open private password pipe")?;
            stdin.write_all(password.as_bytes())?;
            stdin.write_all(b"\n")?;
            Ok(())
        })();
        if let Err(error) = written {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error).context("Write private password pipe");
        }
        let mut data = Vec::new();
        let read = child
            .stdout
            .take()
            .context("Open unlocked-PDF pipe")?
            .take(256 * 1024 * 1024 + 1)
            .read_to_end(&mut data);
        if read.is_err() || data.len() > 256 * 1024 * 1024 {
            let _ = child.kill();
            let _ = child.wait();
            read?;
            bail!("Unlocked PDF exceeds its 256-MiB limit");
        }
        let status = child.wait()?;
        (data, status)
    };
    if !status.success() {
        // Child diagnostics are discarded: a helper must never echo a password into status/logs.
        bail!("qpdf could not prepare this unlocked PDF (exit {status}); original preserved");
    }
    if data.is_empty() {
        bail!("qpdf returned an empty unlocked PDF");
    }
    Ok(Arc::new(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn helper_receives_password_only_on_stdin_and_cannot_echo_it_into_errors() {
        use std::os::unix::fs::PermissionsExt;
        let folder = tempfile::tempdir().unwrap();
        let helper = folder.path().join("qpdf-fixture");
        let args = folder.path().join("args");
        let stdin = folder.path().join("stdin");
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\ncat > '{}'\ncat \"$4\"\n",
            args.display(),
            stdin.display()
        );
        std::fs::write(&helper, script).unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
        let password = "  synthetic secret čšž  ";
        let original = b"%PDF fixture bytes";
        assert_eq!(
            decrypt_with(original, password, &helper)
                .unwrap()
                .as_slice(),
            original
        );
        assert_eq!(
            std::fs::read_to_string(&stdin).unwrap(),
            format!("{password}\n")
        );
        assert!(!std::fs::read_to_string(&args).unwrap().contains(password));
        std::fs::write(&helper, "#!/bin/sh\ncat >&2\nexit 1\n").unwrap();
        let error = decrypt_with(original, password, &helper)
            .unwrap_err()
            .to_string();
        assert!(!error.contains(password));
    }
    // Characterize the baseline stall before changing process supervision. The
    // fixture releases cooperatively and has its own finite fallback lifetime.
    #[cfg(unix)]
    #[test]
    fn stalled_helper_holds_decryption_until_fixture_release() {
        use std::{
            os::unix::fs::PermissionsExt,
            sync::mpsc,
            thread,
            time::{Duration, Instant},
        };
        let folder = tempfile::tempdir().unwrap();
        let helper = folder.path().join("qpdf-stall-fixture");
        let ready = folder.path().join("ready");
        let release = folder.path().join("release");
        let script = format!(
            "#!/bin/sh\ncat >/dev/null\nprintf ready > '{}'\ni=0\nwhile [ ! -e '{}' ] && [ \"$i\" -lt 100 ]; do sleep 0.05; i=$((i+1)); done\ncat \"$4\"\n",
            ready.display(),
            release.display()
        );
        std::fs::write(&helper, script).unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
        let original = b"%PDF generated stalled-helper snapshot";
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            let result = decrypt_with(original, "generated password", &helper)
                .map_err(|error| error.to_string());
            let _ = tx.send(result);
        });
        let startup = Instant::now();
        while !ready.exists() && startup.elapsed() < Duration::from_secs(5) {
            thread::sleep(Duration::from_millis(10));
        }
        let started = ready.exists();
        let stalled = matches!(
            rx.recv_timeout(Duration::from_millis(250)),
            Err(mpsc::RecvTimeoutError::Timeout)
        );
        // Release before assertions so a failed observation still cleans up.
        std::fs::write(&release, b"release").unwrap();
        let result = rx.recv_timeout(Duration::from_secs(10));
        worker.join().unwrap();
        assert!(
            started,
            "Generated helper did not reach its controlled stall"
        );
        assert!(
            stalled,
            "Decryption unexpectedly completed before fixture release"
        );
        assert_eq!(result.unwrap().unwrap().as_slice(), original);
    }

    #[test]
    fn password_validation_preserves_spaces_and_unicode_without_echoing_rejected_input() {
        assert!(validate_password("  Žig čšž \"$`  ").is_ok());
        assert!(validate_password("").is_ok());
        for password in [
            "private\nvalue".to_string(),
            "private\0value".to_string(),
            "x".repeat(4097),
        ] {
            let error = validate_password(&password).unwrap_err().to_string();
            assert!(!error.contains(&password));
        }
    }
}
