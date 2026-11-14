# Metadata helper worker and normal shutdown — review draft

Status: REVIEW draft. Final functional head e71a6726 passed all hosted gates,
eighteen helper-window cases and scoped native X11 print/discovery replays.
Experienced lifecycle review remains required for drafts #10/#16/#17 before
merge. Historical increments and their failures are retained below. Actual
Wayland/native Windows desktop/physical output and operator acceptance remain
separate. Production baking stays disabled.

## Reproduced problem

`LoadReview` previously decrypted, inspected metadata, wrote a clean reading copy
and validated it on the sole PDF worker. Even finite helper deadlines could leave
navigation waiting for tens of seconds. A generated 50-page PDF and a controlled
metadata helper reproduced a blank page 25 until helper release on verified source
`df8a27b4abe4115114dbdde9f6950db2b0699701`. Ordinary UI input continued, but the
queued page render could not run. The source PDF remained unchanged.

## Ownership and admission

One existing PDF worker retains the sole reader PDFium instance and every native
document/page handle. One additional helper thread accepts owned immutable bytes,
private password input, metadata values and prepared byte-only review operations.
No PDFium handle, borrowed page or library initialization enters that thread.
Geometry conversion and loading the resulting clean PDF stay on the PDF worker.

Two admission leases cover active work, queued work and unconsumed results together.
Only one helper task executes at a time. The inspection-to-cleaning continuation
reuses its lease rather than admitting another task. Busy admission returns a
visible retryable review error; it does not suspend rendering or create more threads.
Original snapshots use shared immutable bytes, not re-reading a mutable source.
Decryption retains its 256 MiB output limit; metadata stdout remains 64 MiB. Clean
reading bytes are bounded by the working source size plus 64 MiB and read in
cancellable chunks. These are logical byte limits, not a measured whole-app RSS
budget. Large-metadata CPU work and aggregate reader memory remain performance work.

## Identity and cancellation

Each task carries document ID, pool generation and an atomic cancellation token.
Each UI review request also has a monotonically increasing per-tab generation,
carried through page-unit and review events. Both worker and UI reject stale work.
A tab close or retry cancels its previous request. Late replies release their
bytes/lease without changing a closed/reopened tab or newer review request.

Decryption, metadata inspection and clean-copy qpdf calls observe that token.
Existing supervisor cleanup kills the owned Linux process group or Windows job
and waits for the root helper. Private password stdin and discarded diagnostics
remain unchanged. Clean preparation uses temporary output, preserving the original
and existing atomic destination checks. It checks cancellation before publication.

## Normal shutdown and review gate

`Worker::drop` cancels registered metadata tasks, releases its response receiver
before joining the PDF worker (so a full event queue cannot deadlock teardown),
and releases command admission. The worker checks stop state between requests with
a finite idle wait. Its helper pool closes admission, cancels queued/active tasks
and joins the helper thread. The native document/library destructors remain on
the existing PDF thread, with native handles destroyed before their PDFium owner.

This adds an explicit wait for normal worker teardown instead of allowing process
exit to race detached cleanup. It therefore needs experienced lifecycle review
before merge, as required by the implementation plan's lifecycle/engine rule.
Review must check destructor order, blocked event sends, cancellation during each
phase, no engine handles across threads, and no dependency on another UI callback
to finish shutdown. The candidate must remain a draft until that gate is satisfied.

This does not promise cleanup after SIGKILL, native-engine preemption, cancellation
of user-owned print dialogs, or supervision of every existing detached export/print
thread. Image-page metadata and detached page-export/print ownership still require
subsequent migration. The current overlay migration is specified below. Native user dialogs retain their existing separate policy.

## Required evidence

- Pure worker tests: two-lease bound including unconsumed replies, continuation,
  cancelled identity rejection, joined shutdown and recovery after task panic.
- Generated Linux window test: render a distant page while metadata stays stalled,
  cancel/reopen without stale completion, retry to a new generation, and normal
  app shutdown with direct helper reaped. Source hash must remain unchanged.
- Existing password, review, page-unit, alias, reader, recovery and print suites,
  including native Windows compilation/hosted corpora. Explicit desktop/printer
  acceptance remains separate.
