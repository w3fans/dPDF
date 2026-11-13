# T03 — Helper inventory and controlled-stall reproduction

Status: IN_PROGRESS; reproduction prepared, Rust execution pending.
Baseline: 268b234e4336789922a6b15dd1236963e131d707.
Local branch: fix/public-release-adapter-tests (preserving combined candidate).
Focused public branch: fix/bounded-qpdf-helpers, draft PR
https://github.com/w3fans/dPDF/pull/6, candidate
c4e869db8158c835d5d424030ab3321ad69fabf7. Remote encrypted.rs blob matches local.
T01 public unit baseline passes; its authority/review gate remains pending separately.

Initial helper inventory from source:

| Path | Call context / existing bounds | Remaining risk to reproduce |
| --- | --- | --- |
| encrypted::decrypt_with | working_bytes preparation on sole PDF worker; password via stdin, stderr discarded, stdout 256 MiB cap | No deadline on stdin/write, stdout/read or child wait |
| assembly::inspect_metadata/inspect_snapshot | LoadReview on PDF worker and other snapshot users; stdout 64 MiB cap | No deadline; metadata parse and other resource limits need measurement |
| review::checked | Preparation may execute on PDF worker; prepared save on export/print thread | Blocking output captures unbounded output; no deadline |
| assembly::page_count/export | Page export runs separate thread after working snapshot preparation | Blocking output/version/check helpers lack deadline/output cap |
| pdf_units image-unit rewrite | qpdf update/output plus metadata checks | Blocking output helper lacks deadline/output cap |
| native_print/direct_pdf subset prep | Native print preparation path | Blocking qpdf status/page-count; printer discovery also needs inventory. User-owned print dialog must retain separate policy |

Added Unix characterization test to encrypted.rs: helper reads password through
stdin, signals readiness and waits for a release file. Test observes the operation
pending for 250ms after readiness, releases helper, joins and verifies unchanged
snapshot bytes. Helper loop has a finite fallback lifetime; assertions run after
release. No production helper fix or engine/thread ownership change yet.

Checks: exact shell fixture extracted from the Rust string passed locally using
generated input (ready, controlled stall, release, unchanged output). cargo fmt
--check and git diff --check PASS. Rust test NOT RUN locally; its passing assertion
would characterize existing behavior, not prove timeout/cancellation handling.
No claim of a reproduced app freeze or bounded process supervision yet.

Next: poll exact PR candidate CI, obtain Rust reproduction evidence, then implement
finite helper deadline/output/cancellation policy and replace baseline stall
expectation with final timeout/cancel regression. Keep passwords out of diagnostics,
kill/reap failures and output floods bounded, preserve immutable snapshots/atomic
publication, and keep engine handles on their owner. Add warning/flood/retry tests.
Do not merge this characterization as completion of T03.
Rollback: revert the focused new test only; retain user data, all earlier task work
and historical/PDF validation receipts.

Public CI run 43464481255 confirmed IN_PROGRESS for c4e869db8158c835d5d424030ab3321ad69fabf7. Poll that run next; do not restart while live.

CI baseline repair: Linux logs confirmed the same Rust 1.100 needless_range_loop failure in block_probe.rs. Included the identical iterator-only correction on PR #6, candidate 2d8cc157951c2d0c604319073fd396afb8fce7af. Earlier run remains FAIL, not retried unchanged. Windows PR #5 prior candidate passed its hosted lane; that does not qualify the new commit.

## Linux characterization suite result — 2026-11-13

Run 43465832464, candidate 2d8cc157951c2d0c604319073fd396afb8fce7af:
Linux job 132024588061 passed format, strict Clippy in both configurations and
`cargo test --locked` debug unit suite. The new Unix characterization test is
unignored and included by this Linux command, supporting the controlled-stall
baseline. Exact per-test output still awaits completed-job logs/artifact. Release
unit tests remain live; Windows hosted checks passed (Unix fixture excluded on
Windows). This is not a helper-timeout or responsiveness PASS.

Next implementation: bounded helper process supervision, starting with private
password/stdin and bounded unlocked-PDF stdout. Account for descendants retaining
pipes, concurrent bounded output draining, kill/reap and deadline/cancel; do not
fix only child.wait while leaving read_to_end indefinitely blocked. Keep platform
implementation/qualification separate and preserve required Windows behavior.

