//! Byte-only overlay preparation/export with publication serialized against tab close.
use super::*;
use std::io::Read;

pub(super) struct Save {
    review: Option<crate::review::Prepared>,
    overlays: Option<crate::overlay::PreparedExport>,
    destination: PathBuf,
    source: PathBuf,
}

impl Save {
    pub(super) fn run(
        self,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<crate::assembly::StagedExport> {
        crate::background_jobs::check_cancelled(Some(cancel))?;
        let staged = crate::assembly::StagedExport::new(&self.destination, vec![self.source])?;
        let result = match (self.review, self.overlays) {
            (Some(mut review), overlays) => {
                if let Some(overlays) = overlays {
                    let folder = tempfile::tempdir()?;
                    let path = folder.path().join("marked-base.pdf");
                    overlays.save_cancellable(&path, Some(cancel))?;
                    let mut input = std::fs::File::open(&path)?;
                    let length = usize::try_from(input.metadata()?.len())
                        .context("Marked snapshot is too large")?;
                    let mut bytes = Vec::new();
                    bytes
                        .try_reserve_exact(length)
                        .context("Reserve marked snapshot memory")?;
                    loop {
                        crate::background_jobs::check_cancelled(Some(cancel))?;
                        let mut chunk = [0; 64 * 1024];
                        let count = input.read(&mut chunk)?;
                        if count == 0 {
                            break;
                        }
                        ensure!(
                            count <= length.saturating_sub(bytes.len()),
                            "Marked snapshot changed while reading"
                        );
                        bytes.extend_from_slice(&chunk[..count]);
                    }
                    ensure!(
                        bytes.len() == length,
                        "Marked snapshot changed while reading"
                    );
                    review.set_source_bytes(Arc::new(bytes));
                }
                review.save_cancellable(staged.path(), Some(cancel))?
            }
            (None, Some(overlays)) => overlays.save_cancellable(staged.path(), Some(cancel))?,
            _ => bail!("Missing prepared export"),
        };
        crate::background_jobs::check_cancelled(Some(cancel))?;
        Ok(staged.with_pages(result.pages))
    }
}

enum Phase {
    Inspect(Box<(Command, crate::assembly::SourceSnapshot)>),
    Save,
}

pub(super) struct Pending {
    pub(super) request: Request,
    phase: Phase,
}

fn prepare(
    pdfium: &Pdfium,
    document: &PdfDocument<'_>,
    original: crate::assembly::SourceSnapshot,
    command: Command,
    metadata: Option<&serde_json::Value>,
) -> Result<Save> {
    let source = original.path.clone();
    let overlay = |original, placements: &[crate::overlay::Placement]| -> Result<_> {
        let units = crate::pdf_units::from_metadata(
            metadata.context("Missing overlay metadata")?,
            document.pages().len() as usize,
        )?;
        crate::overlay::prepare_with_units(pdfium, document, original, placements, &units)
    };
    let (review, overlays, destination) = match command {
        Command::ExportMarks {
            placements,
            destination,
            ..
        } => (None, Some(overlay(original, &placements)?), destination),
        Command::ExportWorkspace {
            placements,
            review,
            mode,
            destination,
            ..
        } => {
            let prepared = crate::review::prepare(original.clone(), document, &review, mode)?;
            let overlays = if placements.is_empty() {
                None
            } else {
                Some(overlay(original, &placements)?)
            };
            (Some(prepared), overlays, destination)
        }
        _ => bail!("Unexpected overlay export command"),
    };
    Ok(Save {
        review,
        overlays,
        destination,
        source,
    })
}

pub(super) fn start(
    pdfium: &Pdfium,
    documents: &HashMap<DocumentId, OpenDocument<'_>>,
    jobs: &mut Jobs<ReviewWork, ReviewData>,
    command: Command,
) -> Result<Pending> {
    let id = command
        .working_document()
        .context("Missing export document")?;
    let document = documents.get(&id).context("Document was closed")?;
    let original = crate::assembly::SourceSnapshot {
        path: document.path.clone(),
        bytes: document.working_bytes()?,
    };
    // Review-only export already inspects its metadata in save_cancellable.
    // Do not make it depend on overlay-specific unit validation.
    if matches!(&command, Command::ExportWorkspace { placements, .. } if placements.is_empty()) {
        let save = prepare(pdfium, &document.pdf, original, command, None)?;
        let request = jobs.submit(id, ReviewWork::SaveOverlay(Box::new(save)))?;
        Ok(Pending {
            request,
            phase: Phase::Save,
        })
    } else {
        let request = jobs.submit(
            id,
            ReviewWork::Inspect {
                bytes: original.bytes.clone(),
                password: None,
            },
        )?;
        Ok(Pending {
            request,
            phase: Phase::Inspect(Box::new((command, original))),
        })
    }
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
    let Some(mut state) = pending.remove(&generation) else {
        return;
    };
    if !state.request.matches(&completion.ticket) {
        return;
    }
    let result = (|| -> Result<Option<crate::assembly::ExportResult>> {
        let document = documents
            .get(&state.request.id)
            .context("Document was closed")?;
        match state.phase {
            Phase::Inspect(operation) => {
                let metadata = match completion.result {
                    Ok(ReviewData::Inspected { metadata, .. }) => metadata,
                    Err(message) => bail!(message),
                    _ => bail!("Unexpected overlay metadata result"),
                };
                let (command, original) = *operation;
                let save = prepare(pdfium, &document.pdf, original, command, Some(&metadata))?;
                jobs.continue_task(completion.ticket, ReviewWork::SaveOverlay(Box::new(save)))?;
                state.phase = Phase::Save;
                pending.insert(generation, state);
                Ok(None)
            }
            Phase::Save => {
                let staged = match completion.result {
                    Ok(ReviewData::Exported(staged)) => staged,
                    Err(message) => bail!(message),
                    _ => bail!("Unexpected prepared export result"),
                };
                // Close and publication execute on this same owner thread.
                // A completion dropped after Close cannot replace its destination.
                ensure!(
                    state.request.matches(&completion.ticket),
                    "Export cancelled before publication"
                );
                Ok(Some(staged.publish()?))
            }
        }
    })();
    match result {
        Ok(None) => {}
        result => {
            let _ = responses.send(Event::ExportFinished(
                result
                    .and_then(|value| value.context("Missing prepared export result"))
                    .map_err(|error| format!("{error:#}")),
            ));
        }
    }
}