- Replay the visible controlled-stall reproduction on the corrected, verified
  binary. A build or source inspection alone does not prove responsiveness.

Run the generated Linux regression with isolated data and a private display:

```sh
python3 scripts/check-helper-responsiveness.py --binary /path/to/dpdf \
  --pdfium /path/to/libpdfium.so --qpdf /usr/bin/qpdf
```

Reviewer decision: **PENDING** (GO / REVISE / NO-GO, reviewer/date/source required).


## Recovery snapshot validation increment

The prior LoadReview candidate still used synchronous metadata inside
`describe_open(..., recovery=true)`. A generated durable checkpoint, controlled
helper and native XTest page jump reproduced a blank page 25 until helper release
on c9153a54, executable 79731920. Source and checkpoint were generated; the helper
was reaped after release. See progress/T03-recovery-stall-reproduction.json.

The current increment describes native geometry on the PDF worker, then submits
owned snapshot bytes/private password to the same two-lease helper pool. Native
PDF documents stay in the worker's map while recovery is pending; they are never
passed to the helper thread. A pending record holds the request identity and
owned DocumentInfo only. Recovery's Opened event is withheld until returned
metadata validates the physical page sizes, preserving the existing mark-size
safety check. Protected input caches validated decrypted bytes for later use.

On failure, the pending native document is removed on its owner thread and an
OpenFailed event leaves the durable checkpoint available. Busy admission also
fails visibly before publication; it never restores at guessed units. Close
cancels pending recovery by document/request identity. Normal shutdown uses the
existing shared cancellation/join path. Late results cannot publish a closed or
replaced request. No new native engine, helper thread or admission capacity.

The added generated window regression prepares a real durable checkpoint, arms a
metadata stall only after ordinary reading is ready, begins recovery, renders
page 25 in the still-open original tab, then exits normally with recovery pending.
It requires the owned helper reaped, source unchanged and checkpoint retained.
Existing changed/missing/protected recovery, geometry, passwords and review suites
remain required. Recovery candidate 2d75b74b passed the full build, this regression and native
replay in run 43506767494. That evidence is scoped to that candidate; subsequent
increments require their own gates.

Experienced lifecycle review must include this unpublished-to-opened transition,
pending document cleanup, pool admission failure and shutdown ordering on the
reviewed head. The prior review request remains outstanding; green CI alone does
not satisfy it. Production baking and unrelated detached export/print ownership
are outside this increment and remain unqualified.


## Early protected print/export preparation increment

A generated encrypted 50-page PDF and two controlled decryption helpers reproduced
a remaining stall on recovery candidate 2d75b74b (Linux merge b067f63f). Ctrl+P
before initial review inspection finishes invokes another synchronous decrypt
through working_bytes. Page status advances to 25 while the viewport/thumbnail
remain blank until release. Both children were released/reaped and the source
remained unchanged; see progress/T03-protected-preparation-stall.json.

All six print/export command variants now defer missing protected working bytes
through the existing two-lease helper pool. A pending record owns the command
and its document/request identity; no native handle enters it. Commands resume
on the PDF worker only after a matching completion caches immutable decrypted
bytes. The reader-side working_bytes accessor now refuses a missing cache; it
has no synchronous decryption fallback. Passwords remain private process memory
and child stdin, never command arguments, logs or durable drafts.

Multiple operations are keyed by pool generation, so print and export requests
cannot overwrite one another. Admission failure produces the appropriate terminal
print/export event. Tab close cancels all pending commands for that document and
produces terminal failures that clear UI busy state. Late replies cannot restart
a closed operation. Normal shutdown cancels active/queued work and joins the
existing pool; cancelled queued decryptions never start a child. Prepared commands
are drained before accepting more operations, keeping their queue within the
same two-result admission bound. A concurrently completed review may have already
populated the identical immutable decrypted snapshot.

Two new generated window cases keep initial decryption stalled, queue an early
native print or page export, then navigate to page 25. Print quits normally while
preparation is pending; export closes its tab and requires busy state cleared.
Both require source/existing destination preservation, child reaping, and exactly
one child launch (queued decryption must not start after cancellation). GTK is
limited to the file backend; no print is submitted. Full current-source gates and
native replay remain PENDING for this increment. Existing detached final export/
print threads and overlay/image metadata are still separate unfinished work.