## Linux supervisor implementation — 2026-11-13

Candidate daeb68dd41841ebdd6180a731adf1ed2ce17eb7b on draft PR #6 adds
src/helper_process.rs and Linux-only module registration plus decryption wiring.
All three remote source blob hashes match local files. New run 43468230896 is
confirmed IN_PROGRESS; baseline run 43465832464 belongs to the earlier source.
The currently running remote wrapper build also predates this supervisor; its
result cannot qualify this new implementation.

Supervisor sets a separate child process group, nonblocking Linux pipe flags,
bounded stdout/stderr reads per iteration, byte caps, deadline and optional atomic
cancel checks. RAII cleanup kills only the owned group/child and waits for the
direct child; no blocking reader/writer threads are detached. This handles an
exited shell whose descendant retains a pipe. Narrow Linux fcntl/kill FFI is
isolated and documented; no new package or engine lifecycle/handle changes.

Linux decryption uses 30 seconds, 256 MiB stdout, and discarded stderr. Password
input stays in memory/stdin; original source path remains a protected immutable
snapshot staged privately as before. Windows/other-platform decryption remains
unchanged and needs a separately qualified supervisor. App-level cancellation
is not wired yet (the primitive exposes it, decryption passes None).

Prepared tests cover success/exit-3 warning, inherited pipe holder, blocked stdin,
stdout/stderr floods, pre-start/live cancellation and discarded private diagnostics.
Existing password channel/error test remains. The earlier 250ms characterization
still tests delayed-success handling; the supervisor deadline tests exercise
actual termination separately.

Checks: cargo fmt --check and git diff --check PASS. New Rust compilation/tests
PENDING run 43468230896. No timeout, cancellation, performance or cleanup PASS
claimed before execution. Full app closure/shutdown responsiveness, other helper
callers, Windows tree cleanup and reader/export/print corpora remain unfinished.
Next: inspect exact run results; fix failures, then extend supervised policy to
metadata/review/assembly/print preparation with caller-specific output limits.
Never apply this noninteractive deadline to the user's native print dialog.

## Metadata caller migration prepared — 2026-11-13

Local src/assembly.rs now routes Linux inspect_metadata through the supervisor:
30-second deadline, existing 64 MiB stdout cap, stderr discarded, nonzero/warning
exit rejected before JSON decoding. Non-Linux path is retained. Shared callers
include LoadReview/page-unit metadata on the PDF worker; no handles moved and no
async responsiveness claim. Added injected-helper tests for stall, invalid JSON,
warning exit with private diagnostics, flood, successful retry and unchanged input.
Formatting/diff checks pass. Compilation/execution is PENDING; this change is not
yet in public PR #6. Keep run 43468230896 for the prior supervisor candidate
daeb68dd41841ebdd6180a731adf1ed2ce17eb7b alive through full corpus completion
before pushing the next source change, so its expensive release tests are not
unnecessarily cancelled. The debug suite already passed on that prior candidate.

Next: inspect terminal current run and retain per-suite logs, then publish this
metadata migration and validate it. Do not transfer the prior candidate's PASS to
this newer assembly.rs. Remaining app-level cancellation and worker responsiveness,
Windows process supervision and other helper call sites remain explicitly open.

## Review-export caller migration prepared — 2026-11-13

Local src/review.rs now uses the same Linux supervisor for annotation-write and
output-validation qpdf calls: 60-second deadline, 64 KiB stdout, discarded stderr,
warning/nonzero refusal. Added helper tests for timeout, warning diagnostics, flood
and successful retry. Non-Linux implementation remains unchanged. Formatting and
diff checks PASS; compilation/execution PENDING. This is local prepared work, not
in the active PR #6 candidate. Metadata/review changes will be published together
after current run 43468230896 reaches a terminal outcome, retaining expensive
prior-candidate corpus evidence. No source/destination publication path changed.
T01 is now merged; local working base is 1cf522aeccc1794e5cbad4af42a7a02fd1b05c75
on work/implementation-plan with earlier edits preserved byte-for-byte.

## Broader Linux caller migration published — 2026-11-13

Run 43468230896 completed all jobs SUCCESS for supervisor candidate
daeb68dd41841ebdd6180a731adf1ed2ce17eb7b. Retain this exact-source decryption/
supervisor result; it does not qualify subsequent caller migrations.

