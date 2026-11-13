//! qpdf page assembly against immutable snapshots, validated before atomic destination replacement.
use crate::pages::{PagePlan, parse_selection, ranges};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::Arc,
};
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
use std::{io::Read, process::Stdio};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationKind {
    Replace,
    Insert,
    Remove,
    Extract,
    Merge,
    RotateRight,
    RotateLeft,
    Reorder,
}

impl OperationKind {
    pub fn from_name(name: &str) -> Result<Self> {
        Ok(match name {
            "replace" => Self::Replace,
            "insert" => Self::Insert,
            "remove" => Self::Remove,
            "extract" => Self::Extract,
            "merge" => Self::Merge,
            "rotate-right" => Self::RotateRight,
            "rotate-left" => Self::RotateLeft,
            "reorder" => Self::Reorder,
            _ => bail!("Unknown page operation: {name}"),
        })
    }
    pub fn from_index(index: i32) -> Self {
        match index {
            1 => Self::Insert,
            2 => Self::Remove,
            3 => Self::Extract,
            4 => Self::Merge,
            5 => Self::RotateRight,
            6 => Self::RotateLeft,
            7 => Self::Reorder,
            _ => Self::Replace,
        }
    }
    pub fn uses_source(self) -> bool {
        matches!(self, Self::Replace | Self::Insert | Self::Merge)
    }
}

#[derive(Clone, Debug)]
pub struct PageOperation {
    pub kind: OperationKind,
    pub pages: String,
    pub source_pages: String,
}

#[derive(Clone)]
pub struct SourceSnapshot {
    pub path: PathBuf,
    pub bytes: Arc<Vec<u8>>,
}

/// Helper-produced bytes awaiting publication by the document-owning worker.
/// Dropping a stale/cancelled completion deletes its staging file.
pub(crate) struct StagedExport {
    temporary: tempfile::TempPath,
    destination: PathBuf,
    protected: Vec<PathBuf>,
    pages: usize,
}

impl StagedExport {
    pub fn new(destination: &Path, protected: Vec<PathBuf>) -> Result<Self> {
        let parent = destination
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .canonicalize()
            .context("Find output directory")?;
        let destination = parent.join(
            destination
                .file_name()
                .context("Choose an output PDF filename")?,
        );
        let inputs: Vec<_> = protected.iter().map(PathBuf::as_path).collect();
        crate::storage::ensure_separate_output(&destination, &inputs)?;
        let temporary = tempfile::NamedTempFile::new_in(&parent)?.into_temp_path();
        Ok(Self {
            temporary,
            destination,
            protected,
            pages: 0,
        })
    }
    pub fn path(&self) -> &Path {
        &self.temporary
    }
    pub fn with_pages(mut self, pages: usize) -> Self {
        self.pages = pages;
        self
    }
    pub fn publish(self) -> Result<ExportResult> {
        ensure!(self.pages > 0, "Prepared export has no pages");
        let inputs: Vec<_> = self.protected.iter().map(PathBuf::as_path).collect();
        crate::storage::ensure_separate_output(&self.destination, &inputs)?;
        self.temporary
            .persist(&self.destination)
            .map_err(|error| error.error)
            .context("Atomically save prepared PDF")?;
        Ok(ExportResult {
            path: self.destination,
            pages: self.pages,
        })
    }
}

pub struct ExportResult {
    pub path: PathBuf,
    pub pages: usize,
}

pub(crate) fn qpdf_executable() -> PathBuf {
    if let Some(path) = std::env::var_os("DPDF_QPDF_BINARY") {
        return PathBuf::from(path);
    }
    let name = if cfg!(target_os = "windows") {
        "qpdf.exe"
    } else {
        "qpdf"
    };
    if let Some(path) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join(name)))
        && path.is_file()
    {
        return path;
    }
    name.into()
}

fn checked(output: Output, action: &str) -> Result<Output> {
    if !output.status.success() {
        // qpdf code 3 means warnings. Do not silently export repaired input as clean output.
        bail!(
            "{action}: qpdf exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(2000)
                .collect::<String>()
                .trim()
        );
    }
    Ok(output)
}

/// Bounded scalar/status qpdf calls. Metadata uses its separate 64-MiB policy.
pub(crate) fn run_qpdf(command: &mut Command, action: &str) -> Result<Output> {
    run_qpdf_cancellable(command, action, None)
}

