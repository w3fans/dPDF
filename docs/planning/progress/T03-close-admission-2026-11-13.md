# T03 accepted-close/publication ordering reproduction

Status: INVESTIGATION; deterministic regression prepared, hosted result PENDING.
Base is qualified overlay head 10b443f4. No source fix is claimed yet.

Application::close removes the UI tab once Command::Close is admitted. The PDF
owner polls helper replies before reading queued commands, so a ready staged
reply can still see its native document/pending request before queued Close is
processed. Existing source-bound tests hold helpers until Close/shutdown and do
not exercise this ready-result boundary. The current design explicitly serializes
backend Close/publication; it has not proven cancellation at UI acceptance.

Introduce one Worker::try_send admission method and route the existing UI command
call through it, with the same forwarding behavior. A pure worker test creates a
real bounded helper reply, admits Close into an unconsumed command queue, and
requires its request to be cancelled before owner cleanup. This should expose the
suspected gap without creating a native engine. A second test requires a rejected
Close on a full queue to leave the open document's request usable.

Run these on approved hosted infrastructure; no local Rust compilation. Once
reproduced, cancellation must be signalled atomically with successful admission,
including work not yet registered when the accepted Close waits in the queue.
Publication needs an atomic claim against cancellation, with an explicit commit
point: a publication already claimed before Close may finish. Do not use a load
of a bool followed by an unguarded rename as proof of this ordering.

The existing lifecycle review gate remains required. This investigation does not
qualify the separate image/page draft or promise rollback after commit begins.