Expanded PR #6 to candidate f21a47459d5b9513056c851d53177bb9024a7fc3, with a
normal merge parent for main 1cf522aeccc1794e5cbad4af42a7a02fd1b05c75. Preserved
merged tooling/changelog and the previous helper history without force push.
Remote readback hashes for all five migrated caller files match local source.

Added shared scalar/status run_qpdf (Linux: 60s, stdout/stderr64KiB, fatal warnings),
then migrated page count/version, assembly/check, bookmark restoration, image-page
unit rewrite and compact-print subset preparation. Metadata keeps its separate
30s/64MiB policy. Review export/check uses60s/64KiB stdout/private stderr discarded.
The noninteractive lpstat queue check is bounded at5s; failure retains its
existing renderer fallback. No user print-dialog deadline or submission retry.
Non-Linux run_qpdf preserves blocking execution and still needs supervision.

Added page-count tests for warning, flood, malformed/zero/oversized count, retry
and unchanged input, plus the previously prepared metadata/review helper tests.
Local cargo fmt --check and git diff --check PASS. New implementation tests/corpora
are PENDING on the expanded candidate; no source/destination atomicity or native
CUPS/platform qualification inferred before checks. App cancellation/worker
responsiveness, Windows handling and other discovered helpers remain open.

## Linux CUPS boundaries and Windows draft — 2026-11-13

Expanded Linux candidate f21a47459d5b9513056c851d53177bb9024a7fc3 completed
all jobs SUCCESS in run 43472050969. This qualifies its named automated suites,
not app-level cancellation/responsiveness or native desktop/physical acceptance.

PR #6 candidate 3c6b21730fc0ddda248cd0fa1fa72219d9d47f6d reconciles merged
T04 through a normal two-parent merge and bounds platform CUPS helpers: discovery
5s, submission30s, stdout/stderr64KiB each, displayed diagnostics2000chars.
Submission errors explicitly say the job may already be queued and require queue
inspection before another submission; no automatic retry. Generated tests compare
finite old output blocking with a150ms supervised stall, assert exactly one
submission attempt, exercise both output floods, discovery timeout and later
successful explicit retry. The prior test-boundary ETXTBSY correction is retained.
GitHub branch update initially timed out; ref/file readback proved it unchanged,
then the lease-checked retry succeeded. Source blob37711590b8ba739fabe9ee89e9c0f7a70590e6a3
matches the local CUPS candidate. This is not the later Windows platform.rs.

Run43476228011 Windows job132060632840 FAIL: LLVM out-of-memory building the
application under the existing2GiB job bound. Unit tests63pass, Clippy/formatPASS;
application corporaSKIP. Downloaded diagnostic artifact14063307163 to
/tmp/dpdf-t03-cups-windows-failure.zip and verified SHA256
b61b5069d4f40f87229ea1b136eeace9a9afa28487205fb1cfc8c38403660ace.
Preserve failure; no memory-limit increase or redundant build launched. Linux
full PDF/UI job was still live at last observation; inspect terminal outcome.

Prepared Windows candidate4e6b853bcc2ece4022f38a1f692f39ca42589058, draft
PR #8 https://github.com/w3fans/dPDF/pull/8, based on PR #6 (neither merged).
Uses an unnamed noninheritable Job Object, CREATE_SUSPENDED, assignment before
ToolHelp/ResumeThread, KILL_ON_JOB_CLOSE and explicit termination/root wait.
Bounded readers and borrowed-input writer live in a thread scope; process guard
lives inside its closure, so cleanup precedes scope joins even on error/unwind.
No detached pipe tasks or PDFium handles. Windows qpdf callers use the existing
Linux policies; silent Quick Print60s/64KiB streams reports uncertain submission.
The native user-owned print-dialog policy remains separate.

Native test subprocess fixtures cover finite legacy stall, warning/private stdin,
blocked input, floods, prestart/live cancellation, retry and root-exit/pipe-holding
descendant termination. Tests/compilation are PENDING native CI, not source-review
PASS. All seven uploaded source Git blob hashes match local source. Formatting
and diff checksPASS locally. Cargo.lock/pins unchanged; existing windows-sys API
features enabled. Real offline Cargo metadata/tree plus collector regenerated
Linux inventory348packages/646notice references; checkerPASS and all notice
bytes already existed. No local Rust compilation occurred.

