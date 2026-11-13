//! Document-bound preparation followed by one owned immutable print action.
use super::*;
use std::{
    io::{Read, Write},
    sync::atomic::AtomicBool,
};

pub(super) struct Options {
    current: usize,
    range: String,
    printer: String,
    fit: bool,
    native: bool,
}

enum Snapshot {
    Bytes(Arc<Vec<u8>>),
    File {
        _folder: tempfile::TempDir,
        path: PathBuf,
    },
}

pub(super) struct Ready {
    snapshot: Snapshot,
    source: PathBuf,
    options: Options,
}

pub(super) struct Preparation {
    ready: Ready,
    review: Option<crate::review::Prepared>,
}
impl Preparation {
    pub(super) fn run(mut self, cancel: &AtomicBool) -> Result<Ready> {
        crate::background_jobs::check_cancelled(Some(cancel))?;
        if let Some(review) = self.review.take() {
            let folder = tempfile::tempdir()?;
            let path = folder.path().join("review-print.pdf");
            review.save_cancellable(&path, Some(cancel))?;
            self.ready.snapshot = Snapshot::File {
                _folder: folder,
                path,
            };
        }
        crate::background_jobs::check_cancelled(Some(cancel))?;
        Ok(self.ready)
    }
}

fn read_native(path: &Path, cancel: &AtomicBool) -> Result<Vec<u8>> {
    let mut input = std::fs::File::open(path)?;
    let length = usize::try_from(input.metadata()?.len())?;
    ensure!(
        length <= 256 * 1024 * 1024,
        "Native printing supports PDF snapshots up to 256 MiB; use Quick Print for this file"
    );
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .context("Reserve print snapshot memory")?;
    loop {
        crate::background_jobs::check_cancelled(Some(cancel))?;
        let mut chunk = [0; 64 * 1024];
        let count = input.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        ensure!(
            count <= length.saturating_sub(bytes.len()),
            "Print snapshot changed while reading"
        );
        bytes.extend_from_slice(&chunk[..count]);
    }
    ensure!(
        bytes.len() == length,
        "Print snapshot changed while reading"
    );
    Ok(bytes)
}

impl Ready {
    fn run(self, cancel: &AtomicBool) -> Result<String> {
        crate::background_jobs::check_cancelled(Some(cancel))?;
        let o = self.options;
        if o.native {
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            {
                let bytes = match &self.snapshot {
                    Snapshot::Bytes(bytes) => bytes.clone(),
                    Snapshot::File { path, .. } => Arc::new(read_native(path, cancel)?),
                };
                return crate::platform::native_print(
                    &bytes,
                    &self.source,
                    o.current,
                    &o.range,
                    &o.printer,
                    o.fit,
                    cancel,
                );
            }
            #[cfg(not(any(target_os = "linux", target_os = "windows")))]
            {
                bail!("Native printing is unavailable on this platform");
            }
        }
        match &self.snapshot {
            Snapshot::File { path, .. } => {
                crate::platform::print_pdf(path, &o.range, &o.printer, o.fit, Some(cancel))
            }
            Snapshot::Bytes(bytes) => {
                let mut snapshot = tempfile::Builder::new()
                    .prefix("dpdf-print-")
                    .suffix(".pdf")
                    .tempfile()?;
                for chunk in bytes.chunks(64 * 1024) {
                    crate::background_jobs::check_cancelled(Some(cancel))?;
                    snapshot.write_all(chunk)?;
                }
                snapshot.flush()?;
                crate::platform::print_pdf(
                    snapshot.path(),
                    &o.range,
                    &o.printer,
                    o.fit,
                    Some(cancel),
                )
            }
        }
    }
}

pub(super) struct Actor {
    jobs: Jobs<Ready, String>,
    active: Option<Request>,
    cancellation: Cancellation,
    shutdown: Cancellation,
}
impl Actor {
    pub(super) fn new(shutdown: Cancellation) -> Result<Self> {
        Self::with_executor(shutdown, Ready::run)
    }
    fn with_executor(
        shutdown: Cancellation,
        execute: impl Fn(Ready, &AtomicBool) -> Result<String> + Send + 'static,
    ) -> Result<Self> {
        // Separate registry: closing a document after handoff must not dismiss
        // its user-owned dialog. Actor drop/stop still cancels and joins it.
        let cancellation = Cancellation::default();
        let stopping = shutdown.clone();
        let jobs = Jobs::new(cancellation.clone(), move |ready: Ready, _| {
            execute(ready, stopping.shutdown_flag())
        })?;
        Ok(Self {
            jobs,
            active: None,
            cancellation,
            shutdown,
        })
    }
    pub(super) fn busy(&self) -> bool {
        self.active.is_some()
    }
    fn start(&mut self, ready: Ready) -> Result<()> {
        ensure!(!self.busy(), "A print action is already active");
        self.active = Some(self.jobs.submit(0, ready)?);
        Ok(())
    }
    pub(super) fn poll(&mut self, responses: &SyncSender<Event>) {
        while let Some(reply) = self.jobs.poll() {
            self.active = None;
            let _ = responses.send(Event::PrintFinished(reply.result));
        }
    }
    pub(super) fn stop(&self) {
        self.shutdown.stop();
        self.cancellation.stop();
    }
}
impl Drop for Actor {
    fn drop(&mut self) {
        // Signal the flag used by process I/O before Jobs joins its worker.
        self.stop();
    }
}

