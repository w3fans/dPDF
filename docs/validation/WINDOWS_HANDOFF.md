# Windows validation agreement

## Native Windows validation entry point

Resolve the authoritative release tag/commit or the owner's explicit candidate
request before every run. Never infer a release tag from the Cargo manifest.

Handoff snapshot, **2026-11-05**: the tagged preview was **v0.1.45**, commit
`d8425df31fe9ddd525e1079a1aa7622103bd1307`. The reconciled packaging/printing
candidate is the separate, **untagged** commit
`1e0edee390a65c23fa477a51dba399d524ee8394`; its 0.1.45 Cargo manifest does not
make it the v0.1.45 release tag. Historical requests below retain their original
version/evidence context.

Validate a candidate from a new clean Git worktree using the existing authorized
identity, native MSVC setup and dedicated dPDF caches. Keep existing edits and
other projects/toolchains intact. Run that candidate's
`scripts/ci/validate_windows.py` with its exact `--commit` and `--ref`, an output
directory outside source, and `--lane self-hosted`. Use the actual installed
Python executable; do not assume a particular launcher exists.

The existing harness enforces one logical CPU equivalent, one build job and
2 GiB aggregate child memory; admission waits for other native builds and at
least 3 GiB free RAM. Report resource/prerequisite blocking without removing limits.
The optional desktop lane requires a dedicated visible fixture session, not the
normal user's desktop or clipboard. Use generated fixtures only.

Return exact source/executable/report hashes and actual results privately before
publishing a report with local host/user/path details removed. Node enrollment,
notification receipt, manual validation, hosted PR CI and physical acceptance are
separate states. A staged request is not delivery, and a local PASS does not
replace required hosted checks. Stable-milestone acceptance does not freeze normal
qualified development patches.

## Historical requests

Implementation and Fedora checks run from the configured [private]/[private] workflow.
The owner's separate Codex on [private] validates Windows builds and runtime behavior.
GitHub commit IDs and validation branches are the shared handoff; direct SSH to
[private] is unnecessary. File share/RDP remain available if needed.

The first report, [windows-1c57871.md](windows-1c57871.md), confirms a native MSVC
build on [private]. It records the existing dGK GNU toolchain remaining intact. The
owner also recalls [private]-driven dGK Windows builds; inspect that established setup
before introducing an additional cross-build path.

## Next requested validation: v0.1.1

Once the tag is published, record its exact application commit separately from
the report commit. Follow local Windows agent/build rules and use the existing
MSVC/PDFium/qpdf setup. Keep fixes off main; report findings for implementation.

1. Build with `build.ps1 -Release`; run formatting, locked tests and Clippy.
2. Rerun the unchanged `check-pages.py` and `check-marks.py` corpora. The former
   now closes the reserved temporary-output handle before qpdf writes it; both
   corpora use filenames legal on Windows. Verify successful exports and unchanged
   inputs/previous destinations after invalid operations.
3. Run `--smoke-ui` with application scale 1, 1.5 and 2. The diagnostic now uses
   actual Slint sidebar/canvas positions and configured thumbnail dimensions.
   Record native pointer behavior separately; synthetic dispatch does not establish it.
4. Repeat the reader corpus and native Open/Save As/copy/navigation checks.
5. Keep printing marked incomplete until actual current/range/all-page output,
   page counts and scaling are verified. Avoid attributing automation-tool capture
   failures to the application without independent evidence.

Publish the report on `validation/windows-v0.1.1` at
`docs/validation/windows-v0.1.1.md`. Include the tag/commit, toolchain, executable
hash, results and reproductions; use only generated fixtures. Exclude private
documents, clipboard contents, credentials and normal mark/preference data.
Later previews follow the same `validation/windows-VERSION` convention.

This handoff requests validation; it does not claim the fixes have already passed
on [private] or qualify a stable milestone.

## Additional studio validation: v0.1.2

Test the same v0.1.1 corrections plus the new Marks window. Use an explicit empty
DPDF_DATA_DIR and the generated scripts/fixtures/page-photo.jpg with
`dpdf.exe --smoke-marks IMAGE`. This studio test needs no PDFium and checks crop
dragging, changed-source capture, save/reopen and physical sizes. Repeat at useful
application scales and inspect native window usability separately.

Manually test Import Image, Paste Image with a generated clipboard fixture, source
and cleaned previews, crop/deskew/levels, library browsing, mm/aspect controls and
Save Default Size. Keep normal signature assets untouched. The explicit
DPDF_SMOKE_MARKS_CLIPBOARD diagnostic writes fixture clipboard contents; on Linux
the supplied --marks clipboard script isolates this with Xvfb. Do not mistake
that X11 result for Windows or Wayland acceptance.

Report on validation/windows-v0.1.2 at docs/validation/windows-v0.1.2.md, keeping
the exact application commit/executable hash separate from the report commit.
On-page placement and overlay export are still future work in this preview.

## Additional overlay-engine validation: v0.1.3

