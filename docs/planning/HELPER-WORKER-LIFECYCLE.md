# Metadata helper worker and normal shutdown — review draft

Status: implementation candidate; compilation, lifecycle review and exact-source
acceptance pending. This does not qualify all helper callers or production baking.

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
thread. Other preparation paths still using `working_bytes`/metadata on the PDF
worker require subsequent migration; this is the `LoadReview` increment.

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
