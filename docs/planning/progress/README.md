# Implementation progress index

Current local base: e7bec799b0d5ff30175fa28cd2ff743d9b9b4a72, merged PR #15.
Focused text-history fix #12 is included.
This is a development worktree with preserved uncommitted documentation and
async helper work, not a release candidate. Refer to task cards for full scope;
partial increments below do not replace their exit criteria.

| Task | State | Evidence / next action |
| --- | --- | --- |
| T01 | DONE for public portability | Public unit split merged #5; installed authority integration remains separately pending |
| T02 | DONE for reconciliation/publication | T02-documentation-2026-11-13.md; canonical documentation and plan/evidence merged in PR #15; later receipts remain pending |
| T03 | REVIEW for PDF/print helper increment | Drafts #10/#16/#17 retain dependency order and experienced lifecycle gate. Functional e71a6726 passed all jobs, eighteen helper windows and exact-source native print/discovery replays; manual platform/operator evidence remains separate. See T03-print-ownership-2026-11-14.md |
| T04 | DONE for implementation/generated checks | Shared selectors and history fixes merged #4/#9/#12/#13; explicit focus rules and three-scale native X11 replays. See T04-exit-2026-11-13.md; actual Wayland/Windows visual/input acceptance remains T06 |
| T05 | PENDING T03 | Generated headless baseline exists; full performance baseline and high-zoom tiles not implemented |
| T06 | PENDING T04/T05 | Exact-source real Wayland/Windows input/clipboard acceptance required |
| T07 | PENDING T03 | Protected secondary inputs and metadata/refusal compatibility |
| T08 | PENDING T03 | Native Windows driver/spool measurements and qualified optimization |
| T09 | PENDING T08 | Physical Fedora/Windows printers and measured ruler output |
| T10 | PENDING T06/T07/T09 | Mark/recovery/interop and physical sizing acceptance |
| T11 | PENDING T06/T10 | Review interop/native and complete Scenario E |
| T12 | REVIEW | SECURE-REGION-CONTRACT.md requires experienced PDF review; no GO received |
| T13–T18 | PENDING dependency chain | Removal proof, backend, region UI, templates, modes and independent security/product qualification remain open; production baking stays disabled |
| T19 | IN_PROGRESS | Parser advisory patch merged #14 with full native/geometry/notice/UI gates; dependency inventory, package checks and bounded compiler investigation continue; real upgrade/default/data acceptance remains separate |
| T20 | PENDING milestone gates | No owner A–E or stable milestone approval inferred |
| T21 | PENDING T12 | Digital-seal interface planning remains open |
| T22 | OPTIONAL backlog | Optional enhancements excluded from the baseline plan |

## Current source and external work

PR #14 merged as 1adea6f519160c89934a20d8c183885fa4a03fab after final head
f3cb4bf331d398d113131b07cf278612c2a2e059 passed every job in run 43510850834.
Prior code candidate a4e4896f passed run 43508365932; verified archives, exact
Linux/Windows inventory fields and every notice text; all 18 affected before/after
native About snapshots are pixel-identical at scales 1/1.5/2. Qualification and
artifact identities are in docs/validation/pulldown-cmark-0.13.6.{md,json}.
Integrated the six-file merged patch locally after an 80-file full backup
(/tmp/dpdf-before-main-1ade.*); all 79 unrelated pending hashes stayed identical,
and CHANGELOG gained only the focused parser bullet. Local notice/format/diff/
metadata checks pass. This patch is not broader T19/platform/security clearance.

PR #10 remains draft. Prior recovery head 2d75b74b passed every job in run
43506767494 and native replay, but no submitted lifecycle review/owner GO exists.
A new native Ctrl+P reproduction on that source confirmed early protected
preparation can still stall the reader; see T03-protected-preparation-stall.json.
The increment defers all missing protected print/export working bytes through
the same bounded pool, cancels retained commands on close and joins on shutdown.
Two generated regressions cover protected print shutdown/export close, source
and pre-existing destination preservation, no queued child launch and reaping.

Candidate 6f54acf8 failed Linux compilation in run 43512132506 because recovery's
match did not yet reject the new decryption-only result. Fixed that exhaustiveness
error in f13080e049bf8261bc489cfb17dbe445006d9137, tree
d43290b1ffcebf974fe01cb33b39b4ec2ee019bc; exact local/remote worker blob verified
9023baabcadc281f49e0a7335c5837c991d200cf. Corrected run 43512520241 passed compile/Clippy/unit tests and Windows hosted
checks, but Linux runtime failed because ordinary smoke disables native print
admission. The new protected-print fixture never queued its command. Fixed that
fixture by enabling admission only around the controlled request (GTK remains
file-backend-only), and integrated qualified main 1adea6f in normal merge head
8a26ab45ff5fba5cfcaef8621a55706daac5033e, tree
7f4cc3858904e0ea4bfe288c7085e8a68f1e1ad3. Protected head 8a26ab45 passed every job in run 43514021130 and the original native
Ctrl+P/page-25 replay; see T03-protected-responsive-replay.json. The subsequent
overlay preflight/owned export increment remains unqualified.
Format/diff/Python syntax/metadata checks pass; no local Rust compilation. Other
T03 overlay/image preflight and detached final export/print ownership remain
unfinished. Keep PR draft pending current-source experienced lifecycle review.

PR #11 merged as a55cd895 after final fa02b37e passed full native/Linux gates,
measured ~1.65 GiB fresh compiler commit under the unchanged 2 GiB cap, exact
Linux/Windows notice equality and repeated generated reader comparisons. This
is scoped evidence, not full T19 completion or a universal memory bound.