The required reviewer must assess this command retention/resumption, shared
admission bound, completion routing, failure event and close/shutdown policy on
the actual reviewed head. Prior green CI does not waive that review gate.


## Overlay preflight and staged publication increment

A recovered generated mark and native Ctrl+Shift+S through a private GTK portal
reproduced another reader stall on candidate 2d75b74b: page status advanced to 25
while content/thumbnail stayed blank until overlay metadata was released. The
separate output then passed qpdf validation, the source stayed unchanged and the
helper was reaped. See progress/T03-overlay-stall-reproduction.json.

Overlay export now has three phases under the same two admission leases:

1. Owned immutable metadata inspection runs on the helper worker. Original native
   documents remain on the PDF worker. Review-only export skips overlay unit
   inspection and retains its independent validation policy.
2. The PDF worker converts validated units and native geometry into owned edited
   page bytes. It reuses the original lease for the final save. No subprocess runs
   in this phase; native engine creation/destruction stays on its sole owner.
3. Byte-only serialized-unit verification, assembly, bookmark repair and review
   export run in the helper pool with cancellation propagated through every helper
   call. The helper writes only to an owned sibling staging file; it never replaces
   the user's destination. Mixed review export reads the validated marked snapshot
   in cancellable chunks, with fallible reservation and a fixed observed file size.

The final completion carries a TempPath, destination, protected input paths and
page count. The document-owning worker checks the request/document again, rechecks
output aliases and atomically publishes. Tab close and publication are serialized
on this worker: if close is processed first, the pending request is removed and
the late TempPath is deleted without replacing the destination. If publication is
processed first, its success precedes close. Checking an AtomicBool only before a
helper-thread rename would leave an unchecked close/publication race; that is why
publication returns to the owner. Normal shutdown cancels helper work and drops
unconsumed staged replies. Cancellation does not promise rollback of an atomic
publication that has already begun.

Admission failure and helper/native preparation errors produce terminal export
failures. Close cancels both inspected and saving phases and clears UI busy state.
Old generation replies cannot be routed into another operation or closed tab.
This replaces detached mark/workspace export threads; image/page assembly and
print threads remain separate work. Native image/mark object creation still
occupies the PDF worker and existing image/placement limits remain unchanged.
The two-lease count is not a measured whole-app RSS budget; T05 remains open.

New generated window cases hold first overlay metadata, non-default serialized
page-unit validation, final assembly and post-overlay review metadata. They require
page 25 to render, close/shutdown to reap the child, no published/changed existing
destination and no sibling staging leftovers. Pure staging tests cover dropped
replies, an alias swapped in before publication, and successful publication.
Existing page-unit/overlay/review/alias/password/recovery and native Windows
hosted gates remain required. Current-source CI and replay are PENDING.

Experienced review must cover lease continuation, native handle ownership,
publication ordering, alias races, cancellation of every helper, TempPath teardown
and the intentionally unfinished image/page-export/native-dialog ownership paths.


## Sequential page-input and image preparation increment

Native replacement of page 1 from a fixture photo in a generated 50-page,
UserUnit=2 document reproduced a blank page 25 until target-unit metadata returned.
The native file/save dialogs used a private GTK portal, the separate output then
validated, the source stayed unchanged and the helper was reaped. See
progress/T03-image-stall-reproduction.json. This is separate from overlay export.

The page operation retains one admission lease across all inputs and phases.
The PDF worker first reduces target page geometry to scalar values. The helper
worker reads one immutable input snapshot, lazily validates replacement units,
and decodes/orients/resizes/encodes one image. A byte/pixel-only completion returns
to the PDF worker for native image-object construction and serialization. Those
native documents are destroyed there before returning raw bytes to the helper.
The helper finalizes UserUnit and continues with the next input using the same
lease. It never accumulates decoded RGBA for every selected image. PDF input
snapshots and generated PDF bytes accumulate as in the existing page workflow;
this is not a complete measured aggregate RSS bound (T05 remains open).

Existing PNG/JPEG compressed/decoded/resize limits and original JPEG preservation
are retained. Input snapshots use cancellable chunks and fallible reservation;
regular files are required and size changes during reading are refused. OS file
I/O and image-codec calls are not preempted inside a call; cancellation is checked
between chunks/phases. No filesystem-latency guarantee is inferred from qpdf's
process deadlines. Source paths stay attached for source/output alias checks.
Selected protected secondary sources still need T07's separate prompt queue.

