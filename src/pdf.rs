//! PDFium lives on one worker thread. No PDFium handles or borrowed objects enter the UI.
use crate::{cache::ByteCache, document::*};
use anyhow::{Context, Result, bail, ensure};
use pdfium_render::prelude::*;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::Duration,
};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct RenderKey {
    pub document: DocumentId,
    pub page: usize,
    pub width: i32,
    pub rotation: u8,
    pub thumbnail: bool,
}

#[derive(Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub enum Command {
    Open(DocumentId, PathBuf),
    OpenSnapshot(DocumentId, PathBuf, Arc<Vec<u8>>),
    Snapshot(DocumentId),
    Unlock(DocumentId, String),
    LoadReview(DocumentId),
    PrintReviewed {
        id: DocumentId,
        review: crate::review::Review,
        current: usize,
        range: String,
        printer: String,
        fit: bool,
        native: bool,
    },
    ExportWorkspace {
        id: DocumentId,
        placements: Vec<crate::overlay::Placement>,
        review: crate::review::Review,
        mode: crate::review::ExportMode,
        destination: PathBuf,
    },
    Close(DocumentId),
    Render {
        generation: u64,
        key: RenderKey,
    },
    Search {
        id: DocumentId,
        token: u64,
        query: String,
    },
    SelectText {
        id: DocumentId,
        page: usize,
        token: u64,
        bounds: NormalizedRect,
    },
    SaveCopy(DocumentId, PathBuf),
    ExportPages {
        id: DocumentId,
        operation: crate::assembly::PageOperation,
        additional: Vec<PathBuf>,
        destination: PathBuf,
    },
    ExportMarks {
        id: DocumentId,
        placements: Vec<crate::overlay::Placement>,
        destination: PathBuf,
    },
    Print {
        id: DocumentId,
        range: String,
        printer: String,
        fit: bool,
    },
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    PrintNative {
        id: DocumentId,
        current: usize,
        range: String,
        printer: String,
        fit: bool,
    },
}

pub enum Event {
    Ready,
    Opened(DocumentInfo),
    Snapshot(DocumentId, Arc<Vec<u8>>),
    OpenFailed(DocumentId, String),
    PasswordRequired(DocumentId, PathBuf, bool),
    ReviewLoaded(DocumentId, Result<Option<crate::review::Review>, String>),
    PageUnitsLoaded(DocumentId, Result<Vec<f64>, String>),
    Rendered {
        generation: u64,
        key: RenderKey,
        frame: Arc<Frame>,
    },
    RenderFailed {
        generation: u64,
        key: RenderKey,
        message: String,
    },
    SearchResults {
        id: DocumentId,
        token: u64,
        hits: Vec<SearchHit>,
        truncated: bool,
    },
    SelectedText {
        id: DocumentId,
        token: u64,
        text: Result<String, String>,
    },
    ExportFinished(Result<crate::assembly::ExportResult, String>),
    PrintFinished(Result<String, String>),
    Notice(String),
    Unavailable(String),
}

pub struct Worker {
    pub commands: SyncSender<Command>,
    pub events: Receiver<Event>,
    pub generation: Arc<AtomicU64>,
    pub search_token: Arc<AtomicU64>,
}

pub fn bind_pdfium() -> Result<Pdfium> {
    Ok(Pdfium::new(bind_pdfium_library()?))
}

pub(crate) fn bind_pdfium_library() -> Result<Box<dyn PdfiumLibraryBindings>> {
    if let Some(path) = std::env::var_os("DPDF_PDFIUM_LIBRARY") {
        let path = PathBuf::from(path);
        ensure!(
            path.is_absolute(),
            "DPDF_PDFIUM_LIBRARY must be an absolute path"
        );
        return Pdfium::bind_to_library(path).context("Load DPDF_PDFIUM_LIBRARY");
    }
    // Never search the working directory: a document's folder is not a trusted library location.
    if let Some(directory) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        let path = Pdfium::pdfium_platform_library_name_at_path(&directory);
        if path.is_file() {
            return Pdfium::bind_to_library(path).context("Load bundled PDFium");
        }
    }
    #[cfg(target_os = "windows")]
    {
        // Windows' bare-name DLL search includes the working directory and PATH.
        // Missing deployment files must not load an engine from a PDF's folder.
        bail!(
            "PDFium is unavailable. On Windows, place pdfium.dll beside dPDF or set DPDF_PDFIUM_LIBRARY to an absolute library path. See docs/BUILDING.md."
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        Pdfium::bind_to_system_library().context("PDFium is unavailable. Set DPDF_PDFIUM_LIBRARY to a PDFium 7881 library, or place the library next to dPDF. See docs/BUILDING.md.")
    }
}