PR #13 merged as de7a53d after run 43502593114 and native original reproduction
replay at scales 1/1.5/2. Local integration had a TESTING prose conflict; code and
paragraph were merged explicitly, checked present and unrelated pending hashes
preserved. Full backup: /tmp/dpdf-before-main-de7.* (75 pending files).

Pending source in this active worktree matches the corrected PR #10 functional
delta plus the unqualified overlay increment on main e7bec79. Canonical T02/T12/T19 planning documentation is published; later receipts are preserved here. Source
format, diff and retained notice checks passed; the public candidate excludes
unrelated prose/changelog edits. No release/version/tag or production baking.

## Preservation

Before advancing to merged a50144c, saved tracked patch, all 62 edited/untracked
file hashes and a complete file archive under /tmp/dpdf-before-main-a501.*.
Applied the validated focused fix without discarding async/T02/T12/T19 work;
unrelated file identities were checked unchanged. No release/tag/version changes.


Documentation publication candidate: draft PR #15, head
1b25e39cf1fc38d6288a7073a2f243e339e6d308, tree
38945204174c310353222e0cd3b35537c96c9def, on main 1adea6f. All 71 source/evidence
blobs matched readback; local file targets across 130 Markdown documents and all
74 requirement IDs passed. Run 43513506628 is live. This prose/evidence branch
contains no functional helper code and cannot qualify draft PR #10. The active
implementation worktree's canonical changes remain preserved separately.

Protected candidate 8a26ab45 passed run 43514021130 and native replay; subsequent overlay source is unqualified.


PR #15 merged as e7bec799b0d5ff30175fa28cd2ff743d9b9b4a72 after every job in
43513506628 passed (Linux 132189437561, Windows 132189437996,
policy 132189437988). Canonical T02 documentation, original plan/audit and dated
progress/evidence are now published. Local integration used an 85-file full
backup /tmp/dpdf-before-main-e7be.* and preserved all 84 unrelated hashes;
CHANGELOG gained only the focused documentation bullet. Two newer progress
records were retained instead of overwritten by their published snapshots.

Overlay work now reproduces native Save As blocking page 25 until qpdf release.
The draft implementation moves preflight/serialized validation/final save into
the owned helper pool, with only byte results crossing threads. Final publication
returns to the document owner so Close can discard stale staging files. New
multi-stage failure injection and pure staging/alias cases are prepared; current
compiled/runtime qualification is PENDING, lifecycle review still required.


Overlay candidate 285843d2 passed full Linux suites and all ten helper cases;
original native recovered-mark Save As replay renders page 25 before release,
with zero aligned body-pixel changes after save. Windows strict Clippy found the
page_count wrapper now used only by Linux printing/tests. Corrected its target
cfg in current head 10b443f4d6c83cdda2097140fd2eeabcad50f5cc, tree
d1a4400c5b3b57324f57d0d2d689bf1bb1299872. Run 43519053632 is live. Keep PR #10
draft; required experienced lifecycle review has not been supplied.

Next image/page increment is isolated in /tmp/dpdf-image-preflight, branch
work/image-preflight based on 285843d2, with the Linux-wrapper cfg fix also applied.
Its native image replacement reproduction on qualified 8a26ab45 confirmed blank
page 25 until target-unit metadata release; see T03-image-stall-reproduction.json.
Draft code snapshots/decodes one image at a time off the reader, creates native
PDF image objects on the owner, then finalizes units/assembly in the owned pool.
Page exports use staged publication. Source-only checks pass; this isolated code
has no compiled/runtime qualification and has not been published.


Overlay 10b443f4 passed all jobs in 43519053632 and repeated exact-source native
replay (zero aligned body pixel differences). It remains draft for lifecycle
review. A subsequent deterministic admission test on 1985833a reproduced that a
successfully queued Close left a ready helper request valid before owner cleanup:
run 43523509748, 90 tests passed/one intended contract failed. A rejected-close
case passed. Current proposed fix marks/cancels on successful admission, rejects
late registration without leaking a lease, and atomically claims publication.
It still needs its own full hosted/runtime validation.

Image/page draft PR #16: head dfa5a999efea887e599a676e786b446eb7599177, tree
d0f18952276566a483b8581cccdede9052be2c60. Run 43522795495 passed every job,
after 72cdbd21's Linux success exposed another legacy Windows-unused wrapper.
All fifteen Linux helper cases passed, including the 55-page mixed-input retry;
source-bound native image replacement replay renders page 25 before helper release
with identical aligned body pixels. Exact-source final-head replay and retention
are still being completed separately; it depends on PR #10 and must integrate the
close-admission/publication follow-up before any merge. Its source remains isolated
under /tmp/dpdf-image-preflight, never overlaid onto this active worktree.


## Final PDF/print helper increment ready for review

Current functional head e71a6726 in draft #17 passed every job in run
43532813603 and all eighteen helper-window cases. Original native discovery close
now cancels/reaps its helper (~0.018s). Native print shutdown, tab-close/user-cancel
and nested-helper group cleanup passed again on that exact Linux artifact, with
unchanged sources, empty print scratch and no submission. Inventories/text hashes
match both native targets. Exact qualification and nested-reaping scope are in
T03-print-ownership-2026-11-14.md and linked replay JSON receipts. #10/#16 parents
retain their earlier exact-source qualification. All three require experienced
lifecycle review before merge; T03 is REVIEW, not complete. Subsequent receipt-only
publication does not silently transfer source or manual acceptance.