pub(crate) fn run_qpdf_cancellable(
    command: &mut Command,
    action: &str,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Output> {
    crate::background_jobs::check_cancelled(cancel)?;
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let output = crate::helper_process::run(
        command,
        b"",
        crate::helper_process::Limits {
            timeout: std::time::Duration::from_secs(60),
            stdout: 64 * 1024,
            stderr: 64 * 1024,
        },
        cancel,
    )
    .with_context(|| format!("Run qpdf: {action}"))?;
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let output = command
        .output()
        .with_context(|| format!("Run qpdf: {action}"))?;
    checked(output, action)
}

pub(crate) fn inspect_metadata(path: &Path) -> Result<Value> {
    inspect_metadata_cancellable(path, None)
}
pub(crate) fn inspect_metadata_cancellable(
    path: &Path,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Value> {
    crate::background_jobs::check_cancelled(cancel)?;
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        inspect_metadata_with_cancel(
            path,
            &qpdf_executable(),
            crate::helper_process::Limits {
                timeout: std::time::Duration::from_secs(30),
                stdout: 64 * 1024 * 1024,
                stderr: 0,
            },
            cancel,
        )
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let mut child = Command::new(qpdf_executable())
            .arg("--json")
            .arg("--json-stream-data=none")
            .arg(path_string(path)?.as_ref())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("Start qpdf metadata inspection")?;
        let mut bytes = Vec::new();
        let read = child
            .stdout
            .take()
            .context("PDF metadata inspection pipe")?
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes);
        if read.is_err() || bytes.len() > 64 * 1024 * 1024 {
            let _ = child.kill();
            let _ = child.wait();
            read?;
            bail!("PDF metadata inspection exceeds 64 MiB");
        }
        if !child.wait()?.success() {
            bail!("qpdf could not inspect PDF metadata");
        }
        serde_json::from_slice(&bytes).context("Read qpdf document metadata")
    }
}