fn render(document: &PdfDocument<'_>, key: &RenderKey) -> Result<Frame> {
    let rotation = match key.rotation % 4 {
        1 => PdfPageRenderRotation::Degrees90,
        2 => PdfPageRenderRotation::Degrees180,
        3 => PdfPageRenderRotation::Degrees270,
        _ => PdfPageRenderRotation::None,
    };
    let page = document.pages().get(key.page as i32)?;
    let bitmap = page.render_with_config(
        &PdfRenderConfig::new()
            .set_target_width(key.width.clamp(32, 3072))
            .set_maximum_height(4096)
            .rotate(rotation, true),
    )?;
    Ok(Frame {
        width: bitmap.width() as u32,
        height: bitmap.height() as u32,
        rgba: bitmap.as_rgba_bytes(),
    })
}

fn find_matches(
    page: &PdfPage<'_>,
    index: usize,
    query: &str,
    limit: usize,
    cancelled: impl Fn() -> bool,
) -> Result<(Vec<SearchHit>, bool)> {
    let text = page.text()?;
    let search = text.search(query, &PdfSearchOptions::new().match_case(false))?;
    // Native PageToDevice handles crop-box offsets and inherited PDF rotation.
    // A square coordinate surface gives independent normalized x/y values without rasterization.
    let config = PdfRenderConfig::new().set_fixed_size(10_000, 10_000);
    let mut hits = Vec::new();
    while let Some(segments) = search.find_next() {
        if cancelled() {
            break;
        }
        if hits.len() == limit {
            return Ok((hits, true));
        }
        let mut bounds = Vec::new();
        for segment in segments.iter() {
            let rect = segment.bounds();
            let mut points = Vec::with_capacity(4);
            for (x, y) in [
                (rect.left(), rect.top()),
                (rect.right(), rect.top()),
                (rect.left(), rect.bottom()),
                (rect.right(), rect.bottom()),
            ] {
                points.push(page.points_to_pixels(x, y, &config)?);
            }
            let left =
                (points.iter().map(|p| p.0).min().unwrap_or(0) as f32 / 10_000.0).clamp(0.0, 1.0);
            let top =
                (points.iter().map(|p| p.1).min().unwrap_or(0) as f32 / 10_000.0).clamp(0.0, 1.0);
            let right =
                (points.iter().map(|p| p.0).max().unwrap_or(0) as f32 / 10_000.0).clamp(0.0, 1.0);
            let bottom =
                (points.iter().map(|p| p.1).max().unwrap_or(0) as f32 / 10_000.0).clamp(0.0, 1.0);
            if right > left && bottom > top {
                bounds.push(NormalizedRect {
                    x: left,
                    y: top,
                    width: right - left,
                    height: bottom - top,
                });
            }
        }
        hits.push(SearchHit {
            page: index,
            bounds,
        });
    }
    Ok((hits, false))
}

fn selected_text(page: &PdfPage<'_>, bounds: NormalizedRect) -> Result<String> {
    if ![bounds.x, bounds.y, bounds.width, bounds.height]
        .iter()
        .all(|x| x.is_finite())
        || bounds.x < -0.00001
        || bounds.y < -0.00001
        || bounds.width <= 0.0
        || bounds.height <= 0.0
        || bounds.x + bounds.width > 1.00001
        || bounds.y + bounds.height > 1.00001
    {
        bail!("Text selection must be a nonempty rectangle inside the page");
    }
    // The selection is normalized to the native rendered page, before view rotation.
    // DeviceToPage accounts for crop offsets and intrinsic /Rotate.
    let config = PdfRenderConfig::new().set_fixed_size(10_000, 10_000);
    let (left, top) = page.pixels_to_points(
        (bounds.x * 10_000.0).round() as i32,
        (bounds.y * 10_000.0).round() as i32,
        &config,
    )?;
    let (right, bottom) = page.pixels_to_points(
        ((bounds.x + bounds.width) * 10_000.0).round() as i32,
        ((bounds.y + bounds.height) * 10_000.0).round() as i32,
        &config,
    )?;
    Ok(page
        .text()?
        .inside_rect(PdfRect::new(bottom, left, top, right)))
}

// The PDFium reader owns an Arc to the same immutable opened snapshot used by
// Save As/recovery. A reader avoids a second whole-file Vec without borrowing
// from OpenDocument or returning to a mutable source file on disk.
struct SharedPdfBytes(Arc<Vec<u8>>);
impl AsRef<[u8]> for SharedPdfBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}
fn load_shared_pdf<'a>(
    pdfium: &'a Pdfium,
    bytes: Arc<Vec<u8>>,
    password: Option<&str>,
) -> std::result::Result<PdfDocument<'a>, PdfiumError> {
    pdfium.load_pdf_from_reader(std::io::Cursor::new(SharedPdfBytes(bytes)), password)
}

