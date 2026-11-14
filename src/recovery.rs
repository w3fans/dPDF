//! Bounded local crash recovery. One process owns the store; PDF sources are never rewritten.
use crate::{
    document::{DocumentId, PageSize},
    overlay::{Geometry, Placement},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    thread,
};

const MAGIC: &[u8; 8] = b"dPDFdr01";
const MAX_META: u64 = 262_144;
const MAX_PDF: u64 = 128 * 1024 * 1024;
const MAX_IMAGES: u64 = 64 * 1024 * 1024;
const MAX_STORE: u64 = 512 * 1024 * 1024;

#[derive(Clone)]
pub struct DraftMark {
    pub placement: Placement,
    pub default_size: (f32, f32),
    pub aspect_locked: bool,
}
#[derive(Clone)]
pub struct Draft {
    pub source: PathBuf,
    pub original: Arc<Vec<u8>>,
    pub marks: Vec<DraftMark>,
    pub selected: Option<usize>,
    pub review: Option<crate::review::Review>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedMark {
    page: usize,
    geometry: Geometry,
    opacity: f32,
    asset: usize,
    default_size: (f32, f32),
    aspect_locked: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    schema_version: u32,
    source: PathBuf,
    pdf_length: u64,
    assets: Vec<u64>,
    marks: Vec<SavedMark>,
    selected: Option<usize>,
    #[serde(default)]
    review: Option<crate::review::Review>,
}

fn valid_mark(mark: &SavedMark) -> bool {
    let g = mark.geometry;
    [
        g.x_mm,
        g.y_mm,
        g.width_mm,
        g.height_mm,
        mark.opacity,
        mark.default_size.0,
        mark.default_size.1,
    ]
    .iter()
    .all(|n| n.is_finite())
        && g.x_mm >= 0.0
        && g.y_mm >= 0.0
        && g.quarter_turns < 4
        && [
            g.width_mm,
            g.height_mm,
            mark.default_size.0,
            mark.default_size.1,
        ]
        .iter()
        .all(|n| (0.1..=300.0).contains(n))
        && (0.0..=1.0).contains(&mark.opacity)
}
fn validate(meta: &Metadata) -> Result<()> {
    if meta.schema_version != 1
        || meta.pdf_length == 0
        || meta.pdf_length > MAX_PDF
        || meta.source.as_os_str().is_empty()
        || (meta.marks.is_empty() && meta.review.is_none())
        || meta.marks.len() > 256
        || (meta.assets.is_empty() && !meta.marks.is_empty())
        || meta.assets.len() > 256
        || meta.assets.iter().any(|n| *n == 0 || *n > MAX_IMAGES)
        || meta.assets.iter().sum::<u64>() > MAX_IMAGES
        || meta.selected.is_some_and(|i| i >= meta.marks.len())
        || meta
            .marks
            .iter()
            .any(|m| m.asset >= meta.assets.len() || !valid_mark(m))
    {
        bail!("Unsupported or invalid draft recovery data");
    }
    if let Some(review) = &meta.review {
        review.validate(100_000)?;
    }
    Ok(())
}
fn read_header(file: &mut File) -> Result<Metadata> {
    let mut magic = [0; 8];
    file.read_exact(&mut magic)?;
    if &magic != MAGIC {
        bail!("Unrecognized recovery file");
    }
    let mut length = [0; 4];
    file.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length) as u64;
    if length == 0 || length > MAX_META {
        bail!("Recovery metadata exceeds its limit");
    }
    let mut bytes = vec![0; length as usize];
    file.read_exact(&mut bytes)?;
    let meta: Metadata = serde_json::from_slice(&bytes)?;
    validate(&meta)?;
    if file.metadata()?.len() != 12 + length + meta.pdf_length + meta.assets.iter().sum::<u64>() {
        bail!("Truncated recovery file or unexpected trailing data");
    }
    Ok(meta)
}
fn write_draft(draft: &Draft, path: &Path) -> Result<()> {
    let mut assets: Vec<Arc<Vec<u8>>> = vec![];
    let mut indices = HashMap::new();
    let marks = draft
        .marks
        .iter()
        .map(|mark| {
            let ptr = Arc::as_ptr(&mark.placement.image) as usize;
            let asset = *indices.entry(ptr).or_insert_with(|| {
                assets.push(mark.placement.image.clone());
                assets.len() - 1
            });
            SavedMark {
                page: mark.placement.page,
                geometry: mark.placement.geometry,
                opacity: mark.placement.opacity,
                asset,
                default_size: mark.default_size,
                aspect_locked: mark.aspect_locked,
            }
        })
        .collect();
    let meta = Metadata {
        schema_version: 1,
        source: draft.source.clone(),
        pdf_length: draft.original.len() as u64,
        assets: assets.iter().map(|a| a.len() as u64).collect(),
        marks,
        selected: draft.selected,
        review: draft.review.clone(),
    };
    validate(&meta)?;
    let bytes = serde_json::to_vec(&meta)?;
    if bytes.len() as u64 > MAX_META {
        bail!("Recovery metadata exceeds its limit");
    }
    let parent = path.parent().context("Recovery directory")?;
    let mut total = 12 + bytes.len() as u64 + meta.pdf_length + meta.assets.iter().sum::<u64>();
    for entry in std::fs::read_dir(parent)? {
        let entry = entry?;
        if entry.path() != path
            && entry.file_type()?.is_file()
            && entry.path().extension().is_some_and(|e| e == "dpdf")
        {
            total = total.saturating_add(entry.metadata()?.len());
        }
    }
    if total > MAX_STORE {
        bail!("Local recovery storage exceeds 512 MiB; recover or discard older drafts");
    }
    let mut file = tempfile::Builder::new()
        .prefix(".checkpoint-")
        .tempfile_in(parent)?;
    file.write_all(MAGIC)?;
    file.write_all(&(bytes.len() as u32).to_le_bytes())?;
    file.write_all(&bytes)?;
    file.write_all(&draft.original)?;
    for asset in assets {
        file.write_all(&asset)?;
    }
    file.as_file().sync_all()?;
    file.persist(path)
        .map_err(|e| e.error)
        .context("Publish local recovery checkpoint")?;
    Ok(())
}