Windows cross-target notice collection on Linux refused mismatched host-build
packages dlib/yeslogic-fontconfig-sys. Do not fake the retained inventory fingerprint:
native Windows collector must supply the exact graph. Windows inventory checker
currently FAILS configuration as expected. PR #8 first CI candidate must remain
draft until native-generated inventory and all relevant checks pass. CI only
targets main; initial stacked base did not start a workflow, so retargeted main
while retaining PR #6 ancestry. App cancellation, tab close/shutdown, reader
responsiveness and broader Windows qualification remain open. Next inspect both
existing runs and PR #8 native logs/inventory; investigate repeated build OOM
without relaxing the established resource bound.

Platform API sources consulted: Microsoft Job Objects, Nested Jobs,
AssignProcessToJobObject, Process Creation Flags, ToolHelp thread traversal,
ResumeThread and JOBOBJECT_EXTENDED_LIMIT_INFORMATION documentation:
https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects
https://learn.microsoft.com/en-us/windows/win32/procthread/nested-jobs
https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-assignprocesstojobobject
https://learn.microsoft.com/en-us/windows/win32/toolhelp/traversing-the-thread-list
https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-resumethread
This supervises cooperative installed helper trees; it is not a hostile-code sandbox.

Follow-up05154d3a1482b1279c6c90fa86ba917c687b2de9 adds exact cap/cap+1
cases at0/1/4096/8192/8193bytes. Native module blob
50e480d915b7773bdc4cedad0cdc96881c206f0f matches local source. Retargeting
alone started no CI (default event filter); this meaningful test follow-up now
runs as43478128715, policyPASS, Windows and Linux checks confirmed live.
Run43476228011 is terminal: full Linux/PDF/private-Xvfb and policySUCCESS,
Windows buildOOMFAIL as above. No old process or failed job restarted.

Run43478128715 Linux job132067288735 terminatedFAIL at retained Windows notice
configuration, as explicitly pending. Debug84testsPASS, release84testsPASS and
release build completed before that gate. Do not skip or relabel the gate; obtain
native-generated Windows inventory. Windows job132067289022 remains live.

## Native Windows supervisor passed; real notices imported — 2026-11-13

Run43478128715 Windows job132067289022 completedSUCCESS for05154d3a1482b1279c6c90fa86ba917c687b2de9.
Build,68unit tests, Clippy/format and all hosted service corpora passed. One ignored
fixture_process is invoked explicitly by supervisor tests, not skipped acceptance.
All five native supervisor tests passed, including descendant termination and
exact-cap boundaries. Executable SHA2568f5861e74bea4865c743f98519e72c604704c0b0b1982fb47f13186c9b2a25af.
This does not prove the earlier compiler OOM deterministically fixed.

Downloaded artifact14064048896 to /tmp/dpdf-t03-windows-supervisor-pass.zip,
verified SHA25632efae7f0079ac9d7e3effc5258a9170bba2a10aeb1ba2d6bea62aa3c261e16a,
safely extracted /tmp/dpdf-t03-windows-supervisor-pass. Its notice summary binds
source05154d3; manifest/lock fingerprints match. Initial inspection used the root
cwd for a relative inventory path and failed read-only; corrected cwd. Initial
import expected zero additional texts and refused before writing; inspected the
actual extra before importing. No inventory check bypassed.

Native target graph254packages/483notice references removes prior conservative
Linux-build-host dlib/libc/libloading0.8.9/pkg-config/yeslogic-fontconfig-sys entries.
Only shared windows-sys features and Winit license bytes differ among retained
packages. Windows checkout CRLF Winit LICENSE is byte-equivalent after CRLF->LF
to tracked vendored LICENSE. Imported its exact11523bytes as hash-addressed text
de556eb1415648cc1bba68d1e602d8a54e3e5b3592770df34d863449e8f0303c; no license-choice
change. Both source notice checks nowPASS (Linux348/646, Windows254/483), and
remote new inventory/license Git blob hashes match local files.