struct OpenDocument<'a> {
    pdf: PdfDocument<'a>,
    // The reader saves the exact opened bytes, even if the file on disk changes later.
    original: Arc<Vec<u8>>,
    path: PathBuf,
    password: Option<String>,
    unlocked: std::cell::OnceCell<Arc<Vec<u8>>>,
}
impl OpenDocument<'_> {
    fn working_bytes(&self) -> Result<Arc<Vec<u8>>> {
        let Some(password) = &self.password else {
            return Ok(self.original.clone());
        };
        if let Some(bytes) = self.unlocked.get() {
            return Ok(bytes.clone());
        }
        let bytes = crate::encrypted::decrypt(&self.original, password)?;
        let _ = self.unlocked.set(bytes.clone());
        Ok(bytes)
    }
}
struct PendingDocument {
    recovery: bool,
    path: PathBuf,
    original: Arc<Vec<u8>>,
}
fn describe(
    id: DocumentId,
    path: PathBuf,
    pdf: &PdfDocument<'_>,
    password_protected: bool,
) -> Result<DocumentInfo> {
    if pdf.pages().is_empty() {
        bail!("The PDF contains no pages");
    }
    Ok(DocumentInfo {
        id,
        path,
        password_protected,
        physical_pages: None,
        // Dictionary-only geometry avoids loading every page's content/resources
        // before the first frame. Propagate errors rather than silently truncating
        // the document at an unreadable page through PdfPagesIterator::next().
        pages: pdf
            .pages()
            .page_sizes()?
            .iter()
            .map(|rect| PageSize {
                width: rect.width().value,
                height: rect.height().value,
            })
            .collect(),
        bookmarks: pdf
            .bookmarks()
            .iter()
            .filter_map(|b| {
                Some(Bookmark {
                    title: b.title()?,
                    page: b.destination()?.page_index().ok()? as usize,
                })
            })
            .collect(),
        signatures: pdf.signatures().len() as usize,
    })
}
fn describe_open(
    id: DocumentId,
    document: &OpenDocument<'_>,
    recovery: bool,
) -> Result<DocumentInfo> {
    let mut info = describe(
        id,
        document.path.clone(),
        &document.pdf,
        document.password.is_some(),
    )?;
    if recovery {
        let bytes = document.working_bytes()?;
        let units = crate::pdf_units::load(bytes.as_slice())?;
        info.physical_pages = Some(
            physical_page_sizes(&info.pages, &units)
                .context("Cannot verify recovered physical page dimensions")?,
        );
    }
    Ok(info)
}

fn password_error(error: &PdfiumError) -> bool {
    matches!(
        error,
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError)
    )
}

fn open_pdf_error(error: PdfiumError) -> anyhow::Error {
    // Keep engine details in diagnostics rather than the single-line status bar.
    eprintln!("PDF open failed: {error}");
    let message = match error {
        PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::FormatError) => {
            "Cannot open PDF: damaged or invalid file. Try another copy."
        }
        _ => "Cannot read this PDF. Try another copy or a different PDF.",
    };
    anyhow::anyhow!(message)
}

struct SearchJob {
    id: DocumentId,
    token: u64,
    query: String,
    page: usize,
    hits: Vec<SearchHit>,
    truncated: bool,
}

