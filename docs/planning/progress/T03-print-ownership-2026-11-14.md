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


## First hosted result and test correction

Head bdfdaf9e / tree f0ab685a, run 43529700684: policy passed; Linux passed
both strict Clippy modes but failed debug tests (97 passed, 1 failed, 1 intentional
subprocess fixture ignored). New actor/supervisor tests passed. The new SIGKILL
assertion was accidentally placed in the existing private-diagnostics test, whose
fixture deliberately exits 1. Moved that assertion to the intended owned nested
helper-abort test; no product policy or old fixture expectation weakened. Current
corrected source must repeat all gates before any qualification is claimed.


Added two source-bound real-window reviewed-print cases before qualification:
initial review loads normally, then an armed fake qpdf holds reviewed output
metadata while page 25 must render. One closes the tab and requires print busy
state to clear before handoff; the other quits normally and requires cleanup.
The generated model contains a real note and the fixture permits only GTK's file
backend. Held preparation prevents native handoff; no submission occurs. This
extends the existing fifteen helper windows to seventeen for the current source.


## Qualified immutable print increment

Head affa712c2c8ff0c35c5ec98ce32f0602db4fcd06, tree
940dd3dac1f845d6936aae1bfcee45e7378a8ef0, passed ALL jobs in run 43530445402
(Linux 132246572773, Windows 132246572584, policy 132246572779), including all
seventeen helper windows and new actor/process tests in debug and release.
Linux artifact 14087985393 ZIP SHA-256
86f832ba4752b6b7c0167b642972477550374027dc630295c54fb666b3661f91;
tested merge a2dd9cbde822744baee36b379d3adc3c7a21ae7c, executable SHA-256
0f2231fb14922ff16351f44532cec948b3828f644c695e61c75e91808a2ce6d1.
Windows artifact 14087592207 ZIP SHA-256
2d50582a974a052ac5f59d935d057a4c5298f634ac0cf49321b9b0113f0565c1;
source affa712c, executable SHA-256
51e8ca51bc2009f0a387921a32cd626adeddadaec9256dba673d69d2e175e9ac.
Both native notice inventory objects match Linux 349 / Windows 255 components;
all 648 / 485 notice references verify.

Three original native replays on that exact artifact passed. Standard main-window
close now exits 0 with the native child reaped. Closing the tab after handoff keeps
its dialog alive; user cancellation reaps it while the reader remains alive.
Actual-size preflight's held qpdf joins the native host group and is no longer live
after normal app exit. All three leave private scratch empty and source unchanged,
with GTK file backend and no submission. The fixture subreaper reaped the adopted
nested descendant; this does not prove the GUI directly waits on grandchildren.
See the three T03-print-owned-*-replay.json receipts. Experienced review must
assess that scope and the fatal nested-error policy. These passes do not close
printer discovery ownership, physical/native desktop or the review gate.


## Printer discovery follow-up (current source UNQUALIFIED)

Qualified affa712c native Print options opening auto-starts discovery. With a
fixture lpstat held, standard main-window close exited 0 in 0.012 seconds while
its child remained alive. Source unchanged; fixture killed/reaped the orphan.
See T03-printer-discovery-shutdown-reproduction.json. An initial fixture tried to
click the disabled Refresh button after discovery had already started; corrected
that setup without treating it as a product failure.

Application now owns a separate bounded Jobs<(), Vec<String>> service for printer
names, with its own cancellation registry. The existing loading flag permits one
pending UI request; replies are polled exactly as before. Source/tab lifetime is
irrelevant to this service. Normal Application field destruction cancels/kills/
reaps/joins it; no detached sender thread survives main exit. Discovery keeps its
five-second deadline, existing output limits and Windows unsupported behavior.
No PDFium handle enters it and it cannot stall the native dialog's separate actor.

Added an eighteenth real-window helper case: completed initial review, start held
lpstat discovery, render page 25 while the loading flag remains set, then quit
normally and require the child gone/reaped. The original native panel-open/close
reproduction must also be replayed on the new artifact. Earlier affa712c passes
qualify only its seventeen-case immutable-print scope; the discovery follow-up
requires new full gates and experienced review. No release or production baking.


First discovery head aea1df69 / tree 9689a881 failed Linux compilation in run
43532394665: the existing focus diagnostic inherited mpsc from Application's
parent import, which was removed with the obsolete printer channel. Added an
explicit std::sync::mpsc import to the smoke module. No product behavior or gate
weakened; corrected source requires the full rerun.