Published normal merge candidate daa2914af43dfbe57473251a8f67fe4293f0b40e,
tree22fd6c9bd42d81374d7ec0f43f315ecffd99bfc5, parents05154d3 and merged main87560c3.
Retains exact prior helper source while incorporating runtime/CI/MSI main changes;
T04 history fix PR #9 is separate. Fresh combined-source CI PENDING. Keep PR #8
draft until all gates pass. Older public PR #6 remains unmerged with its OOM
receipt; do not merge its failing candidate independently. App-level cancellation,
closure/shutdown and sole-worker responsiveness remain next T03 work.

Fresh combined-source run43480685212 confirmed live for daa2914; policyPASS,
Windows native and Linux build checks live. Preserve its handle to completion;
no repeated build or old-job restart. Both local notice checks and4collector
contract testsPASS after exact native import.

## Reader-worker stall reproduced and async metadata candidate prepared — 2026-11-13

Previous combined helper candidate daa2914 passed every job in43480685212.
Reconciled merged Review shortcut fix a80603e through normal merge4f55b5d5ae410ff1671afc2e9bde87a215b209f6,
tree2a15ae68f33ba2ead8d07d2eb5ef9d5d74d35d7b; run43483197569 is confirmed live.
Windows/policy completedSUCCESS; Linux full PDF/UI remains live at last observation.
These gates do not qualify the later uncommitted async implementation.

Reproduced remaining worker blocking using verified history-fix runtime
source df8a27b4abe4115114dbdde9f6950db2b0699701, binary SHA256
da8d01e41fd9b1af87f955704df2251d89236403e4a813074a5ed36731b5283f.
Generated50-page PDF and finite controlled metadata helper on private authenticated
X11/Fedora45. While helper signalled ready and waited, jumping to page25 left
reader/thumbnail tiles blank; after explicit release both rendered. Captures,
identity and script in /tmp/dpdf-t03-reader-stall and .py. Source hash unchanged,
normal generated app/display process exited. Relevant original PDF worker path is
unchanged by toolbar/shortcut fixes; this is not a pass of the new supervisor or
async code. Never infer an async reader from finite helper deadlines alone.

Prepared LoadReview async increment: one byte-only helper worker, two outstanding
leases including results, immutable snapshot/private password handoff, bounded
metadata/decrypt/clean qpdf phases, cancellable source-relative clean-byte reads.
Native geometry and document installation stay on the existing PDF worker.
Document/pool generation rejects stale internal results; per-tab generation also
travels in UI unit/review events. Close/retry cancels prior requests. Worker Drop
cancels registered metadata work, unblocks response sends and joins normal worker/
helper teardown. See HELPER-WORKER-LIFECYCLE.md for concrete technical review gate.
No new PDFium instance/handle transfer, dependency or version change.

Added pure admission/identity/cancel/join/panic tests, cancelled-save source/output
preservation, plus generated window close/retry/shutdown fixture and required Linux
CI suite. Existing other export/print preparation still needs migration; do not
mark full T03 complete. Local format/diff, Python syntax and CI contracts14cases
(10pass/4existing skips), both retained notice checksPASS. New Rust compilation,
window execution, lifecycle review and full corpora PENDING; candidate not merged.

## Core supervision merged; async candidate entered CI — 2026-11-13

Run43483197569 completed every jobSUCCESS for4f55b5d5ae410ff1671afc2e9bde87a215b209f6.
Marked PR #8 ready and squash-merged as30fafd7aba05a0be4765d621c2c0fad99e1a86da.
Closed superseded PR #6 without merging its failing candidate. Previous OOM and
skipped native corpora remain failed/skipped history. T03 overall staysIN_PROGRESS.

Published async prototype6ef1e643af70e5cef9eebbaf0963243a0ce23fad,
treed1214a35a018191ff788e802ffa5ae27d1b92481 on draft PR #10:
https://github.com/w3fans/dPDF/pull/10. All11source blob hashes matched local.
Per-tab request generation now travels in metadata/review UI events in addition
to internal document/pool identity. No late response can satisfy a newer request.
Lifecycle design at docs/planning/HELPER-WORKER-LIFECYCLE.md explicitly requires
experienced review before merge; do not promote the draft from code inspection.