#[cfg(all(test, target_os = "linux"))]
fn inspect_metadata_with(
    path: &Path,
    executable: &Path,
    limits: crate::helper_process::Limits,
) -> Result<Value> {
    inspect_metadata_with_cancel(path, executable, limits, None)
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn inspect_metadata_with_cancel(
    path: &Path,
    executable: &Path,
    limits: crate::helper_process::Limits,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Value> {
    let output = crate::helper_process::run(
        Command::new(executable)
            .arg("--json")
            .arg("--json-stream-data=none")
            .arg(path_string(path)?.as_ref()),
        b"",
        limits,
        cancel,
    )
    .context("Inspect PDF metadata")?;
    if !output.status.success() {
        bail!("qpdf could not inspect PDF metadata");
    }
    serde_json::from_slice(&output.stdout).context("Read qpdf document metadata")
}

/// One immutable metadata snapshot shared by independent page-unit/review readers.
pub(crate) fn inspect_snapshot(bytes: &[u8]) -> Result<Value> {
    inspect_snapshot_cancellable(bytes, None)
}
pub(crate) fn inspect_snapshot_cancellable(
    bytes: &[u8],
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Value> {
    crate::background_jobs::check_cancelled(cancel)?;
    let directory = tempfile::Builder::new()
        .prefix("dpdf-metadata-")
        .tempdir()?;
    let input = directory.path().join("snapshot.pdf");
    std::fs::write(&input, bytes).context("Stage PDF metadata snapshot")?;
    inspect_metadata_cancellable(&input, cancel)
}

#[cfg(target_os = "linux")]
pub(crate) fn page_count(executable: &Path, path: &Path) -> Result<usize> {
    page_count_cancellable(executable, path, None)
}

fn page_count_cancellable(
    executable: &Path,
    path: &Path,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<usize> {
    let output = run_qpdf_cancellable(
        Command::new(executable)
            .arg("--show-npages")
            .arg(path_string(path)?.as_ref()),
        "Inspect PDF",
        cancel,
    )?;
    let count: usize = String::from_utf8(output.stdout)?
        .trim()
        .parse()
        .context("Read qpdf page count")?;
    if count == 0 || count > 100_000 {
        bail!("Page operations support between 1 and 100,000 pages per source PDF");
    }
    Ok(count)
}

pub fn build_plan(counts: Vec<usize>, operation: &PageOperation) -> Result<PagePlan> {
    let mut plan = PagePlan::new(counts.clone())?;
    match operation.kind {
        OperationKind::Replace => {
            if counts.len() != 2 {
                bail!("Choose one replacement PDF or PNG/JPEG image");
            }
            let target = parse_selection(&operation.pages, counts[0])?;
            let source = parse_selection(&operation.source_pages, counts[1])?;
            if target.len() != 1 || source.len() != 1 {
                bail!("Choose one target page and one replacement source page");
            }
            plan.replace(target[0], 1, source[0])?;
        }
        OperationKind::Insert => {
            if counts.len() != 2 {
                bail!("Choose one PDF or PNG/JPEG image to insert");
            }
            let before: usize = operation
                .pages
                .trim()
                .parse()
                .context("Enter the page before which to insert")?;
            if before == 0 || before > counts[0] + 1 {
                bail!("Insertion position must be between 1 and {}", counts[0] + 1);
            }
            plan.insert(
                before - 1,
                1,
                &parse_selection(&operation.source_pages, counts[1])?,
            )?;
        }
        OperationKind::Merge => {
            if counts.len() < 2 {
                bail!("Choose at least one PDF or PNG/JPEG image to merge");
            }
            for (source, count) in counts.iter().enumerate().skip(1) {
                plan.insert(
                    plan.pages().len(),
                    source,
                    &parse_selection(&operation.source_pages, *count)?,
                )?;
            }
        }
        OperationKind::Remove => plan.remove(&parse_selection(&operation.pages, counts[0])?)?,
        OperationKind::Extract => plan.extract(&parse_selection(&operation.pages, counts[0])?)?,
        OperationKind::Reorder => plan.reorder(&parse_selection(&operation.pages, counts[0])?)?,
        OperationKind::RotateRight | OperationKind::RotateLeft => plan.rotate(
            &parse_selection(&operation.pages, counts[0])?,
            if operation.kind == OperationKind::RotateRight {
                1
            } else {
                -1
            },
        )?,
    }
    if plan.pages().len() > 100_000 {
        bail!("Select at most 100,000 output pages");
    }
    Ok(plan)
}

pub(crate) fn path_string(path: &Path) -> Result<Cow<'_, str>> {
    let text = path
        .to_str()
        .context("PDF page operations require Unicode file paths")?;
    #[cfg(windows)]
    {
        windows_helper_path(text)
    }
    #[cfg(not(windows))]
    {
        Ok(Cow::Borrowed(text))
    }
}

// qpdf's CLI parses verbatim Windows prefixes differently from its JSON reader.
// Keep Rust's canonical paths internally; normalize only the helper boundary.
#[cfg(any(windows, test))]
fn windows_helper_path(text: &str) -> Result<Cow<'_, str>> {
    let Some(tail) = text.strip_prefix(r"\\?\") else {
        return Ok(Cow::Borrowed(text));
    };
    let bytes = tail.as_bytes();
    let normalized = if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && &bytes[1..3] == b":\\"
    {
        tail.to_owned()
    } else if tail
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("UNC\\"))
    {
        format!(r"\\{}", &tail[4..])
    } else {
        bail!("qpdf supports local-drive and UNC paths, not Windows device paths");
    };
    if normalized
        .split('\\')
        .any(|part| part.ends_with([' ', '.']))
    {
        bail!("qpdf cannot safely normalize Windows path components ending in a dot or space");
    }
    Ok(Cow::Owned(normalized))
}

fn job(
    plan: &PagePlan,
    snapshots: &[PathBuf],
    output: &Path,
    preserve_catalog: bool,
    qpdf_major: u16,
) -> Result<Value> {
    if snapshots.len() != plan.source_count() {
        bail!("Source snapshot count does not match page plan");
    }
    let mut pages = Vec::new();
    let mut index = 0;
    while index < plan.pages().len() {
        let source = plan.pages()[index].source;
        let start = index;
        while index < plan.pages().len() && plan.pages()[index].source == source {
            index += 1;
        }
        let selection: Vec<_> = plan.pages()[start..index].iter().map(|p| p.page).collect();
        pages.push(json!({ "file": if source == 0 && preserve_catalog { Cow::Borrowed(".") } else { path_string(&snapshots[source])? }, "range": ranges(&selection) }));
    }
    let mut rotations = Vec::new();
    for turns in 1..4 {
        let selected: Vec<_> = plan
            .pages()
            .iter()
            .enumerate()
            .filter(|(_, p)| p.turns == turns)
            .map(|(i, _)| i)
            .collect();
        if !selected.is_empty() {
            rotations.push(format!("+{}:{}", turns as u16 * 90, ranges(&selected)));
        }
    }
    let mut value = json!({ "outputFile": path_string(output)?, "pages": pages, "streamData": "preserve", "objectStreams": "preserve" });
    // qpdf 12 accepts a list of rotations; qpdf 11 job JSON accepts one string only.
    if qpdf_major >= 12 {
        value["rotate"] = json!(rotations);
    } else if rotations.len() > 1 {
        bail!("Rotating pages by different angles in one export requires qpdf 12 or newer");
    } else if let Some(rotation) = rotations.pop() {
        value["rotate"] = json!(rotation);
    }
    if preserve_catalog {
        value["inputFile"] = json!(path_string(&snapshots[0])?);
    } else {
        value["empty"] = json!("");
    }
    Ok(value)
}

/// Writes a separate assembled file. Inputs are snapshots, and input paths cannot be destinations.
pub fn export(
    sources: &[SourceSnapshot],
    operation: &PageOperation,
    destination: &Path,
) -> Result<ExportResult> {
    export_plan(
        sources,
        destination,
        operation.kind != OperationKind::Extract,
        |counts| build_plan(counts, operation),
    )
}

pub fn export_plan(
    sources: &[SourceSnapshot],
    destination: &Path,
    preserve_catalog: bool,
    create_plan: impl FnOnce(Vec<usize>) -> Result<PagePlan>,
) -> Result<ExportResult> {
    export_plan_cancellable(sources, destination, preserve_catalog, create_plan, None)
}

pub(crate) fn export_cancellable(
    sources: &[SourceSnapshot],
    operation: &PageOperation,
    destination: &Path,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<ExportResult> {
    export_plan_cancellable(
        sources,
        destination,
        operation.kind != OperationKind::Extract,
        |counts| build_plan(counts, operation),
        cancel,
    )
}

pub(crate) fn export_plan_cancellable(
    sources: &[SourceSnapshot],
    destination: &Path,
    preserve_catalog: bool,
    create_plan: impl FnOnce(Vec<usize>) -> Result<PagePlan>,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<ExportResult> {
    crate::background_jobs::check_cancelled(cancel)?;
    if sources.is_empty() {
        bail!("Open a PDF before assembling pages");
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .context("Find output directory")?;
    let destination = parent.join(
        destination
            .file_name()
            .context("Choose an output PDF filename")?,
    );
    let protected: Vec<_> = sources.iter().map(|source| source.path.as_path()).collect();
    crate::storage::ensure_separate_output(&destination, &protected)?;
    let executable = qpdf_executable();
    let version = run_qpdf_cancellable(
        Command::new(&executable).arg("--version"),
        "Check qpdf",
        cancel,
    )?;
    let text = String::from_utf8_lossy(&version.stdout);
    let major = text
        .split_whitespace()
        .nth(2)
        .and_then(|v| v.split('.').next())
        .and_then(|v| v.parse::<u16>().ok());
    let Some(major) = major.filter(|major| *major >= 11) else {
        bail!("Page editing requires qpdf 11 or newer");
    };
    let scratch = tempfile::tempdir().context("Create private page-assembly workspace")?;
    let mut snapshots = Vec::new();
    let mut counts = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        crate::background_jobs::check_cancelled(cancel)?;
        let path = scratch.path().join(format!("source-{index}.pdf"));
        std::fs::write(&path, &*source.bytes)?;
        counts.push(page_count_cancellable(&executable, &path, cancel)?);
        snapshots.push(path);
    }
    let plan = create_plan(counts)?;
    if plan.pages().is_empty() || plan.pages().len() > 100_000 {
        bail!("Select between 1 and 100,000 output pages");
    }
    let bookmarks = if preserve_catalog {
        crate::page_bookmarks::capture(&snapshots[0], &plan, cancel)?
    } else {
        Vec::new()
    };
    // qpdf must open the output itself. Keeping NamedTempFile's handle open denies
    // that write on Windows. TempPath retains cleanup ownership with the handle closed.
    let temporary = tempfile::NamedTempFile::new_in(&parent)
        .context("Create output beside destination")?
        .into_temp_path();
    let job_path = scratch.path().join("assembly.json");
    std::fs::write(
        &job_path,
        serde_json::to_vec(&job(
            &plan,
            &snapshots,
            &temporary,
            preserve_catalog,
            major,
        )?)?,
    )?;
    run_qpdf_cancellable(
        Command::new(&executable).arg(format!("--job-json-file={}", path_string(&job_path)?)),
        "Assemble pages",
        cancel,
    )?;
    crate::page_bookmarks::restore(&bookmarks, &temporary, scratch.path(), cancel)?;
    run_qpdf_cancellable(
        Command::new(&executable)
            .arg("--check")
            .arg(path_string(&temporary)?.as_ref()),
        "Validate assembled PDF",
        cancel,
    )?;
    if page_count_cancellable(&executable, &temporary, cancel)? != plan.pages().len() {
        bail!("Assembled PDF has an unexpected page count");
    }
    // Flush with write access (required by Windows), then close before atomic rename.
    std::fs::OpenOptions::new()
        .write(true)
        .open(&temporary)?
        .sync_all()?;
    crate::storage::ensure_separate_output(&destination, &protected)?;
    crate::background_jobs::check_cancelled(cancel)?;
    temporary
        .persist(&destination)
        .map_err(|error| error.error)
        .context("Atomically save assembled PDF")?;
    Ok(ExportResult {
        path: destination,
        pages: plan.pages().len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staged_exports_preserve_destination_until_publication_and_recheck_aliases() {
        let folder = tempfile::tempdir().unwrap();
        let source = folder.path().join("source.pdf");
        let destination = folder.path().join("existing.pdf");
        std::fs::write(&source, b"immutable generated source").unwrap();
        std::fs::write(&destination, b"previous output").unwrap();
        let staged = StagedExport::new(&destination, vec![source.clone()]).unwrap();
        let scratch = staged.path().to_owned();
        std::fs::write(&scratch, b"prepared bytes").unwrap();
        drop(staged.with_pages(1)); // Closed document / stale reply.
        assert_eq!(std::fs::read(&destination).unwrap(), b"previous output");
        assert!(!scratch.exists());

        let staged = StagedExport::new(&destination, vec![source.clone()]).unwrap();
        let scratch = staged.path().to_owned();
        std::fs::write(&scratch, b"prepared bytes").unwrap();
        std::fs::remove_file(&destination).unwrap();
        std::fs::hard_link(&source, &destination).unwrap();
        assert!(staged.with_pages(1).publish().is_err());
        assert_eq!(
            std::fs::read(&source).unwrap(),
            b"immutable generated source"
        );
        assert!(!scratch.exists());

        std::fs::remove_file(&destination).unwrap();
        let staged = StagedExport::new(&destination, vec![source]).unwrap();
        std::fs::write(staged.path(), b"validated prepared bytes").unwrap();
        let result = staged.with_pages(2).publish().unwrap();
        assert_eq!(result.pages, 2);
        assert_eq!(
            std::fs::read(result.path).unwrap(),
            b"validated prepared bytes"
        );
    }
    #[test]
    fn helper_paths_normalize_drive_and_unc_without_changing_rust_paths() {
        for (input, expected) in [
            (r"\\?\C:\temp\signed copy.pdf", r"C:\temp\signed copy.pdf"),
            (
                r"\\?\UNC\fixture-server\Office files\Žig.pdf",
                r"\\fixture-server\Office files\Žig.pdf",
            ),
            (r"C:\ordinary.pdf", r"C:\ordinary.pdf"),
            (
                r"\\fixture-server\share\copy.pdf",
                r"\\fixture-server\share\copy.pdf",
            ),
        ] {
            assert_eq!(windows_helper_path(input).unwrap(), expected);
        }
        assert!(windows_helper_path(r"\\?\GLOBALROOT\Device\file.pdf").is_err());
        assert!(windows_helper_path(r"\\?\C:\folder.\copy.pdf").is_err());
        assert!(windows_helper_path(r"\\?\C:\folder \copy.pdf").is_err());
    }
    #[test]
    fn assembly_job_preserves_source_groups_and_applies_relative_output_rotation() {
        let mut plan = PagePlan::new(vec![10, 1]).unwrap();
        plan.replace(9, 1, 0).unwrap();
        plan.rotate(&[9], 1).unwrap();
        let value = job(
            &plan,
            &[
                "original snapshot.pdf".into(),
                "replacement \"page\".pdf".into(),
            ],
            Path::new("output.pdf"),
            true,
            12,
        )
        .unwrap();
        assert_eq!(value["pages"][0]["range"], "1-9");
        assert_eq!(value["pages"][1]["file"], "replacement \"page\".pdf");
        assert_eq!(value["rotate"][0], "+90:10");
        assert_eq!(value["streamData"], "preserve");
        assert_eq!(value["objectStreams"], "preserve");
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn metadata_helpers_bound_stalls_and_reject_bad_output_without_touching_source() {
        use std::{
            os::unix::fs::PermissionsExt,
            time::{Duration, Instant},
        };
        let folder = tempfile::tempdir().unwrap();
        let source = folder.path().join("immutable.pdf");
        let original = b"%PDF generated metadata snapshot";
        std::fs::write(&source, original).unwrap();
        let helper = folder.path().join("metadata-helper");
        let run = |script: &str, timeout| {
            std::fs::write(&helper, script).unwrap();
            std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
            inspect_metadata_with(
                &source,
                &helper,
                crate::helper_process::Limits {
                    timeout,
                    stdout: 4096,
                    stderr: 0,
                },
            )
        };
        let start = Instant::now();
        let error = run("#!/bin/sh\nsleep 30\n", Duration::from_millis(150)).unwrap_err();
        assert!(format!("{error:#}").contains("deadline"));
        assert!(start.elapsed() < Duration::from_secs(3));
        for script in [
            "#!/bin/sh\nprintf not-json\n",
            "#!/bin/sh\nprintf '%s' '{\"pages\":[]}'\nprintf 'private helper detail' >&2\nexit 3\n",
            "#!/bin/sh\ncat /dev/zero\n",
        ] {
            let error = run(script, Duration::from_secs(3)).unwrap_err();
            assert!(!format!("{error:#}").contains("private helper detail"));
            assert_eq!(std::fs::read(&source).unwrap(), original);
        }
        let value = run(
            "#!/bin/sh\nprintf '%s' '{\"pages\":[]}'\n",
            Duration::from_secs(3),
        )
        .unwrap();
        assert_eq!(value["pages"], json!([]));
        assert_eq!(std::fs::read(&source).unwrap(), original);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn page_count_rejects_warning_flood_and_bad_counts_before_retry() {
        use std::os::unix::fs::PermissionsExt;
        let folder = tempfile::tempdir().unwrap();
        let source = folder.path().join("source.pdf");
        let original = b"%PDF generated page-count fixture";
        std::fs::write(&source, original).unwrap();
        let helper = folder.path().join("page-count-helper");
        for (body, expected) in [
            ("printf 7; exit 3", None),
            ("cat /dev/zero", None),
            ("printf invalid", None),
            ("printf 0", None),
            ("printf 100001", None),
            ("printf 7", Some(7)),
        ] {
            std::fs::write(&helper, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
            let result = page_count(&helper, &source);
            if let Some(count) = expected {
                assert_eq!(result.unwrap(), count);
            } else {
                assert!(result.is_err());
            }
            assert_eq!(std::fs::read(&source).unwrap(), original);
        }
    }

    #[test]
    fn assembly_job_uses_qpdf_11_rotation_syntax() {
        let snapshots: [PathBuf; 1] = ["original.pdf".into()];
        let mut plan = PagePlan::new(vec![4]).unwrap();
        let unrotated = job(&plan, &snapshots, Path::new("out.pdf"), true, 11).unwrap();
        assert!(unrotated.get("rotate").is_none());
        plan.rotate(&[1, 2], 1).unwrap();
        let single = job(&plan, &snapshots, Path::new("out.pdf"), true, 11).unwrap();
        assert_eq!(single["rotate"], "+90:2-3");
        plan.rotate(&[3], 2).unwrap();
        assert!(job(&plan, &snapshots, Path::new("out.pdf"), true, 11).is_err());
        let modern = job(&plan, &snapshots, Path::new("out.pdf"), true, 12).unwrap();
        assert_eq!(modern["rotate"].as_array().unwrap().len(), 2);
    }
    #[test]
    fn export_plan_supports_multiple_merge_sources_and_rejects_ambiguous_replacement() {
        let operation = PageOperation {
            kind: OperationKind::Merge,
            pages: String::new(),
            source_pages: String::new(),
        };
        assert_eq!(
            build_plan(vec![2, 3, 1], &operation).unwrap().pages().len(),
            6
        );
        let replacement = PageOperation {
            kind: OperationKind::Replace,
            pages: "1-2".into(),
            source_pages: "1".into(),
        };
        assert!(build_plan(vec![10, 1], &replacement).is_err());
    }
}