pub struct RestoredMark {
    pub mark: DraftMark,
    pub preview: Arc<image::RgbaImage>,
}
pub struct Restored {
    pub review: Option<crate::review::Review>,
    pub source: PathBuf,
    pub original: Arc<Vec<u8>>,
    pub marks: Vec<RestoredMark>,
    pub selected: Option<usize>,
}
impl Restored {
    pub fn validate_pages(&self, pages: &[PageSize]) -> Result<()> {
        if let Some(review) = &self.review {
            review.validate(pages.len())?;
        }
        for mark in &self.marks {
            let size = pages
                .get(mark.mark.placement.page)
                .context("Recovered mark references a missing page")?;
            mark.mark
                .placement
                .geometry
                .normalized(size.width, size.height)?;
        }
        Ok(())
    }
}
fn load_draft(path: &Path) -> Result<Restored> {
    let mut file = File::open(path)?;
    let meta = read_header(&mut file)?;
    let mut original = vec![0; meta.pdf_length as usize];
    file.read_exact(&mut original)?;
    let mut assets = vec![];
    let mut preview_bytes = 0;
    for length in &meta.assets {
        let mut bytes = vec![0; *length as usize];
        file.read_exact(&mut bytes)?;
        let (image, _, format) = crate::imaging::decode(&bytes)?;
        if format != image::ImageFormat::Png {
            bail!("Recovered marks must contain cleaned PNG images");
        }
        let preview = image.thumbnail(400, 400).to_rgba8();
        preview_bytes += u64::from(preview.width()) * u64::from(preview.height()) * 16;
        if preview_bytes > MAX_IMAGES {
            bail!("Recovered preview images exceed 64 MiB");
        }
        assets.push((Arc::new(bytes), Arc::new(preview)));
    }
    let marks = meta
        .marks
        .into_iter()
        .map(|m| RestoredMark {
            mark: DraftMark {
                placement: Placement {
                    page: m.page,
                    geometry: m.geometry,
                    opacity: m.opacity,
                    image: assets[m.asset].0.clone(),
                },
                default_size: m.default_size,
                aspect_locked: m.aspect_locked,
            },
            preview: assets[m.asset].1.clone(),
        })
        .collect();
    Ok(Restored {
        review: meta.review,
        source: meta.source,
        original: Arc::new(original),
        marks,
        selected: meta.selected,
    })
}