pub fn start() -> Worker {
    let (commands, requests) = mpsc::sync_channel(64);
    let (responses, events) = mpsc::sync_channel(8);
    let generation = Arc::new(AtomicU64::new(0));
    let search_token = Arc::new(AtomicU64::new(0));
    let active = generation.clone();
    let search_active = search_token.clone();
    thread::Builder::new().name("dpdf-pdf".into()).spawn(move || {
        let pdfium = match bind_pdfium() {
            Ok(pdfium) => pdfium,
            Err(error) => { let _ = responses.send(Event::Unavailable(format!("{error:#}"))); return; }
        };
        let _ = responses.send(Event::Ready);
        let mut documents: HashMap<DocumentId, OpenDocument<'_>> = HashMap::new();
        let mut passwords:HashMap<DocumentId,PendingDocument>=HashMap::new();
        let mut cache = ByteCache::new(64 * 1024 * 1024);
        let mut search: Option<SearchJob> = None;
        loop {
            let command = if search.is_some() { requests.try_recv().ok() } else {
                match requests.recv() { Ok(command) => Some(command), Err(_) => break }
            };
            match command {
                Some(request @ (Command::Open(..) | Command::OpenSnapshot(..))) => {
                    let (id,path,snapshot)=match request {
                        Command::Open(id,path)=>(id,path,None),
                        Command::OpenSnapshot(id,path,bytes)=>(id,path,Some(bytes)),
                        _=>unreachable!(),
                    };
                    let recovery=snapshot.is_some();
                    let result = (|| -> Result<Event> {
                        let original=match snapshot {Some(bytes)=>bytes,None=>Arc::new(std::fs::read(&path).with_context(||format!("Read {}",path.display()))?)};
                        match load_shared_pdf(&pdfium,original.clone(),None) {
                            Ok(pdf)=>{
                                let document=OpenDocument{pdf,original,path,password:None,unlocked:Default::default()};
                                let info=describe_open(id,&document,recovery)?;
                                documents.insert(id,document);
                                Ok(Event::Opened(info))
                            }
                            Err(error) if password_error(&error)=>{
                                if passwords.len()>=16 || original.len().saturating_add(passwords.values().map(|p|p.original.len()).sum::<usize>())>256*1024*1024 {bail!("Pending protected PDFs exceed 16 files / 256 MiB; finish or cancel their prompts first");}
                                passwords.insert(id,PendingDocument{recovery,path:path.clone(),original});
                                Ok(Event::PasswordRequired(id,path,false))
                            }
                            Err(error)=>Err(open_pdf_error(error)),
                        }
                    })();
                    let event=result.unwrap_or_else(|error|Event::OpenFailed(id,format!("{error:#}")));
                    if responses.send(event).is_err(){break;}
                }
                Some(Command::Unlock(id,password))=>{
                    let Some(pending)=passwords.remove(&id) else{continue;};
                    let path=pending.path.clone();
                    let result=(||->Result<Event>{
                        crate::encrypted::validate_password(&password)?;
                        match load_shared_pdf(&pdfium,pending.original.clone(),Some(&password)) {
                            Ok(pdf)=>{
                                let document=OpenDocument{pdf,original:pending.original,path,password:Some(password),unlocked:Default::default()};
                                let info=describe_open(id,&document,pending.recovery)?;
                                documents.insert(id,document);
                                Ok(Event::Opened(info))
                            }
                            Err(error) if password_error(&error)=>{
                                passwords.insert(id,pending);Ok(Event::PasswordRequired(id,path,true))
                            }
                            Err(error)=>Err(open_pdf_error(error)),
                        }
                    })();
                    let event=result.unwrap_or_else(|error|Event::OpenFailed(id,format!("{error:#}")));
                    if responses.send(event).is_err(){break;}
                }
                Some(Command::PrintReviewed{id,review,current,range,printer,fit,native})=>{
                    let prepared=(||->Result<_>{let document=documents.get(&id).context("Document was closed")?;let source=crate::assembly::SourceSnapshot{path:document.path.clone(),bytes:document.working_bytes()?};let path=source.path.clone();Ok((crate::review::prepare(source,&document.pdf,&review,crate::review::ExportMode::Current)?,path))})();
                    match prepared {
                        Ok((prepared,path))=>{let reply=responses.clone();thread::spawn(move || {
                            let result=(||->Result<String>{
                                let folder=tempfile::tempdir()?;let snapshot=folder.path().join("review-print.pdf");prepared.save(&snapshot)?;
                                if native {
                                    #[cfg(any(target_os="linux",target_os="windows"))]
                                    {return crate::platform::native_print(&std::fs::read(snapshot)?,&path,current,&range,&printer,fit);}
                                    #[cfg(not(any(target_os="linux",target_os="windows")))]
                                    {let _=(path,current);bail!("Native printing is unavailable on this platform");}
                                }
                                let _=(path,current);
                                crate::platform::print_pdf(&snapshot,&range,&printer,fit)
                            })().map_err(|e|format!("{e:#}"));let _=reply.send(Event::PrintFinished(result));
                        });},
                        Err(e)=>{let _=responses.send(Event::PrintFinished(Err(format!("{e:#}"))));}
                    }
                }
                Some(Command::LoadReview(id))=>{
                    let mut units = None;
                    let result=(||->Result<Option<crate::review::Review>> {
                        let document=documents.get_mut(&id).context("Document was closed")?;
                        let source=crate::assembly::SourceSnapshot{path:document.path.clone(),bytes:document.working_bytes()?};
                        let metadata=crate::assembly::inspect_snapshot(source.bytes.as_slice())?;
                        units=Some(crate::pdf_units::from_metadata(&metadata,document.pdf.pages().len() as usize).map_err(|e|format!("{e:#}")));
                        let model=crate::review::from_metadata(&metadata,&document.pdf)?;
                        if let Some(model)=&model {
                            let folder=tempfile::tempdir()?;let path=folder.path().join("reading-base.pdf");
                            crate::review::prepare(source,&document.pdf,model,crate::review::ExportMode::Clean)?.save(&path)?;
                            document.pdf=pdfium.load_pdf_from_byte_vec(std::fs::read(path)?,None)?;
                            cache.retain(|key:&RenderKey|key.document!=id);
                        }
                        Ok(model)
                    })().map_err(|e|format!("{e:#}"));
                    let units=units.unwrap_or_else(||Err(result.as_ref().err().cloned().unwrap_or_else(||"Page dimensions unavailable".into())));
                    let _=responses.send(Event::PageUnitsLoaded(id,units));
                    let _=responses.send(Event::ReviewLoaded(id,result));
                }
                Some(Command::ExportWorkspace{id,placements,review,mode,destination})=>{
                    let result=(||->Result<_> {
                        let document=documents.get(&id).context("Document was closed")?;
                        let source=crate::assembly::SourceSnapshot{path:document.path.clone(),bytes:document.working_bytes()?};
                        let review=crate::review::prepare(source.clone(),&document.pdf,&review,mode)?;
                        let overlays=if placements.is_empty(){None}else{Some(crate::overlay::prepare(&pdfium,&document.pdf,source,&placements)?)};
                        Ok((review,overlays))
                    })();
                    match result {
                        Ok((mut review,overlays))=>{let reply=responses.clone();thread::spawn(move || {
                            let result=(||->Result<_>{
                                if let Some(overlays)=overlays {let folder=tempfile::tempdir()?;let path=folder.path().join("marked-base.pdf");overlays.save(&path)?;review.set_source_bytes(Arc::new(std::fs::read(path)?));}
                                review.save(&destination)
                            })().map_err(|e|format!("{e:#}"));
                            let _=reply.send(Event::ExportFinished(result));
                        });},
                        Err(e)=>{let _=responses.send(Event::ExportFinished(Err(format!("{e:#}"))));}
                    }
                }
                Some(Command::Snapshot(id)) => {
                    if let Some(document)=documents.get(&id) {let _=responses.send(Event::Snapshot(id,document.original.clone()));}
                }
                Some(Command::Close(id)) => {
                    passwords.remove(&id);
                    documents.remove(&id);
                    cache.retain(|k: &RenderKey| k.document != id);
                    if search.as_ref().is_some_and(|s| s.id == id) { search = None; }
                }
                Some(Command::Render { generation, key }) => {
                    if generation != active.load(Ordering::Relaxed) { continue; }
                    let Some(document) = documents.get(&key.document) else { continue; };
                    let result = match cache.get(&key) { Some(frame) => Ok(frame), None => render(&document.pdf, &key).map(|frame| {
                        let frame = Arc::new(frame); cache.insert(key.clone(), frame.clone(), frame.rgba.len()); frame
                    }) };
                    if generation != active.load(Ordering::Relaxed) { continue; }
                    let event = match result {
                        Ok(frame) => Event::Rendered { generation, key, frame },
                        Err(error) => Event::RenderFailed { generation, key, message: format!("{error:#}") },
                    };
                    if responses.send(event).is_err() { break; }
                }
                Some(Command::Search { id, token, query }) => {
                    search = Some(SearchJob { id, token, query, page: 0, hits: vec![], truncated: false });
                }
                Some(Command::SelectText { id, page, token, bounds }) => {
                    let result = (|| -> Result<String> {
                        let document = documents.get(&id).context("Document was closed")?;
                        selected_text(&document.pdf.pages().get(page as i32)?, bounds)
                    })().map_err(|error| format!("{error:#}"));
                    let _ = responses.send(Event::SelectedText { id, token, text: result });
                }
                Some(Command::ExportPages { id, operation, additional, destination }) => {
                    let original=match documents.get(&id).context("Document was closed").and_then(|d|Ok(crate::assembly::SourceSnapshot{path:d.path.clone(),bytes:d.working_bytes()?})) {
                        Ok(source)=>source,Err(e)=>{let _=responses.send(Event::ExportFinished(Err(format!("{e:#}"))));continue;}
                    };
                    // Image creation shares this worker's PDFium instance. Never initialize or
                    // destroy another PDFium library while the reader has live documents.
                    let prepared = crate::image_pages::prepare(&pdfium, &documents[&id].pdf, &original, &operation, &additional);
                    let mut sources = vec![original];
                    match prepared {
                        Ok(additional) => sources.extend(additional),
                        Err(error) => {
                            let _ = responses.send(Event::ExportFinished(Err(format!("{error:#}"))));
                            continue;
                        }
                    }
                    let reply = responses.clone();
                    thread::spawn(move || {
                        let result = crate::assembly::export(&sources, &operation, &destination)
                            .map_err(|error| format!("{error:#}"));
                        let _ = reply.send(Event::ExportFinished(result));
                    });
                }
                Some(Command::ExportMarks { id, placements, destination }) => {
                    let result=(|| -> Result<_> {
                        let document=documents.get(&id).context("Document was closed")?;
                        crate::overlay::prepare(&pdfium,&document.pdf,crate::assembly::SourceSnapshot{path:document.path.clone(),bytes:document.working_bytes()?},&placements)
                    })();
                    match result {
                        Ok(prepared)=> {let reply=responses.clone();thread::spawn(move || {let _=reply.send(Event::ExportFinished(prepared.save(&destination).map_err(|e|format!("{e:#}"))));});},
                        Err(error)=>{let _=responses.send(Event::ExportFinished(Err(format!("{error:#}"))));},
                    }
                }
                #[cfg(any(target_os="linux",target_os="windows"))]
                Some(Command::PrintNative { id,current,range,printer,fit }) => {
                    let Some(document)=documents.get(&id) else{let _=responses.send(Event::PrintFinished(Err("Document was closed".into())));continue;};
                    let original=match document.working_bytes(){Ok(bytes)=>bytes,Err(e)=>{let _=responses.send(Event::PrintFinished(Err(format!("{e:#}"))));continue;}};
                    let path=document.path.clone();
                    let reply=responses.clone();
                    thread::spawn(move || {let _=reply.send(Event::PrintFinished(crate::platform::native_print(&original,&path,current,&range,&printer,fit).map_err(|e|format!("{e:#}"))));});
                }
                Some(Command::Print { id, range, printer, fit }) => {
                    let original=match documents.get(&id).context("Document was closed").and_then(OpenDocument::working_bytes) {
                        Ok(bytes)=>bytes,Err(e)=>{let _=responses.send(Event::PrintFinished(Err(format!("{e:#}"))));continue;}
                    };
                    let reply = responses.clone();
                    // Spooler/print-helper latency must not block PDF rendering or searching.
                    thread::spawn(move || {
                        let result = (|| -> Result<String> {
                            use std::io::Write;
                            let mut snapshot = tempfile::Builder::new().prefix("dpdf-print-").suffix(".pdf").tempfile()?;
                            snapshot.write_all(&original)?;
                            snapshot.flush()?;
                            crate::platform::print_pdf(snapshot.path(), &range, &printer, fit)
                        })().map_err(|error| format!("{error:#}"));
                        let _ = reply.send(Event::PrintFinished(result));
                    });
                }
                Some(Command::SaveCopy(id, path)) => {
                    let result = documents.get(&id).context("Document was closed").and_then(|d| save_original(&d.original, &path));
                    let message = match result { Ok(()) => format!("Saved unchanged PDF to {}", path.display()), Err(e) => format!("Save failed: {e:#}") };
                    if responses.send(Event::Notice(message)).is_err() { break; }
                }
                None => {
                    if let Some(job) = &mut search {
                        if search_active.load(Ordering::Relaxed) != job.token { search = None; continue; }
                        let Some(document) = documents.get(&job.id) else { search = None; continue; };
                        if job.page >= document.pdf.pages().len() as usize || job.query.is_empty() || job.truncated {
                            let _ = responses.send(Event::SearchResults { id: job.id, token: job.token, hits: std::mem::take(&mut job.hits), truncated: job.truncated });
                            search = None;
                        } else {
                            let result = (|| -> Result<(Vec<SearchHit>, bool)> {
                                let page = document.pdf.pages().get(job.page as i32)?;
                                find_matches(&page, job.page, &job.query, 10_000 - job.hits.len(), || search_active.load(Ordering::Relaxed) != job.token)
                            })();
                            match result {
                                Ok((hits, truncated)) => { job.hits.extend(hits); job.truncated = truncated; }
                                Err(error) => {
                                    let _ = responses.send(Event::Notice(format!("Search failed on page {}: {error}", job.page + 1)));
                                    let _ = responses.send(Event::SearchResults { id: job.id, token: job.token, hits: vec![], truncated: false });
                                    search = None;
                                    continue;
                                }
                            }
                            job.page += 1;
                        }
                    } else { thread::sleep(Duration::from_millis(1)); }
                }
            }
        }
    }).expect("start PDF worker");
    Worker {
        commands,
        events,
        generation,
        search_token,
    }
}