All page operations, including no-image extraction, finish in the owned pool with
cancellable assembly, metadata/bookmark checks and private sibling staging.
Close cancels preparation/finalization and removes pending identity; late pixel
or staged replies are discarded. Only a live matching document owner can publish.
The existing detached page-export thread is removed. Standalone CLI and GUI use
the same image preparation/native construction/unit APIs; CLI coordinates them in
its own process before UI creation. No PDFium initialization/handle crosses into
the helper pool.

Five additional generated window cases hold target-unit inspection, image unit
rewrite, image assembly and no-image assembly; another cancels a mixed JPEG/PNG/PDF
merge and retries after reopening. They require reader page 25 while stalled,
normal shutdown/close cleanup, unchanged sources/existing destination, no staging
leftovers, and a validated 55-page retry output. Original native reproduction must
also be replayed on the built candidate. Full debug/release/Clippy, page/alias/unit/
password/recovery and native Windows gates remain required. Print preparation,
quick-print and native-dialog ownership/cancellation remain separate unfinished
T03 work. The established experienced lifecycle review gate still applies.

## Close admission and publication claim follow-up

Backend serialization alone does not prove cancellation when the UI accepts a
close: helper replies are polled before queued commands. A deterministic pure
worker regression reproduced that an admitted Close left a ready request valid
until owner cleanup (run 43523509748 on 1985833a). No actual output overwrite is
claimed from that data-level test; the ready-result gate remained open.

The proposed follow-up centralizes UI admission in Worker::try_send. Close uses
a short registry lock around nonblocking queue admission, then marks the ID closed
and cancels existing request tokens before returning success to the UI. Rejected
admission leaves the still-open document usable. Registration rejects a marked ID,
and its provisional lease is released. The owner removes the marker only after
processing Close behind preceding FIFO commands and removing its native state.
No registry lock is held while sending replies, joining workers, creating native
objects or publishing files.

Tokens transition atomically from running to cancelled or publishing. Publication
must claim the latter state after identity/document checks; a close that wins
first prevents it. A claim that wins before close acceptance is the commit point
and may finish; cancellation does not roll it back. Children use the existing
separate atomic cancellation flag, and cancelled queued work is not executed.
Closed IDs remain only until owner cleanup. Cancelled ready export replies report
a terminal failure even if they arrive before backend Close, clearing busy state.

This strengthens the earlier backend-order-only description. The follow-up's
hosted/runtime gates are PENDING. This dependent page/image implementation integrates the same admission and
publication policy; its cancelled ready replies also send a terminal error. Experienced review must assess
both orderings, delayed registration, lease release, marker reclamation and no
mutex held across work that needs a UI callback or native owner to complete.


## Owned print follow-up (unqualified draft)

Print preparation uses document identity until an atomic handoff claim. A separate
single-action byte-only actor owns dialog/submission latency and normal-shutdown
join. Closing a tab after handoff retains its immutable print action; shutdown
cancellation is immediate through the shared application stop flag. Actor Drop
sets that flag before joining. Native dialogs have byte limits and cancellation,
with no qpdf-style interaction deadline. Parent-owned private scratch outlives
child/group/job termination. Linux inherited-group nested failures deliberately
abort the host; Windows retains nested jobs without breakaway or relaxed limits.
See progress/T03-print-ownership-2026-11-14.md for the reproduced defect, exact
source, containment tradeoff, tests and pending qualification/review. This changes
the earlier unfinished-print description but does not close its review gate.


Printer discovery follow-up: Application owns its separate Jobs<(), Vec<String>>
service, whose Drop cancels and joins noninteractive discovery. Its registry is
independent of document closure and the print actor; the UI retains one pending
request. The main metadata pool keeps two leases; each UI print/discovery service
admits one action. This is a bounded operation count, not a whole-app RSS claim.
The native panel-open reproduction and eighteenth window case must pass on the
new head. Previous seventeen-case evidence is retained with its exact source.


## Accepted main reconciliation for lifecycle review (14 November)

