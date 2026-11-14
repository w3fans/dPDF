# Implementation progress index

Current local base: 1adea6f519160c89934a20d8c183885fa4a03fab, merged PR #14.
Focused text-history fix #12 is included.
This is a development worktree with preserved uncommitted documentation and
async helper work, not a release candidate. Refer to task cards for full scope;
partial increments below do not replace their exit criteria.

| Task | State | Evidence / next action |
| --- | --- | --- |
| T01 | DONE for public portability | Public unit split merged #5; installed authority integration remains separately pending |
| T02 | DONE locally for reconciliation | T02-documentation-2026-11-13.md; canonical documentation remains uncommitted here |
| T03 | IN_PROGRESS | Core bounded helpers merged #8; async metadata #10 passed CI/replay but awaits experienced lifecycle review; overlay/image/early export preparation and detached operation ownership still need work |
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
9023baabcadc281f49e0a7335c5837c991d200cf. Corrected run 43512520241 is live; qualification remains PENDING.
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

Pending source in this active worktree matches the PR #10 functional delta on
main de7a53d, alongside preserved uncommitted T02/T12/T19 documentation. Source
format, diff and retained notice checks passed; the public candidate excludes
unrelated prose/changelog edits. No release/version/tag or production baking.

## Preservation

Before advancing to merged a50144c, saved tracked patch, all 62 edited/untracked
file hashes and a complete file archive under /tmp/dpdf-before-main-a501.*.
Applied the validated focused fix without discarding async/T02/T12/T19 work;
unrelated file identities were checked unchanged. No release/tag/version changes.


## Independent T19 engine update candidate (14 November local)

Prepared paired PDFium 8125 / pdfium-render 0.9.5 from main e7bec79 in isolated
/tmp/dpdf-engine-upgrade. This source does not include the unmerged T03 lifecycle
drafts. See T19-engine-8125-2026-11-14.md and its exact provenance receipt for
verified crates/archive/source/license identities, coupled pin/source changes and
actual source-only checks. Hosted compilation/runtime, Fedora source build,
remaining advisories and experienced engine review are PENDING. No new platform
or security clearance, version/tag/release or production baking.
