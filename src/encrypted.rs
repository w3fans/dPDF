//! Passwords travel in process memory / private stdin pipes, never command arguments.
use anyhow::{Context, Result, bail};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    sync::Arc,
};

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