pub use crate::storage::atomic_write as save_original;

/// Headless acceptance probe using the same engine and render path as the GUI.
pub fn probe(path: &Path) -> Result<()> {
    probe_page(path, 0)
}

/// Headless phase measurements of the reader's immutable-byte / describe / render
/// path. These are engine delivery times, not physical display presentation times.
pub fn benchmark_reader(path: &Path, compare_eager: bool) -> Result<()> {
    use std::time::Instant;
    let start = Instant::now();
    let pdfium = bind_pdfium()?;
    let bound = start.elapsed();
    let bytes = Arc::new(std::fs::read(path).context("Read reader benchmark input")?);
    let read = start.elapsed();
    let pdf = load_shared_pdf(&pdfium, bytes.clone(), None)?;
    let loaded = start.elapsed();
    let info = describe(0, path.to_path_buf(), &pdf, false)?;
    let described = start.elapsed();
    let key = RenderKey {
        document: 0,
        page: 0,
        width: 1000,
        rotation: 0,
        thumbnail: false,
    };
    let first = Arc::new(render(&pdf, &key)?);
    let first_ready = start.elapsed();
    let mut cache = ByteCache::new(64 * 1024 * 1024);
    cache.insert(key.clone(), first.clone(), first.rgba.len());
    let mut samples = Vec::new();
    for (name, page, width, thumbnail) in [
        ("page_7", 6.min(info.pages.len() - 1), 1000, false),
        ("last_page", info.pages.len() - 1, 1000, false),
        ("zoom", 0, 2400, false),
        ("thumbnail", 0, 148, true),
        ("revisit", 0, 1000, false),
    ] {
        let key = RenderKey {
            document: 0,
            page,
            width,
            rotation: 0,
            thumbnail,
        };
        let sample_start = Instant::now();
        let existing = cache.get(&key);
        let hit = existing.is_some();
        let frame = match existing {
            Some(frame) => frame,
            None => {
                let frame = Arc::new(render(&pdf, &key)?);
                cache.insert(key, frame.clone(), frame.rgba.len());
                frame
            }
        };
        samples.push(serde_json::json!({"name":name,"page":page+1,"render_ms":sample_start.elapsed().as_secs_f64()*1000.0,
            "width":frame.width,"height":frame.height,"frame_bytes":frame.rgba.len(),"cache_hit":hit}));
    }
    let comparison = if compare_eager {
        // Fresh documents and the same engine: metadata gathering alone is timed,
        // not file I/O/library startup. Never bind a second PDFium engine here.
        let lazy_pdf = load_shared_pdf(&pdfium, bytes.clone(), None)?;
        let started = Instant::now();
        let lazy = lazy_pdf.pages().page_sizes()?;
        let lazy_ms = started.elapsed().as_secs_f64() * 1000.0;
        drop(lazy_pdf);
        let eager_pdf = load_shared_pdf(&pdfium, bytes.clone(), None)?;
        let started = Instant::now();
        let mut sizes = Vec::with_capacity(info.pages.len());
        for index in eager_pdf.pages().as_range() {
            let page = eager_pdf.pages().get(index)?;
            sizes.push((page.width().value, page.height().value));
        }
        let eager_ms = started.elapsed().as_secs_f64() * 1000.0;
        let mismatches = lazy.iter().zip(&sizes).enumerate().filter_map(|(i,(rect,(w,h)))| {
            ((rect.width().value-w).abs()>0.01 || (rect.height().value-h).abs()>0.01)
                .then_some(serde_json::json!({"page":i+1,"metadata":[rect.width().value,rect.height().value],"loaded":[w,h]}))
        }).collect::<Vec<_>>();
        Some(
            serde_json::json!({"lazy_page_sizes_ms":lazy_ms,"eager_page_sizes_ms":eager_ms,
            "geometry_matches_loaded":mismatches.is_empty() && lazy.len()==sizes.len(),"mismatches":mismatches}),
        )
    } else {
        None
    };
    println!(
        "{}",
        serde_json::json!({"schema_version":1,"scope":"headless_reader_engine",
        "input_bytes":bytes.len(),"pages":info.pages.len(),"bookmarks":info.bookmarks.len(),
        "bind_ms":bound.as_secs_f64()*1000.0,"read_ms":(read-bound).as_secs_f64()*1000.0,
        "load_ms":(loaded-read).as_secs_f64()*1000.0,"metadata_ms":(described-loaded).as_secs_f64()*1000.0,
        "first_render_ms":(first_ready-described).as_secs_f64()*1000.0,"first_page_ready_ms":first_ready.as_secs_f64()*1000.0,
        "first_frame_bytes":first.rgba.len(),"cache_bytes":cache.bytes_used(),"cache_budget_bytes":64*1024*1024,
        "samples":samples,"comparison":comparison})
    );
    Ok(())
}

