# T03 accepted-close/publication ordering reproduction

Status: REPRODUCED; close-admission fix prepared, its own qualification PENDING.
Base is qualified overlay head 10b443f4. The initial investigation is retained below.

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


## Reproduction and proposed fix (14 November local time)

Initial regression head ee3dd536 failed strict Clippy before running because the
forwarding wrapper returned a 192-byte Command error. Kept its error as a small
static status in 1985833a1e9b29fa21dfaadd3258c9b4343018c0. Run 43523509748,
Linux job 132223440754, then reproduced the intended contract failure:
accepted_close_cancels_a_ready_reply_before_owner_cleanup FAILED with
"Accepted tab close still permits a ready helper result to publish". Ninety
other tests passed, including rejected_close_preserves_the_open_documents_request.
No native engine or production file was used by this deterministic reproduction.

The proposed fix registers a close marker and cancels existing tokens under the
same lock as successful nonblocking admission. A rejected send changes nothing.
Registration rejects a closed ID until the owner processes its FIFO Close and
forgets that marker; rejected registration releases its admission lease. This
covers work whose command was already queued but had not registered a helper.

Each token now has running/cancelled/publishing phases. A single atomic claim
orders publication against cancellation; if cancellation wins, publication cannot
begin. If publication claimed first, its atomic replace may finish. No mutex is
held over native engine work or filesystem publication. The normal child-cancel
flag remains compatible with helper supervisors. A cancelled ready overlay reply
reports its terminal failure instead of consuming the pending record silently,
so UI busy state cannot remain stuck before backend Close is processed.

Expanded pure tests cover failed-send preservation, late-registration/lease
recovery and single-use publication claimed before close. Existing ten real-window
helper cases and full native/geometry/cancellation gates remain required on this
new source. It is not yet compiled/runtime-qualified. Image/page draft #16 must
integrate this parent fix and use the same publication claim and cancelled-result
terminal handling before its dependency/review gates can close.
