//! One bounded byte-only helper worker. Native engine handles must never enter it.
use anyhow::{Context, Result, bail};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
};

const MAX_OUTSTANDING: usize = 2;

pub(crate) fn check_cancelled(cancel: Option<&AtomicBool>) -> Result<()> {
    if cancel.is_some_and(|value| value.load(Ordering::Acquire)) {
        bail!("Helper operation cancelled");
    }
    Ok(())
}

#[derive(Clone, Default)]
pub(crate) struct Cancellation(Arc<CancellationState>);
#[derive(Default)]
struct CancellationState {
    stopped: AtomicBool,
    requests: Mutex<Vec<Weak<AtomicBool>>>,
}
impl Cancellation {
    pub fn stopped(&self) -> bool {
        self.0.stopped.load(Ordering::Acquire)
    }
    pub fn stop(&self) {
        self.0.stopped.store(true, Ordering::Release);
        let requests = self.0.requests.lock().unwrap_or_else(|p| p.into_inner());
        for request in requests.iter().filter_map(Weak::upgrade) {
            request.store(true, Ordering::Release);
        }
    }
    fn register(&self) -> Arc<AtomicBool> {
        let mut requests = self.0.requests.lock().unwrap_or_else(|p| p.into_inner());
        let cancel = Arc::new(AtomicBool::new(self.stopped()));
        requests.retain(|request| request.strong_count() != 0);
        requests.push(Arc::downgrade(&cancel));
        cancel
    }
}

#[derive(Clone)]
pub(crate) struct Request {
    pub id: u64,
    pub generation: u64,
    cancel: Arc<AtomicBool>,
}
impl Request {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
    pub fn matches(&self, ticket: &Ticket) -> bool {
        self.id == ticket.request.id
            && self.generation == ticket.request.generation
            && !self.cancel.load(Ordering::Acquire)
    }
}

struct Lease(Arc<AtomicUsize>);
impl Drop for Lease {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
pub(crate) struct Ticket {
    pub request: Request,
    _lease: Lease,
}
pub(crate) struct Reply<R> {
    pub ticket: Ticket,
    pub result: std::result::Result<R, String>,
}

pub(crate) struct Jobs<T, R> {
    sender: Option<SyncSender<(Ticket, T)>>,
    replies: Receiver<Reply<R>>,
    thread: Option<JoinHandle<()>>,
    outstanding: Arc<AtomicUsize>,
    cancellation: Cancellation,
    generation: u64,
}
impl<T: Send + 'static, R: Send + 'static> Jobs<T, R> {
    pub fn new(
        cancellation: Cancellation,
        execute: impl Fn(T, &AtomicBool) -> Result<R> + Send + 'static,
    ) -> Result<Self> {
        let (sender, requests) = mpsc::sync_channel::<(Ticket, T)>(MAX_OUTSTANDING);
        // At most two leases exist, including replies awaiting consumption.
        // Each ticket is sent once; this channel cannot accumulate more replies.
        let (responses, replies) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("dpdf-metadata".into())
            .spawn(move || {
                for (ticket, task) in requests {
                    let result = if ticket.request.cancel.load(Ordering::Acquire) {
                        Err("Helper operation cancelled".into())
                    } else {
                        match catch_unwind(AssertUnwindSafe(|| {
                            execute(task, &ticket.request.cancel)
                        })) {
                            Ok(result) => result.map_err(|error| format!("{error:#}")),
                            Err(_) => Err("Background helper task failed".into()),
                        }
                    };
                    if responses.send(Reply { ticket, result }).is_err() {
                        break;
                    }
                }
            })
            .context("Start metadata helper worker")?;
        Ok(Self {
            sender: Some(sender),
            replies,
            thread: Some(thread),
            outstanding: Arc::new(AtomicUsize::new(0)),
            cancellation,
            generation: 0,
        })
    }
    pub fn submit(&mut self, id: u64, task: T) -> Result<Request> {
        if self.cancellation.stopped() {
            bail!("Helper worker is stopping");
        }
        self.generation = self
            .generation
            .checked_add(1)
            .context("Helper identity exhausted")?;
        self.outstanding
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_OUTSTANDING).then_some(count + 1)
            })
            .map_err(|_| {
                anyhow::anyhow!("Metadata preparation busy; retry when a document finishes")
            })?;
        let request = Request {
            id,
            generation: self.generation,
            cancel: self.cancellation.register(),
        };
        let ticket = Ticket {
            request: request.clone(),
            _lease: Lease(self.outstanding.clone()),
        };
        self.continue_task(ticket, task)?;
        Ok(request)
    }
    pub fn continue_task(&self, ticket: Ticket, task: T) -> Result<()> {
        self.sender
            .as_ref()
            .context("Metadata worker stopped")?
            .try_send((ticket, task))
            .map_err(|_| anyhow::anyhow!("Metadata helper queue unavailable"))
    }
    pub fn poll(&self) -> Option<Reply<R>> {
        self.replies.try_recv().ok()
    }
}
impl<T, R> Drop for Jobs<T, R> {
    fn drop(&mut self) {
        self.cancellation.stop();
        // Closing admission drains at most two cancelled tasks. Replies do not
        // block the worker; their leases/bytes drop with this receiver.
        drop(self.sender.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn reply<T: Send + 'static, R: Send + 'static>(jobs: &Jobs<T, R>) -> Reply<R> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(reply) = jobs.poll() {
                return reply;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn admission_counts_unconsumed_results_and_continuations_reuse_their_slot() {
        let mut jobs = Jobs::new(Cancellation::default(), |value: u64, _| Ok(value)).unwrap();
        let first = jobs.submit(7, 10).unwrap();
        let second = jobs.submit(7, 20).unwrap();
        assert!(second.generation > first.generation);
        assert!(jobs.submit(8, 30).is_err());
        let result = reply(&jobs);
        assert!(first.matches(&result.ticket));
        assert!(!second.matches(&result.ticket));
        first.cancel();
        assert!(!first.matches(&result.ticket));
        assert!(jobs.submit(8, 30).is_err());
        jobs.continue_task(result.ticket, 40).unwrap();
        drop(reply(&jobs));
        drop(reply(&jobs));
        let third = jobs.submit(8, 30).unwrap();
        assert!(third.matches(&reply(&jobs).ticket));
    }
    #[test]
    fn shutdown_cancels_and_joins_a_live_task() {
        let cancellation = Cancellation::default();
        let (started, ready) = mpsc::channel();
        let mut jobs = Jobs::new(cancellation.clone(), move |_: (), cancel| {
            started.send(()).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while !cancel.load(Ordering::Acquire) {
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(1));
            }
            Ok(())
        })
        .unwrap();
        jobs.submit(1, ()).unwrap();
        ready.recv_timeout(Duration::from_secs(3)).unwrap();
        let start = Instant::now();
        cancellation.stop();
        drop(jobs);
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(cancellation.register().load(Ordering::Acquire));
    }
    #[test]
    fn failed_task_returns_an_error_and_does_not_poison_the_worker() {
        let mut jobs = Jobs::new(Cancellation::default(), |panic: bool, _| {
            assert!(!panic, "generated task failure");
            Ok(42)
        })
        .unwrap();
        jobs.submit(1, true).unwrap();
        assert!(reply(&jobs).result.is_err());
        jobs.submit(2, false).unwrap();
        assert_eq!(reply(&jobs).result.unwrap(), 42);
    }
}