#[derive(Clone)]
pub struct Candidate {
    pub path: PathBuf,
    pub source: PathBuf,
}
pub enum Request {
    Save {
        id: DocumentId,
        generation: u64,
        draft: Draft,
        supersedes: Option<PathBuf>,
    },
    Remove {
        id: DocumentId,
        supersedes: Option<PathBuf>,
    },
    Load(PathBuf),
    Discard(PathBuf),
    /// Delete every local recovery draft, including other sessions' drafts.
    ClearAll,
    Finish,
}
pub enum Reply {
    Available(Vec<Candidate>),
    Saved(DocumentId, u64, Option<PathBuf>),
    Removed(DocumentId, Option<PathBuf>),
    Loaded(PathBuf, Restored),
    Discarded(PathBuf),
    Cleared(usize),
    Failed(String),
    Unavailable(String),
}
struct StoreLock(File);
impl Drop for StoreLock {
    fn drop(&mut self) {
        // Closing only this handle can leave an inherited/duplicated open-file
        // description holding the lock after Store ownership has ended.
        let _ = self.0.unlock();
    }
}
struct Store {
    root: PathBuf,
    nonce: tempfile::TempDir,
    owned: HashMap<DocumentId, PathBuf>,
    candidates: HashSet<PathBuf>,
    retired: HashSet<PathBuf>,
    // Fields drop in declaration order: finish nonce/data cleanup before unlock.
    _lock: StoreLock,
}
impl Store {
    fn open(root: &Path) -> Result<(Self, Vec<Candidate>)> {
        std::fs::create_dir_all(root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))?;
        }
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("recovery.lock"))?;
        lock.try_lock().map_err(|e| {
            anyhow::anyhow!("Recovery is unavailable while another dPDF instance owns it: {e}")
        })?;
        // Own successful acquisition immediately, including initialization errors.
        let lock = StoreLock(lock);
        // Holding the lock, empty nonce folders can only be left by interrupted instances.
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && entry.file_name().to_string_lossy().starts_with(".session-")
            {
                let _ = std::fs::remove_dir(entry.path());
            }
        }
        let nonce = tempfile::Builder::new()
            .prefix(".session-")
            .tempdir_in(root)?;
        let mut candidates = vec![];
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".checkpoint-")
            {
                remove_if_present(&entry.path())?;
                continue;
            }
            if !entry.file_type()?.is_file() || entry.path().extension().is_none_or(|e| e != "dpdf")
            {
                continue;
            }
            if candidates.len() >= 256 {
                bail!("Too many local recovery drafts; existing files were preserved");
            }
            // Corrupt/future files are retained. They must never prevent valid drafts being offered.
            if let Ok(meta) = File::open(entry.path())
                .and_then(|mut f| read_header(&mut f).map_err(std::io::Error::other))
            {
                candidates.push(Candidate {
                    path: entry.path(),
                    source: meta.source,
                });
            }
        }
        candidates.sort_by_key(|c| {
            std::cmp::Reverse(std::fs::metadata(&c.path).and_then(|m| m.modified()).ok())
        });
        let known = candidates.iter().map(|c| c.path.clone()).collect();
        Ok((
            Self {
                root: root.into(),
                nonce,
                owned: HashMap::new(),
                candidates: known,
                retired: HashSet::new(),
                _lock: lock,
            },
            candidates,
        ))
    }
    fn save(&mut self, id: DocumentId, draft: &Draft, supersedes: Option<&Path>) -> Result<()> {
        let path = self
            .owned
            .entry(id)
            .or_insert_with(|| {
                self.root.join(format!(
                    "draft-{}-{id}.dpdf",
                    self.nonce
                        .path()
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .trim_start_matches('.')
                ))
            })
            .clone();
        write_draft(draft, &path)?;
        if let Some(old) = supersedes {
            self.discard(old)?;
        }
        Ok(())
    }
    fn remove(&mut self, id: DocumentId) -> Result<()> {
        if let Some(path) = self.owned.get(&id) {
            remove_if_present(path)?;
            self.owned.remove(&id);
        }
        Ok(())
    }
    /// Remove every draft and interrupted checkpoint in the store; returns drafts removed.
    fn clear_all(&mut self) -> Result<usize> {
        let mut removed = 0;
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let draft = entry.path().extension().is_some_and(|e| e == "dpdf");
            if draft || name.starts_with(".checkpoint-") {
                remove_if_present(&entry.path())?;
                removed += usize::from(draft);
            }
        }
        self.retired.extend(self.candidates.drain());
        self.retired
            .extend(self.owned.drain().map(|(_, path)| path));
        Ok(removed)
    }
    fn discard(&mut self, path: &Path) -> Result<()> {
        if self.retired.contains(path) {
            return Ok(());
        }
        if !self.candidates.contains(path) {
            bail!("Recovery file is not owned by this session");
        }
        remove_if_present(path)?;
        self.candidates.remove(path);
        self.retired.insert(path.to_path_buf());
        Ok(())
    }
}
fn remove_if_present(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub struct Worker {
    pub requests: mpsc::SyncSender<Request>,
    pub replies: mpsc::Receiver<Reply>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(root: PathBuf) -> Self {
        let (sender, requests) = mpsc::sync_channel(4);
        let (replies, receiver) = mpsc::channel();
        let thread = thread::spawn(move || {
            let (mut store, candidates) = match Store::open(&root) {
                Ok(v) => v,
                Err(e) => {
                    let _ = replies.send(Reply::Unavailable(format!(
                        "Local draft recovery unavailable: {e:#}"
                    )));
                    return;
                }
            };
            let _ = replies.send(Reply::Available(candidates));
            while let Ok(request) = requests.recv() {
                if matches!(request, Request::Finish) {
                    break;
                }
                let result = (|| -> Result<Reply> {
                    Ok(match request {
                        Request::Save {
                            id,
                            generation,
                            draft,
                            supersedes,
                        } => {
                            store.save(id, &draft, supersedes.as_deref())?;
                            Reply::Saved(id, generation, supersedes)
                        }
                        Request::Remove { id, supersedes } => {
                            store.remove(id)?;
                            if let Some(path) = &supersedes {
                                store.discard(path)?;
                            }
                            Reply::Removed(id, supersedes)
                        }
                        Request::Load(path) => {
                            if !store.candidates.contains(&path) {
                                bail!("Unknown recovery file");
                            }
                            let draft = load_draft(&path)?;
                            Reply::Loaded(path, draft)
                        }
                        Request::Discard(path) => {
                            store.discard(&path)?;
                            Reply::Discarded(path)
                        }
                        Request::ClearAll => Reply::Cleared(store.clear_all()?),
                        Request::Finish => unreachable!(),
                    })
                })();
                match result {
                    Ok(reply) => {
                        let _ = replies.send(reply);
                    }
                    Err(e) => {
                        let _ =
                            replies.send(Reply::Failed(format!("Local recovery failed: {e:#}")));
                    }
                }
            }
        });
        Self {
            requests: sender,
            replies: receiver,
            thread: Some(thread),
        }
    }
    pub fn finish(&mut self) {
        let _ = self.requests.send(Request::Finish);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.finish();
    }
}

/// Delete all drafts while recovery is off in this instance. Refuses while another
/// dPDF instance owns the store; a missing store has nothing to delete.
pub fn clear_store(root: &Path) -> Result<usize> {
    if !root.is_dir() {
        return Ok(0);
    }
    let (mut store, _) = Store::open(root)?;
    store.clear_all()
}

pub fn default_root() -> Option<PathBuf> {
    crate::marks::default_root().and_then(|p| p.parent().map(|p| p.join("recovery")))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Draft {
        let rgba = image::RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 120]));
        Draft {
            review: None,
            source: PathBuf::from("contract.pdf"),
            original: Arc::new(b"%PDF original snapshot".to_vec()),
            marks: vec![DraftMark {
                placement: Placement {
                    page: 0,
                    geometry: Geometry {
                        x_mm: 10.0,
                        y_mm: 20.0,
                        width_mm: 45.0,
                        height_mm: 30.0,
                        quarter_turns: 0,
                    },
                    opacity: 0.7,
                    image: Arc::new(crate::marks::png_ref(&rgba).unwrap()),
                },
                default_size: (45.0, 30.0),
                aspect_locked: true,
            }],
            selected: Some(0),
        }
    }
    #[test]
    fn remarks_only_recovery_needs_no_signature_assets() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("notes.dpdf");
        let mut draft = sample();
        draft.marks.clear();
        draft.selected = None;
        let mut review = crate::review::Review::default();
        review.new_revision("Second pass").unwrap();
        review
            .add(
                0,
                crate::document::NormalizedRect {
                    x: 0.1,
                    y: 0.1,
                    width: 0.2,
                    height: 0.04,
                },
                crate::review::Kind::Note,
                "Review-only draft",
                1,
            )
            .unwrap();
        draft.review = Some(review.clone());
        write_draft(&draft, &path).unwrap();
        let recovered = load_draft(&path).unwrap();
        assert!(recovered.marks.is_empty());
        assert_eq!(recovered.review, Some(review));
        recovered
            .validate_pages(&[PageSize {
                width: 612.0,
                height: 792.0,
            }])
            .unwrap();
    }
    #[test]
    fn full_store_preserves_last_checkpoint_and_orphan_temporary_files_are_retired() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("draft.dpdf");
        write_draft(&sample(), &path).unwrap();
        let old = std::fs::read(&path).unwrap();
        let large = File::create(dir.path().join("full.dpdf")).unwrap();
        large.set_len(MAX_STORE).unwrap();
        assert!(write_draft(&sample(), &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), old);
        drop(large);
        std::fs::remove_file(dir.path().join("full.dpdf")).unwrap();
        let orphan = dir.path().join(".checkpoint-interrupted");
        std::fs::write(&orphan, b"partial write").unwrap();
        let (_store, candidates) = Store::open(dir.path()).unwrap();
        assert!(!orphan.exists());
        assert_eq!(candidates.len(), 1);
    }
    #[test]
    fn atomic_roundtrip_preserves_snapshot_and_shares_repeated_assets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("draft.dpdf");
        let mut draft = sample();
        draft.marks.push(draft.marks[0].clone());
        write_draft(&draft, &path).unwrap();
        let restored = load_draft(&path).unwrap();
        assert_eq!(restored.original, draft.original);
        assert_eq!(restored.marks.len(), 2);
        assert_eq!(restored.selected, Some(0));
        assert_eq!(
            restored.marks[0].mark.placement.geometry,
            draft.marks[0].placement.geometry
        );
        assert!(Arc::ptr_eq(
            &restored.marks[0].mark.placement.image,
            &restored.marks[1].mark.placement.image
        ));
        assert_eq!(
            restored.marks[0].preview.get_pixel(0, 0).0,
            [10, 20, 30, 120]
        );
        restored
            .validate_pages(&[PageSize {
                width: 612.0,
                height: 792.0,
            }])
            .unwrap();
        assert!(restored.validate_pages(&[]).is_err());
        let old = std::fs::read(&path).unwrap();
        draft.marks[0].placement.opacity = f32::NAN;
        assert!(write_draft(&draft, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), old);
        let mut truncated = old.clone();
        truncated.pop();
        std::fs::write(&path, &truncated).unwrap();
        assert!(load_draft(&path).is_err());
    }
    #[test]
    fn store_lock_recovery_ownership_and_successful_handoff() {
        let dir = tempfile::tempdir().unwrap();
        let draft = sample();
        let (mut store, candidates) = Store::open(dir.path()).unwrap();
        assert!(candidates.is_empty());
        assert!(Store::open(dir.path()).is_err());
        store.save(1, &draft, None).unwrap();
        let old = store.owned[&1].clone();
        assert!(store.discard(&old).is_err());
        drop(store);
        let (mut store, candidates) = Store::open(dir.path()).unwrap();
        assert_eq!(candidates.len(), 1);
        let mut invalid = draft.clone();
        invalid.selected = Some(99);
        assert!(store.save(2, &invalid, Some(&old)).is_err());
        assert!(old.exists());
        store.save(2, &draft, Some(&old)).unwrap();
        assert!(!old.exists());
        assert!(store.owned[&2].exists());
        store.remove(2).unwrap();
        assert!(!store.owned.contains_key(&2));
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn store_handoff_releases_lingering_duplicate_handle() {
        let dir = tempfile::tempdir().unwrap();
        let (mut store, _) = Store::open(dir.path()).unwrap();
        store.save(1, &sample(), None).unwrap();
        let checkpoint = store.owned[&1].clone();
        let original = std::fs::read(&checkpoint).unwrap();
        // A fork before exec can retain the same open-file description, even
        // with close-on-exec set. A duplicate models that lifetime without a
        // timing race or running fork in this multithreaded test harness.
        let lingering = store._lock.0.try_clone().unwrap();
        assert!(Store::open(dir.path()).is_err());
        drop(store);
        let (next, candidates) = Store::open(dir.path())
            .expect("Store destruction must release ownership while a duplicate handle remains");
        assert_eq!(candidates.len(), 1);
        assert_eq!(std::fs::read(&checkpoint).unwrap(), original);
        assert!(Store::open(dir.path()).is_err());
        drop(lingering);
        // Closing the old duplicate must not release the new owner's lock.
        assert!(Store::open(dir.path()).is_err());
        drop(next);
        assert!(Store::open(dir.path()).is_ok());
    }
    #[test]
    fn clearing_removes_every_draft_but_respects_the_owning_instance() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(clear_store(&dir.path().join("missing")).unwrap(), 0);
        assert!(!dir.path().join("missing").exists());
        write_draft(&sample(), &dir.path().join("earlier session.dpdf")).unwrap();
        std::fs::write(dir.path().join("corrupt.dpdf"), b"corrupt").unwrap();
        std::fs::write(dir.path().join("unrelated.txt"), b"kept").unwrap();
        std::fs::create_dir(dir.path().join(".session-crashed")).unwrap();
        std::fs::create_dir_all(dir.path().join(".session-unexpected/content")).unwrap();
        let (mut store, candidates) = Store::open(dir.path()).unwrap();
        assert!(!dir.path().join(".session-crashed").exists());
        assert!(dir.path().join(".session-unexpected/content").exists());
        assert_eq!(candidates.len(), 1);
        store.save(7, &sample(), None).unwrap();
        // Another instance cannot clear a store this instance owns.
        assert!(clear_store(dir.path()).is_err());
        assert_eq!(store.clear_all().unwrap(), 3);
        assert!(store.owned.is_empty() && store.candidates.is_empty());
        assert_eq!(
            std::fs::read(dir.path().join("unrelated.txt")).unwrap(),
            b"kept"
        );
        // A later save after clearing writes a fresh checkpoint.
        store.save(7, &sample(), None).unwrap();
        drop(store);
        assert_eq!(clear_store(dir.path()).unwrap(), 1);
        let (_store, candidates) = Store::open(dir.path()).unwrap();
        assert!(candidates.is_empty());
    }
    #[test]
    fn bad_headers_and_future_data_are_preserved_without_hiding_valid_drafts() {
        let dir = tempfile::tempdir().unwrap();
        write_draft(&sample(), &dir.path().join("valid.dpdf")).unwrap();
        let bad = dir.path().join("bad.dpdf");
        std::fs::write(&bad, b"future or corrupt bytes").unwrap();
        let (_store, candidates) = Store::open(dir.path()).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(std::fs::read(bad).unwrap(), b"future or corrupt bytes");
        let mut header = MAGIC.to_vec();
        header.extend_from_slice(&(MAX_META as u32 + 1).to_le_bytes());
        let oversized = dir.path().join("oversized.dpdf");
        std::fs::write(&oversized, header).unwrap();
        assert!(load_draft(&oversized).is_err());
    }
}
