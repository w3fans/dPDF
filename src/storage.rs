//! Shared atomic replacement for PDFs and small application metadata.
use anyhow::{Context, Result, bail};
use std::{io::Write, path::Path};

/// Protect both path spelling and existing file identity (drive/share aliases,
/// symlinks and hard links). Missing inputs remain usable as in-memory snapshots.
pub fn ensure_separate_output(destination: &Path, sources: &[&Path]) -> Result<()> {
    let resolve = |path: &Path| -> Result<std::path::PathBuf> {
        if let Ok(path) = path.canonicalize() {
            return Ok(path);
        }
        let absolute = std::path::absolute(path)?;
        Ok(absolute
            .parent()
            .and_then(|parent| parent.canonicalize().ok())
            .and_then(|parent| absolute.file_name().map(|name| parent.join(name)))
            .unwrap_or(absolute))
    };
    let resolved = resolve(destination)?;
    let handle = |path: &Path| -> Result<Option<same_file::Handle>> {
        match same_file::Handle::from_path(path) {
            Ok(handle) => Ok(Some(handle)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error)
                .with_context(|| format!("Cannot verify file identity for {}", path.display())),
        }
    };
    let output = handle(destination)?;
    for source in sources {
        if resolved == resolve(source)? {
            bail!("Choose a separate output file to preserve the input (including its aliases)");
        }
        if let Some(output) = &output
            && let Some(input) = handle(source)?
            && *output == input
        {
            bail!(
                "Output identifies a protected input file through another path; choose a separate file"
            );
        }
    }
    Ok(())
}

pub fn atomic_write(bytes: &[u8], destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .context("Create temporary file beside destination")?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(destination)
        .map_err(|e| e.error)
        .context("Atomically replace destination")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protected_inputs_reject_hardlinks_and_keep_missing_snapshot_names() {
        let folder = tempfile::tempdir().unwrap();
        let source = folder.path().join("source.pdf");
        let alias = folder.path().join("alias.pdf");
        std::fs::write(&source, b"original").unwrap();
        std::fs::hard_link(&source, &alias).unwrap();
        assert!(ensure_separate_output(&alias, &[&source]).is_err());
        assert!(ensure_separate_output(&source, &[&source]).is_err());
        assert!(ensure_separate_output(&folder.path().join("copy.pdf"), &[&source]).is_ok());
        std::fs::remove_file(&source).unwrap();
        assert!(ensure_separate_output(&source, &[&source]).is_err());
        assert!(ensure_separate_output(&folder.path().join("copy.pdf"), &[&source]).is_ok());
        assert_eq!(std::fs::read(&alias).unwrap(), b"original");
    }
}