/// Generated-fixture raster export for checking placement and alpha compositing.
pub fn render_probe(path: &Path, index: usize, output: &Path, width: i32) -> Result<()> {
    use std::io::Write;
    ensure!(
        (32..=3072).contains(&width),
        "Raster probe width must be between 32 and 3072"
    );
    crate::storage::ensure_separate_output(output, &[path])?;
    let pdfium = bind_pdfium()?;
    let pdf = pdfium.load_pdf_from_file(path, None)?;
    let frame = render(
        &pdf,
        &RenderKey {
            document: 0,
            page: index,
            width,
            rotation: 0,
            thumbnail: false,
        },
    )?;
    let mut file = std::io::BufWriter::new(std::fs::File::create(output)?);
    write!(file, "P6\n{} {}\n255\n", frame.width, frame.height)?;
    for pixel in frame.rgba.as_chunks::<4>().0 {
        file.write_all(&pixel[..3])?;
    }
    file.flush()?;
    Ok(())
}

pub fn probe_selection(path: &Path, page: usize, bounds: NormalizedRect) -> Result<()> {
    let pdfium = bind_pdfium()?;
    let document = pdfium.load_pdf_from_file(path, None)?;
    let text = selected_text(&document.pages().get(page as i32)?, bounds)?;
    println!("{}", serde_json::json!({ "text": text }));
    Ok(())
}

