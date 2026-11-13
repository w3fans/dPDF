//! Sequential input snapshots and image decoding; native image pages stay on the PDF owner.
use super::*;
use crate::assembly::{PageOperation, SourceSnapshot, StagedExport};

struct Generated {
    path: PathBuf,
    bytes: Vec<u8>,
    unit: f64,
}

pub(super) struct Work {
    target: crate::image_pages::Target,
    inputs: std::vec::IntoIter<PathBuf>,
    sources: Vec<SourceSnapshot>,
    operation: PageOperation,
    destination: PathBuf,
    unit: Option<f64>,
    generated: Option<Generated>,
}

pub(super) struct Image {
    work: Work,
    path: PathBuf,
    prepared: crate::image_pages::PreparedImage,
}

impl Work {
    pub(super) fn run(mut self, cancel: &std::sync::atomic::AtomicBool) -> Result<ReviewData> {
        crate::background_jobs::check_cancelled(Some(cancel))?;
        if let Some(generated) = self.generated.take() {
            let bytes = crate::pdf_units::set_image_page_unit_cancellable(
                generated.bytes,
                generated.unit,
                Some(cancel),
            )?;
            self.sources.push(SourceSnapshot {
                path: generated.path,
                bytes: Arc::new(bytes),
            });
        }
        while let Some(path) = self.inputs.next() {
            let (bytes, image) = crate::image_pages::read_source(&path, Some(cancel))?;
            if image {
                let unit = match self.unit {
                    Some(unit) => unit,
                    None => {
                        let unit = self.target.unit(&self.sources[0], Some(cancel))?;
                        self.unit = Some(unit);
                        unit
                    }
                };
                let prepared = self
                    .target
                    .prepare(bytes, unit, Some(cancel))
                    .with_context(|| format!("Import page image {}", path.display()))?;
                // At most one decoded image is handed off at a time. Do not
                // accumulate RGBA for every selected image before native creation.
                return Ok(ReviewData::PageImage(Box::new(Image {
                    work: self,
                    path,
                    prepared,
                })));
            }
            self.sources.push(SourceSnapshot {
                path,
                bytes: Arc::new(bytes),
            });
        }
        let protected = self
            .sources
            .iter()
            .map(|source| source.path.clone())
            .collect();
        let staged = StagedExport::new(&self.destination, protected)?;
        let result = crate::assembly::export_cancellable(
            &self.sources,
            &self.operation,
            staged.path(),
            Some(cancel),
        )?;
        crate::background_jobs::check_cancelled(Some(cancel))?;
        Ok(ReviewData::Exported(staged.with_pages(result.pages)))
    }
}

pub(super) struct Pending {
    pub(super) request: Request,
}

pub(super) fn start(
    documents: &HashMap<DocumentId, OpenDocument<'_>>,
    jobs: &mut Jobs<ReviewWork, ReviewData>,
    id: DocumentId,
    operation: PageOperation,
    inputs: Vec<PathBuf>,
    destination: PathBuf,
) -> Result<Pending> {
    let document = documents.get(&id).context("Document was closed")?;
    let target = crate::image_pages::Target::new(&document.pdf, &operation, !inputs.is_empty())?;
    let source = SourceSnapshot {
        path: document.path.clone(),
        bytes: document.working_bytes()?,
    };
    let work = Work {
        target,
        inputs: inputs.into_iter(),
        sources: vec![source],
        operation,
        destination,
        unit: None,
        generated: None,
    };
    let request = jobs.submit(id, ReviewWork::Pages(Box::new(work)))?;
    Ok(Pending { request })
}

pub(super) fn completed(
    pdfium: &Pdfium,
    documents: &HashMap<DocumentId, OpenDocument<'_>>,
    pending: &mut HashMap<u64, Pending>,
    jobs: &Jobs<ReviewWork, ReviewData>,
    responses: &SyncSender<Event>,
    completion: Reply<ReviewData>,
) {
    let generation = completion.ticket.request.generation;
    let Some(state) = pending.remove(&generation) else {
        return;
    };
    if !state.request.matches(&completion.ticket) {
        return;
    }
    let result = (|| -> Result<Option<crate::assembly::ExportResult>> {
        ensure!(
            documents.contains_key(&state.request.id),
            "Document was closed"
        );
        match completion.result {
            Ok(ReviewData::PageImage(image)) => {
                let Image {
                    mut work,
                    path,
                    prepared,
                } = *image;
                let (bytes, unit) = prepared.create(pdfium)?;
                work.generated = Some(Generated { path, bytes, unit });
                jobs.continue_task(completion.ticket, ReviewWork::Pages(Box::new(work)))?;
                pending.insert(generation, state);
                Ok(None)
            }
            Ok(ReviewData::Exported(staged)) => {
                ensure!(
                    state.request.matches(&completion.ticket),
                    "Export cancelled before publication"
                );
                Ok(Some(staged.publish()?))
            }
            Err(message) => bail!(message),
            _ => bail!("Unexpected page preparation result"),
        }
    })();
    match result {
        Ok(None) => {}
        result => {
            let _ = responses.send(Event::ExportFinished(
                result
                    .and_then(|value| value.context("Missing page export result"))
                    .map_err(|error| format!("{error:#}")),
            ));
        }
    }
}