Run `scripts/check-overlays.py` with the matching native binary, PDFium and qpdf.
It checks exact untouched-page streams/pixels, affected-page text preservation,
all crop/page/mark quarter turns, opacity, multiple marks and output failure safety.
The new `--overlay INPUT OUTPUT PLACEMENTS.json` path is a diagnostic copy exporter;
mouse placement remains pending. Ordinary overlays are independently editable
images, with no secure baking claim. Record the application tag/commit and binary
hash on `validation/windows-v0.1.3` with the usual report path. Repeat reader,
page and studio checks, and retain the unresolved native printing gate.

## Additional graphical placement validation: v0.1.4

Test Place on PDF from imported and saved assets, click placement, movement/corner
resize, mm/aspect/reset/opacity controls, tab isolation and zoom/view rotation.
Ctrl+S should export a marked copy and reopen its saved view. The source remains
unchanged. Unsaved marks require discard confirmation on close; printing and page
operations must ask the user to save the marked copy first. Check cancellation,
failed exports and an edit made while export runs. They must retain unsaved marks.

Run the studio and reader diagnostics with generated fixtures. Set
DPDF_SMOKE_PLACEMENT_IMAGE to scripts/fixtures/page-photo.jpg for the reader's
placement extension, with PDFium and qpdf configured. Report on
validation/windows-v0.1.4 with exact application commit/executable hash. This
ordinary-overlay preview makes no secure baking or native printing qualification.

## Corrective retest: v0.1.5

The v0.1.3 report is retained at windows-v0.1.3.md. This patch normalizes local-drive
and UNC Windows paths only at qpdf JSON/CLI boundaries; Rust keeps canonical paths
for source checks and atomic publication. Paths whose normalization could change
meaning fail rather than reaching the helper. Rerun the unmodified page and overlay
corpora with the official qpdf binary directly, with no normalization wrapper.
Check generated filenames containing spaces/Unicode and a writable share if available.

Studio save and placement now synchronize locked dimensions before reading them.
The studio diagnostic changes width and invokes Save Default Size in the same event
turn, and repeats a fast width edit before placement. Run it at scales 1/1.5/2 with
fresh data directories. Repeat the graphical v0.1.4 placement and reader checks.
Report on validation/windows-v0.1.5 with exact source commit and binary hash; native
printing remains an independent unresolved acceptance gate.

## v0.1.6 alias correction and independent Fedora work

The v0.1.5 report is imported directly from the validation branch. Existing input
files are now protected by filesystem identity as well as resolved path names.
Rerun check-aliases.py with official PDFium/qpdf, including the existing loopback
UNC case when accessible. Also rerun the ordinary page/overlay corpora and studio
checks. Preserve exact application and report commits as before. Native Windows
qualification is independent; Fedora development and release checks continue
without waiting for [private] or asking the owner to relay messages.

## Review/state follow-up: v0.1.12

The v0.1.10 report has been imported directly. Its native build, standard corpora,
adapted password/recovery and scale checks pass; native password entry, printing
and pointer acceptance remain open. The v0.1.11 review/revision family and v0.1.12
state fixes need their own exact-tag qualification. In addition to usual suites,
run check-review.py and check-review-ui.py. The shared window suites can now use
DPDF_TEST_NATIVE_DISPLAY=1 directly, with generated native fixture windows and
isolated config/data; no private Windows desktop or clipboard diagnostics are
claimed. Check current versus selected revision behavior, tab edit-state reset,
missing-helper retry, mixed/remarks-only recovery and source preservation. Continue
recording native pointer/password/printing limitations independently of dispatch.

## Native Windows printing: v0.1.13

The toolbar and Ctrl+P now use a print-child system dialog/GDI adapter; Quick Print
retains the optional helper. The adapter passed Linux metadata type checking only
so far. Build natively at the exact tag, run the usual corpora/window suites, then
run scripts/check-windows-print.py with --allow-native-dialog as documented in
TESTING.md. It permits only Microsoft Print to PDF, regardless of the selection
made in the dialog. No physical queue should be used by this diagnostic. Record
native build/driver/suite outcomes and separately qualify OS Ctrl+P, copies,
orientation, Unicode/UNC output, reviewed/hidden/protected snapshots and parent
responsiveness. Preserve source hashes. Keep physical output and driver-owned
prompt limitations explicit; do not infer them from print-to-PDF success.
Report at docs/validation/windows-v0.1.13.md on validation/windows-v0.1.13 with the
exact application commit and executable hash, preserving the established handshake.

## Current requests: file handoff — 2026-11-03

From v0.1.42 onward, per-release requests are written to the [private] file handoff
`/zDev/dPDF-handoff/requests/vX.Y.Z.md` (README there describes the protocol);
[private] already uses `/zDev/dPDF-handoff/` to deliver files. Reports still go to
`validation/windows-vX.Y.Z` branches, which the [private] session fetches directly.
The hosted lanes continue to validate every tag automatically; the [private] runner
lane remains pending until `[private]-dPDF` is online.