pub(super) struct Pending {
    pub(super) request: Request,
}
pub(super) fn start(
    documents: &HashMap<DocumentId, OpenDocument<'_>>,
    jobs: &mut Jobs<ReviewWork, ReviewData>,
    command: Command,
) -> Result<Pending> {
    let (id, options, review) = match command {
        Command::PrintReviewed {
            id,
            review,
            current,
            range,
            printer,
            fit,
            native,
        } => (
            id,
            Options {
                current,
                range,
                printer,
                fit,
                native,
            },
            Some(review),
        ),
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        Command::PrintNative {
            id,
            current,
            range,
            printer,
            fit,
        } => (
            id,
            Options {
                current,
                range,
                printer,
                fit,
                native: true,
            },
            None,
        ),
        Command::Print {
            id,
            range,
            printer,
            fit,
        } => (
            id,
            Options {
                current: 0,
                range,
                printer,
                fit,
                native: false,
            },
            None,
        ),
        _ => bail!("Unexpected print preparation command"),
    };
    let document = documents.get(&id).context("Document was closed")?;
    let bytes = document.working_bytes()?;
    let source = document.path.clone();
    let review = review
        .map(|review| {
            crate::review::prepare(
                crate::assembly::SourceSnapshot {
                    path: source.clone(),
                    bytes: bytes.clone(),
                },
                &document.pdf,
                &review,
                crate::review::ExportMode::Current,
            )
        })
        .transpose()?;
    let ready = Ready {
        snapshot: Snapshot::Bytes(bytes),
        source,
        options,
    };
    let request = jobs.submit(
        id,
        ReviewWork::Print(Box::new(Preparation { ready, review })),
    )?;
    Ok(Pending { request })
}

pub(super) fn completed(
    documents: &HashMap<DocumentId, OpenDocument<'_>>,
    pending: Pending,
    actor: &mut Result<Actor>,
    responses: &SyncSender<Event>,
    completion: Reply<ReviewData>,
) {
    let result = (|| -> Result<()> {
        ensure!(
            documents.contains_key(&pending.request.id),
            "Document was closed"
        );
        let ready = match completion.result {
            Ok(ReviewData::Print(ready)) => ready,
            Err(message) => bail!(message),
            _ => bail!("Unexpected print preparation result"),
        };
        let actor = actor
            .as_mut()
            .map_err(|error| anyhow::anyhow!("{error:#}"))?;
        ensure!(!actor.busy(), "A print action is already active");
        ensure!(
            pending.request.claim_publication(&completion.ticket),
            "Print preparation cancelled before handoff"
        );
        actor.start(*ready)
    })();
    if let Err(error) = result {
        let _ = responses.send(Event::PrintFinished(Err(format!("{error:#}"))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::{Duration, Instant},
    };
    fn generated() -> Ready {
        Ready {
            snapshot: Snapshot::Bytes(Arc::new(b"generated print snapshot".to_vec())),
            source: PathBuf::from("generated.pdf"),
            options: Options {
                current: 0,
                range: String::new(),
                printer: String::new(),
                fit: true,
                native: true,
            },
        }
    }
    #[test]
    fn handed_off_action_survives_tab_close_is_single_and_joins_on_app_shutdown() {
        let shutdown = Cancellation::default();
        let count = Arc::new(AtomicUsize::new(0));
        let executed = count.clone();
        let (started, ready) = mpsc::channel();
        let (finished, done) = mpsc::channel();
        let mut actor = Actor::with_executor(shutdown.clone(), move |snapshot, cancel| {
            assert!(matches!(snapshot.snapshot, Snapshot::Bytes(_)));
            executed.fetch_add(1, Ordering::AcqRel);
            started.send(()).unwrap();
            let until = Instant::now() + Duration::from_secs(3);
            while !cancel.load(Ordering::Acquire) {
                assert!(Instant::now() < until, "Print worker was not cancelled");
                thread::sleep(Duration::from_millis(1));
            }
            finished.send(()).unwrap();
            Ok("generated dialog cancelled".into())
        })
        .unwrap();
        actor.start(generated()).unwrap();
        ready.recv_timeout(Duration::from_secs(3)).unwrap();
        shutdown.close_on_success(7, || Ok::<_, ()>(())).unwrap();
        assert!(!shutdown.stopped());
        assert!(actor.start(generated()).is_err());
        assert!(done.recv_timeout(Duration::from_millis(40)).is_err());
        assert_eq!(count.load(Ordering::Acquire), 1);
        let start = Instant::now();
        shutdown.stop();
        drop(actor);
        done.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(count.load(Ordering::Acquire), 1);
    }
    #[test]
    fn actor_drop_signals_io_before_join_even_without_explicit_owner_stop() {
        let shutdown = Cancellation::default();
        let (started, ready) = mpsc::channel();
        let mut actor = Actor::with_executor(shutdown.clone(), move |_, cancel| {
            started.send(()).unwrap();
            let until = Instant::now() + Duration::from_secs(3);
            while !cancel.load(Ordering::Acquire) {
                assert!(Instant::now() < until, "Print worker was not cancelled");
                thread::sleep(Duration::from_millis(1));
            }
            Ok("cancelled".into())
        })
        .unwrap();
        actor.start(generated()).unwrap();
        ready.recv_timeout(Duration::from_secs(3)).unwrap();
        let start = Instant::now();
        drop(actor);
        assert!(shutdown.stopped());
        assert!(start.elapsed() < Duration::from_secs(1));
    }
}