pub fn probe_page(path: &Path, index: usize) -> Result<()> {
    let started = std::time::Instant::now();
    let pdfium = bind_pdfium()?;
    let pdf = pdfium.load_pdf_from_file(path, None)?;
    if pdf.pages().is_empty() {
        bail!("PDF has no pages");
    }
    let opened = started.elapsed();
    let frame = render(
        &pdf,
        &RenderKey {
            document: 0,
            page: index,
            width: 1000,
            rotation: 0,
            thumbnail: false,
        },
    )?;
    let first_frame = started.elapsed();
    let text = pdf.pages().get(index as i32)?.text()?.all();
    let checksum = frame.rgba.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ *byte as u64).wrapping_mul(0x100000001b3)
    });
    println!(
        "pages={} first_frame={}x{} text_chars={} open_ms={} first_frame_ms={} rgba_checksum={:016x}",
        pdf.pages().len(),
        frame.width,
        frame.height,
        text.chars().count(),
        opened.as_millis(),
        first_frame.as_millis(),
        checksum
    );
    Ok(())
}

/// Machine-readable search probe for the generated acceptance corpus.
pub fn probe_search(path: &Path, query: &str) -> Result<()> {
    let pdfium = bind_pdfium()?;
    let pdf = pdfium.load_pdf_from_file(path, None)?;
    let mut hits = Vec::new();
    let mut truncated = false;
    if !query.is_empty() {
        for (index, page) in pdf.pages().iter().enumerate() {
            let (found, limited) =
                find_matches(&page, index, query, 10_000 - hits.len(), || false)?;
            hits.extend(found);
            if limited {
                truncated = true;
                break;
            }
        }
    }
    let rectangles: usize = hits.iter().map(|h| h.bounds.len()).sum();
    let first = hits
        .first()
        .and_then(|h| h.bounds.first())
        .map(|r| vec![r.x, r.y, r.width, r.height])
        .unwrap_or_default();
    println!(
        "{{\"matches\":{},\"rectangles\":{},\"truncated\":{},\"first\":{:?}}}",
        hits.len(),
        rectangles,
        truncated,
        first
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_reader_retains_snapshot_without_copy_and_supports_random_access() {
        use std::io::{Read, Seek, SeekFrom};
        let bytes = Arc::new(b"immutable opened PDF bytes".to_vec());
        let weak = Arc::downgrade(&bytes);
        let original_pointer = bytes.as_ptr();
        let mut reader = std::io::Cursor::new(SharedPdfBytes(bytes.clone()));
        assert_eq!(reader.get_ref().as_ref().as_ptr(), original_pointer);
        assert_eq!(Arc::strong_count(&bytes), 2);
        drop(bytes);
        reader.seek(SeekFrom::End(-5)).unwrap();
        let mut tail = [0; 5];
        reader.read_exact(&mut tail).unwrap();
        assert_eq!(&tail, b"bytes");
        reader.seek(SeekFrom::Start(0)).unwrap();
        let mut prefix = [0; 9];
        reader.read_exact(&mut prefix).unwrap();
        assert_eq!(&prefix, b"immutable");
        assert!(weak.upgrade().is_some());
        drop(reader);
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn save_copy_preserves_original_bytes_and_replaces_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("contract.pdf");
        std::fs::write(&destination, b"previous").unwrap();
        let bytes = b"%PDF-1.7\noriginal signed bytes\n%%EOF\n";
        save_original(bytes, &destination).unwrap();
        assert_eq!(std::fs::read(destination).unwrap(), bytes);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
