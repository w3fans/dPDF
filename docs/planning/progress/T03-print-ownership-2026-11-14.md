# T03 native print shutdown ownership

Status: REPRODUCED; implementation and experienced lifecycle review pending.

Generated 50-page PDF, qualified overlay source 10b443f4 / tested merge
7b074362e57dc04bc84477f59149769f56a6ede5, executable SHA-256
ad5711356b28e8278629a79b7fcf59959b9d0c8b5d47f72427dfc92c1aa2dd26.
Actual Ctrl+P opened the native GTK print dialog on a private X11 display with
GTK_PRINT_BACKENDS=file. Sent the standard WM_DELETE_WINDOW protocol to the main
window. The application exited normally with code 0; its native print child and
visible dialog were still alive afterwards. No print action was submitted and
source SHA-256 stayed unchanged. Fixture subreaper adopted/closed/waited for the
orphan before stopping the private display. See T03-print-shutdown-reproduction.json.
Initial sandbox display setup failed before application launch; reran with the
approved private-display execution. That setup failure is not a product defect.

Source: PrintNative, PrintReviewed and Quick Print launch detached threads; native
printing writes piped stdin synchronously then calls wait_with_output without
owned shutdown cancellation. Reader responsiveness alone does not prove cleanup.
The app's print_pending flag limits UI admission but does not own the child/thread.

Proposed boundary: retain one active print action and join its owned worker during
normal shutdown. Review preparation stays cancellable and tied to document identity
until the PDF owner claims handoff. Once a dialog/submission is handed an immutable
snapshot it can outlive that tab; app shutdown must cancel/reap it. Do not impose a
qpdf-style deadline on time spent in a user dialog. Bound its output and pipe writes,
retain source/alias/geometry checks and report ambiguous submission without retry.

Linux containment must also cover helpers launched by the native child: current
qpdf supervision creates an independent process group, so killing only the outer
native child's group does not prove descendant cleanup. A reviewed inherited-group
mode or equivalent containment is needed, with normal nested-helper success never
killing the print host. Windows must retain job ownership before resume and nested
containment under runner limits. Tests must include a stalled nested helper, normal
user cancellation, tab close after handoff, app shutdown, output flood/blocked input,
review preparation before handoff and exactly one submission. Existing native GTK,
Windows/print-plan, password, review, pages and alias gates remain required.

This receipt is a scoped reproduction and design, not print lifecycle acceptance.
No release/version/tag or production baking. Parent #10 and page/image #16 remain
under their existing qualification/review gates.


## Implementation draft

The same shutdown defect reproduced on combined image head 948264e3 (tested
merge a098af80, executable 66400a88); see the combined reproduction JSON.
Isolated /tmp/dpdf-print-ownership starts at that qualified parent. No local Rust
compilation has run. Current source and runtime qualification are PENDING.

A separate owned print actor admits one immutable action through completion. The
existing document-bound helper pool prepares review output; its reply must claim
handoff atomically against accepted Close. Cancelled/closed preparation reports
PrintFinished. A handed-off dialog or Quick Print action retains its own immutable
snapshot across tab closure. Its process I/O observes the application's shutdown
flag immediately, and actor Drop signals that same flag before its Jobs field
joins. Replies cannot block that worker; the PDF owner forwards them to the UI.
The three detached Print/PrintNative/PrintReviewed threads are removed.

Native child input/output supervision reuses bounded pipe handling with an
explicit no-deadline dialog policy, while noninteractive helpers keep deadlines.
Normal app shutdown cancels/kills/reaps the native host. A parent-owned temporary
directory is passed as the child's TMPDIR/TMP/TEMP and removed after supervision,
including abort paths. Quick Print receives cancellation and retains ambiguous
submission wording, exactly one attempt and no automatic resubmission.

Linux's internal native CLI validates that it leads its externally owned group,
then nested supervised helpers explicitly join that group. Successful nested work
never kills its host. Deadline/flood/I/O failure aborts that entire host group;
the external reader owner reaps the host and cleans its private scratch. This
fatal nested-error policy requires experienced review, including whether any
fallback or submission ambiguity needs more explicit reporting. It is never
enabled in the reader or diagnostic native CLI. Windows keeps suspended-before-
job-assignment, no breakaway and existing runner limits. Microsoft's
[nested jobs](https://learn.microsoft.com/en-us/windows/win32/procthread/nested-jobs)
and [termination](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-terminatejobobject)
document descendant containment; source/runtime tests must demonstrate it here.

New pure/generated tests cover single admission, tab-close survival after handoff,
owner stop and destructor-before-join ordering, no dialog deadline, blocked input,
output floods, successful nested helper host survival, Linux nested failure group
abort, Windows nested job cancellation, and ambiguous CUPS cancellation without
retry. No physical queue or printer is used by these tests.

Required next: approved hosted full checks, original native normal-exit replay,
tab-close/user-cancel and held nested-helper replay, source/scratch/child cleanup,
reviewed-print preparation responsiveness, then experienced lifecycle review.
Existing fifteen helper windows and all native geometry/interop/print-plan/alias
checks remain applicable. Both parent drafts stay unmerged; no releases or pins.