This #10 review candidate now includes accepted main 68910141, retaining
qualified a87095b9 as a normal merge parent. Only changelog insertion conflicted;
both the lifecycle entry and all accepted maintenance entries are preserved.
Every prior lifecycle source blob is byte-identical; the sole src change from
main is the separately qualified test-only mark timing instrumentation. Cargo
manifest/lock, UI crate manifest and workspace UI bytes are unchanged.

New main supplies the declared Rust 1.92 native source checks, bounded mark
diagnostics and complete portable/MSI checks. Their presence does not qualify
this combined lifecycle source. Current-source hosted gates and original native
responsive/close replay must be repeated. Experienced lifecycle review remains
PENDING; no review, manual platform acceptance or production baking GO is inferred.


### Fresh main reconciliation: release fixture failure

Source 15983766 in run 43558979273 failed Linux release unit tests: 92 passed,
one failed, at the generated decryption helper's startup handshake. Debug tests,
both Clippy checks and the Linux native Rust 1.92 source check passed. Later
release/runtime gates were skipped and are not qualified. The retained first
failure receipt is [here](progress/T03-decryption-stall-first-failure.json).
The artifact contains minimum-compiler reports only; the failed release test's
assertion is retained from the source-bound job log.

The cause remains UNKNOWN. The old fixture consumed an early completion/error
then discarded it before asserting the handshake. The diagnostic follow-up
preserves that result and reports only a byte count or sanitized helper error.
All startup, stall, fallback and completion bounds and all existing assertions
remain unchanged. There is no production decryption/process supervision change
and no claim of a fixed timing failure. Fresh hosted checks remain required;
experienced lifecycle review is still required before merge.


### Strict registration diagnostics on the failed lifecycle candidate

Historical 79ae2894 passed Linux/policy, both actual native minimum compilers
and the exact Linux original overlay responsiveness replay. Its Windows
stable report has eighteen passes and one user-registration failure at the
final RegisteredApplications shared snapshot comparison. That cause remains
UNKNOWN, with exact failed artifact and suite log retained in the inherited
T19 registration diagnostic receipt. A passing independent run is not a fix.

This follow-up normally merges accepted main 414a040f, which contains qualified
strict, bounded, names/value-hashed registration diagnostics from PR #24. Only
changelog insertion conflicted; both entries remain. Every lifecycle source,
manifest/lock, UI and production registration byte remains unchanged except one
test-only error-chain formatting expression: the generated decryption fixture
now retains underlying causes rather than only the outer anyhow context.
Production bytes, every assertion and startup/stall/completion deadline remain
unchanged; neither retained failure has an identified production fix.

Fresh source-bound hosted gates remain PENDING on this combined source. Keep
draft for experienced lifecycle review. Passed #16/#17 and independent engine
#18 need no new qualification solely for this test-only diagnostic increment;
combine their affected gates when production/dependency source changes or review
requires it. Their existing manual/review/Fedora gates remain mandatory. No
release/version/tag, resource-cap, runtime pin or production baking change.

Local combined-source checks pass: fifteen suites, 127 tests (123 passed/four
existing platform skips), formatting and whitespace. No local Rust compilation.


### Qualified recovery lock fix integration

Normally merge accepted main 30767936, retaining the prior candidate. The exact
qualified main recovery guard/regression delta is the only Rust production
change; all other source, manifests/lock and UI bytes remain unchanged. Only
changelog insertion conflicted and both entries are retained. Main's focused
fix passed every gate on a68ebe6b: new duplicate-handle regression and original
handoff in both Linux profiles, plus Windows portable handoff and both native
minimum compilers. The original transient descriptors were not captured, so
attribution remains inferred; unrelated historical failures are not diagnosed.

Fresh combined-source hosted/native/minimum-compiler and affected runtime gates
remain PENDING. No new ownership/engine implementation is promoted by this
integration. Existing experienced lifecycle/engine review, actual Fedora source
build and manual/operator gates remain REQUIRED; keep draft. No version/tag/
release, resource-cap/private build route, runtime pin or production baking change.

Local recovery-integration checks pass: fifteen suites, 127 tests: 123 pass/four existing platform skips.
Formatting/whitespace pass; no local Rust/C++ compilation. Hosted gates pending.
