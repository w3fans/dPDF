//! Keep one latest search while the bounded PDF worker queue is full.
use crate::{document::DocumentId, pdf::Command};
use std::sync::mpsc::{SyncSender, TrySendError};

#[derive(Default)]
pub(super) struct SearchQueue {
    pending: Option<Command>,
}

impl SearchQueue {
    pub(super) fn replace(&mut self, id: DocumentId, token: u64, query: String) {
        self.pending = Some(Command::Search { id, token, query });
    }

    /// Full queues are retried by the UI timer; cancelled/tab-switched searches
    /// are discarded before they can consume worker time.
    pub(super) fn flush(
        &mut self,
        sender: &SyncSender<Command>,
        active: Option<DocumentId>,
        current_token: u64,
    ) -> Result<(), ()> {
        let Some(command) = self.pending.take() else {
            return Ok(());
        };
        if !matches!(&command, Command::Search { id, token, .. }
            if Some(*id) == active && *token == current_token)
        {
            return Ok(());
        }
        match sender.try_send(command) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(command)) => {
                self.pending = Some(command);
                Ok(())
            }
            Err(TrySendError::Disconnected(_)) => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{self, TryRecvError};

    #[test]
    fn retries_only_the_latest_query_after_queue_pressure() {
        let (sender, receiver) = mpsc::sync_channel(1);
        assert!(sender.try_send(Command::Snapshot(1)).is_ok());
        let mut searches = SearchQueue::default();
        searches.replace(1, 10, "obsolete".into());
        assert!(searches.flush(&sender, Some(1), 10).is_ok());
        searches.replace(1, 11, "latest".into());
        assert!(searches.flush(&sender, Some(1), 11).is_ok());
        assert!(matches!(receiver.try_recv(), Ok(Command::Snapshot(1))));
        assert!(searches.flush(&sender, Some(1), 11).is_ok());
        assert!(matches!(receiver.try_recv(),
            Ok(Command::Search { id: 1, token: 11, query }) if query == "latest"));
        assert!(searches.flush(&sender, Some(1), 11).is_ok());
        assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn cancelled_or_inactive_document_searches_are_not_retried() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut searches = SearchQueue::default();
        for (active, token) in [(Some(2), 10), (Some(1), 11), (None, 10)] {
            searches.replace(1, 10, "cancelled".into());
            assert!(searches.flush(&sender, active, token).is_ok());
            assert!(searches.pending.is_none());
            assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
        }
    }

    #[test]
    fn disconnected_worker_is_reported_once_and_clears_pending_work() {
        let (sender, receiver) = mpsc::sync_channel(1);
        drop(receiver);
        let mut searches = SearchQueue::default();
        searches.replace(1, 10, "query".into());
        assert!(searches.flush(&sender, Some(1), 10).is_err());
        assert!(searches.pending.is_none());
        assert!(searches.flush(&sender, Some(1), 10).is_ok());
    }
}