First run43485260680: policyPASS; LinuxFAIL strict Clippy (deprecated atomic
fetch_update on new Rust, plus unused review inspect wrapper). Windows job
132091888742 FAIL: application build LLVM out-of-memory under unchanged2GiB,
Clippy same two source warnings; formatPASS and72native unit testsPASS, including
all3new background pool tests and cancelled-save preservation. One subprocess
fixture deliberately ignored at outer level is explicitly invoked by tests.
Application corporaSKIP because build failed. GUI helper responsiveness/retry/
closure/shutdown NOT RUN yet; do not infer them from pure worker tests.

Verified Windows artifact14066968052 archive SHA256
5d69371e8eafb368296527e045c4faa98657871c3739b90a2a342d14f3c3e562 at
/tmp/dpdf-t03-async-first-windows.zip. Recurrent compiler OOM remains unresolved;
a later pass alone must not be called a deterministic memory fix. Standard Cargo
profile/codegen-unit and Windows Job Object memory-query docs were read for a
possible measured investigation, but no codegen/resource/profile change made.

Corrected with explicit compare/exchange admission (keeps declared Rust1.92
compatibility instead of adopting newer try_update API), removed dead wrapper,
and strengthened generated-helper cleanup to require PID/start-time identity.
Published9519b5cd6bc5a88b8f0f665c752894884ed0879e,
tree5fda1243c76205ee933acda314fdddf217a939d0; three readback hashes match local.
Local format/diff/syntax and retained noticesPASS; new native/GUI/full testsPENDING.
Prior run was authoritatively terminal before pushing, so it was not cancelled.

Local branch/index advanced to merged30fafd7 after saving tracked diff and all59
edited/untracked file hashes at /tmp/dpdf-before-main30fafd.patch/.json. All59
file identities verified unchanged before/after; no staged work or source loss.

Corrected async candidate9519b5c is in confirmed live run43486282919. PolicyPASS;
Linux passed Clippy/default+Windows metadata check and debug tests, progressing
through release tests/build. Windows native job132095498710 live. Resume these
exact run/job handles; do not launch a duplicate build. No GUI helper-suite pass
or lifecycle GO has yet been recorded.

Remaining worker-helper inventory after this increment: overlay::prepare still
queries page units synchronously; image_pages::prepare queries replacement units
and runs qpdf image-unit rewriting; some early print/export paths can still call
OpenDocument::working_bytes before async metadata has populated decrypted bytes.
Detached export/print operations still need operation cancellation and shutdown
ownership. User-owned native print dialogs must retain separate policy. These
are concrete next T03 paths; the new LoadReview pool is not full task completion.

Current run43486282919 advanced: Linux Clippy/default+Windows metadata, debug/
release units, release build and retained noticesPASS; full PDF/private-Xvfb
suites running. Windows native job still live. Experienced lifecycle review was
requested asynchronously from the owner, with design/PR links and plan rule7 as
the reason. No approval received; keep PR #10 draft. This review gate does not
block further validation, OOM investigation or independent ready plan work.

## Async metadata validation and visible replay passed — 2026-11-13

Corrected9519b5cd6bc5a88b8f0f665c752894884ed0879e completed every jobSUCCESS
in run43486282919. Linux new helper suite passed distant-page rendering while
metadata remained stalled, close/reopen with stale-result rejection, retry and
normal shutdown with child reaped/source unchanged (0.39/0.22/0.20s in that lane).
Native Windows hosted job132095498710 passed; retain earlier OOM separately.
Technical lifecycle review still PENDING; PR #10 remains draft/unmerged.

Downloaded artifact14068251548, archive SHA256
1ca4d89689722566e1ef85fff87adac515b7b6aeb63ff0da5f628a0b2f68e0f0,
verified/safely extracted at /tmp/dpdf-t03-async-runtime. Report actual tested
merge sourcec9153a54197ac8725c30b662f723378e9323672b; executable SHA256
797319200d5341956bb4370e2ccf0e7a9a69a59dc2bea3a22737eb402c87555d,
PDFium libraryf728930966f503652b92acc89b9374a2eeca00ce42e26dccd3e4b5c5161b2d64.
Replayed original native XTest page25 controlled-stall sequence in new private
X11 environment. The page now renders before helper release. Viewport948x640+216+132
is pixel-identical to baseline's released frame; new stalled/released viewport
also0changed pixels. Source hash unchanged and processes exited. Hash/metric
receipt T03-reader-responsive-replay.json and fixtures /tmp/dpdf-t03-reader-responsive.
This scoped X11 evidence does not qualify every helper path or native platform.
