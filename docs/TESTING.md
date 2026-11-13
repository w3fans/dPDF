# Validation and sample workflows

## Automated checks

Private-repository compilation uses the established controlled build hosts;
paid hosted compiler jobs are disabled. Public repositories may retain useful
independent hosted checks. A skipped or unassigned job is not test evidence; see
[BUILD-INFRASTRUCTURE.md](BUILD-INFRASTRUCTURE.md). Locally, the Linux lane is:

```sh
cargo build --locked --release
python3 scripts/ci/validate_linux.py --commit "$(git rev-parse HEAD)" --ref local \
  --binary target/release/dpdf --pdfium /path/to/libpdfium.so --qpdf "$(command -v qpdf)" \
  --output /tmp/dpdf-linux-validation   # add --no-desktop to skip Xvfb suites
```

Use the standard remote Cargo tools for unit tests and Clippy:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked -- -D warnings
./build
```

Tests cover geometry/fit/rotation/facing edge cases, visible render-window selection,
search-marker rotation, anchored zoom, byte-budgeted cache eviction, bounded printer ranges, and byte-preserving atomic
save replacement, preference persistence/format handling, controlled CUPS argv and
byte transfer, and printer discovery parsing. These checks do not prove printer output or interactive UX.

Generate an original fixture locally (Python standard library, no dependencies):

```sh
python3 scripts/make-sample.py /tmp/dpdf-contract-50.pdf --pages 50
DPDF_PDFIUM_LIBRARY=/path/to/libpdfium.so ./target/debug/dpdf --probe /tmp/dpdf-contract-50.pdf
DPDF_PDFIUM_LIBRARY=/path/to/libpdfium.so ./target/debug/dpdf /tmp/dpdf-contract-50.pdf
```

The fixture has searchable `payment` on every page, unique clause numbers, a
signature line, and bookmarks. `--probe` opens and renders the first page using the
same render function as the UI and reports timings. It is not an end-to-end UI
benchmark; capture real-file timings and hardware separately.

## Headless search corpus

```sh
PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-reader.py \
  --pdfium /path/to/libpdfium.so
```

This runs the built executable against generated fixtures: 100 occurrences on
50 pages, case-insensitive/empty/absent queries, offset crop boxes, intrinsic page
rotations 0/90/180/270, the explicit 10,000-match cap, and invalid PDF failures.
A unique phrase is used for geometry comparisons because intrinsic rotation can
change the extraction order of repeated words. `--probe-search PDF QUERY` exposes
machine-readable search counts and the first match's normalized bounds.

## Real-window diagnostic

`dpdf --smoke-ui SAMPLE.pdf` opens a real desktop window, exercises rendering,
thumbnail/bookmark loading, dispatched rectangle drag/Ctrl+A text selection, navigation, page search and continuous/facing/rotated
views, tab closure/scroll restoration, and dispatched Ctrl+wheel/ordinary wheel input,
thumbnail drag into a changed output order and cancellation,
then exits. Use the generated English fixture with `payment` on every page.
It times out with a failure if the expected states are not reached; it does not
submit print jobs. It exercises the application model/renderer and Slint window-event dispatch;
The print panel is opened through dispatched Ctrl+P, its asynchronous discovery
completes, and it closes without submitting a job. OS-delivered input and native
file-dialog interaction still need manual acceptance. Optional `DPDF_SMOKE_SNAPSHOT` names a
PPM capture of dPDF's own final window for visual inspection.

```sh
DPDF_PDFIUM_LIBRARY=/path/to/libpdfium.so DPDF_SMOKE_SNAPSHOT=/tmp/dpdf.ppm \
  ./target/debug/dpdf --smoke-ui /tmp/dpdf-contract-50.pdf
```

## Scenario A — first stable milestone

On both Fedora and Windows:

1. Open the 50-page fixture via dialog, CLI and file drop. Open a second PDF in a
   tab; switch/close tabs and confirm view state and independent document content.
2. Click page 7 thumbnail; jump to pages 1/50 by entry; use bookmark navigation.
   Type a multi-digit page number slowly to catch accidental input resets.
3. Exercise single, continuous and facing views; scroll across pages and confirm
   the displayed page number; test a 3-page PDF's final facing page.
4. Zoom/fit/100%, rotate four times, toggle themes/sidebar, resize the window,
   and repeat at a different display scale. Confirm no stale page images appear.
5. Search for `payment`, a unique clause phrase, and a nonexistent string. Step
   forward/back through matching pages; change tabs/search during an active search.
6. Save As to a fresh file and compare input/output bytes (`cmp`, or PowerShell
   `Get-FileHash`). Repeat after changing zoom/rotation; the PDF must be unchanged.
7. Print current page 7, range `1-3,7`, all pages, and actual size vs fit using a
   controlled test printer/PDF printer. Verify the actual output page count and
   physical scale. Invalid/out-of-bounds ranges must not submit jobs.
8. Test missing PDFium, nonexistent/corrupt PDFs, encrypted/password PDFs, a
   signed PDF, missing printer/helper, and unwritable save destinations. Errors
   must be clear and leave the source intact.

Native Windows printing/dialog, Windows/HiDPI input validation, packaged
launch and long-document performance remain acceptance work; record failures rather
than declaring v0.2.0 from a passing build alone.

## Later scenarios

v0.4.0: use the fixture and an independently generated signed-last-page PDF/image.
Replace page 10, inspect pages 1–9 for text/resource/visual preservation, open the
result externally, then exercise insert/remove/reorder/extract/merge. Test signature
cleanup and physical sizes against a printed mm ruler before overlay export.

v0.6.0: build a redaction corpus with crossing text/images, shared/nested form
XObjects, transparency, rotation and crop offsets. Bake a signature/stamp block,
reopen, extract text/images/objects and inspect in an independent PDF editor.
Inside live content must be gone; outside content must remain. A visually plausible
covering bitmap is a failure. Verify Revision 1/2/3 note/highlight/underline metadata,
visibility filters, reopen behavior and clean-copy exports independently of baking.

## Preferences acceptance

Normal smoke runs leave user preferences alone. To exercise restoration and save,
create a temporary config directory with a version-1 `preferences.json`, then use
`DPDF_CONFIG_DIR=/tmp/your-test-directory DPDF_SMOKE_PREFERENCES=1` with `--smoke-ui`.
Use the generated fixture and keep `sidebar_visible` true for thumbnail assertions.
Verify that the selected printer/scaling survive and the smoke's final theme/layout
are written. Repeat with corrupt/future-schema JSON and confirm the file is unchanged.
Do not point this diagnostic at your normal configuration directory.

## PDF page-assembly corpus

```sh
PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-pages.py --binary target/release/dpdf \
  --qpdf /path/to/qpdf --pdfium /path/to/libpdfium.so
```

The corpus verifies contract page-10 replacement with exact raw content streams
and rendered equality on pages 1–9; insert/remove/extract/reorder/multi-PDF merge;
relative page rotation without content regeneration; PNG/JPEG replacement, phone
orientation, image aspect/centering, original JPEG stream preservation, alpha PNG,
mixed PDF/image merge, rotated/cropped targets and oversized/corrupt image failure;
clean-catalog extraction;
unchanged inputs; rejection of source overwrite; and preservation of previous
output after invalid requests. Render checksums are diagnostic comparisons, not
cryptographic signatures or security seals.

An additional GUI path is enabled by `DPDF_SMOKE_ASSEMBLY_SOURCE=/path/to/signed-page.pdf` (or a PNG/JPEG scan)
with the generated contract and a configured qpdf. It opens a diagnostic-owned copy,
changes that copy on disk after opening, exports from the reader's retained snapshot,
and verifies that the edited copy opens and renders. It only writes inside its
private temporary workspace, which is removed on exit. This bypasses native file
pickers for deterministic testing; manual picker/certificate-warning interaction
still needs acceptance. No physical printing occurs.

## Native clipboard acceptance on an isolated Linux display

With Xvfb available, run:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-clipboard.py \
  --binary target/release/dpdf --pdfium /path/to/libpdfium.so
```

`--xvfb /path/to/Xvfb` can select a separately provided test helper. The script
creates a private authenticated X display with TCP disabled and a generated PDF,
then dispatches drag selection, Ctrl+A and Ctrl+C. A separate clipboard instance
must read back identical fixture text. The display is terminated on exit; the
normal desktop clipboard is untouched. Do not set `DPDF_SMOKE_CLIPBOARD` for
ordinary desktop smoke runs, since that explicit diagnostic writes fixture text
to its display's clipboard. Wayland and native Windows copy/paste into another
application remain manual platform acceptance.

`DPDF_SMOKE_ORDER_SNAPSHOT=/tmp/order.ppm` captures the output-order preview
before cancellation, after the reordered source thumbnails finish rendering.
The reader smoke also holds a page drag at the bottom edge until the list moves
beyond its initial viewport, checks the exact dropped order, verifies that leaving
the edge stops scrolling, then scrolls upward to the first-page bound and cancels
outside the list. Clipboard acceptance requires these milestones at each scale.

## Document tabs

For overflowing document tabs, run:

```sh
python3 scripts/check-tabs.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --xvfb /path/to/Xvfb --scale 1.5
```

The isolated window opens twelve generated copies, dispatches Ctrl+Tab,
Ctrl+Shift+Tab and Ctrl+W, resizes twice, closes background/all tabs and reopens
one. It checks that the complete selected tab and its close button are visible,
manual strip scrolling survives a label refresh, selection identity survives
background closure and empty strips do not retain stale offsets. `--snapshot`
captures the initial overflow view. Standard Fedora and dedicated Windows desktop
lanes run this at 1×/1.5×/2×; service mode never runs it.

## Reading-position reflow

```sh
python3 scripts/check-viewport.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --xvfb /path/to/Xvfb --scale 2
```

Twelve generated real-window cases combine single/continuous/facing layouts,
fixed/fit-width zoom and rotation. Each hides/restores the sidebar, opens/closes
search and resizes/restores the physical window, checking the page-relative
reading point within 1.5 logical pixels or the required document-edge clamp.
The same cases dispatch Ctrl+plus and Ctrl+minus and verify that zoom preserves
the reading point, including facing-page gaps and rotated pages. Toolbar buttons
use the same zoom command; pointer-wheel zoom retains its pointer anchor.
The suite also clicks the zoom field, selects/replaces its text and presses Enter:
`0%` must leave the view unchanged and restore the current display value, while
`175%` must set that exact zoom and retain the page-relative reading point.
Escape and focus loss must restore an uncommitted value without changing zoom.
The Linux viewport fixture disables access to the ordinary session portal bus.
Fedora releases and the dedicated Windows desktop lane run all cases at three
scales; service mode excludes them. The recorded failing baseline is described
in [the reflow investigation](validation/viewport-reflow.md).

## Engine discovery

Physical mark-unit preflight can be checked with generated fixtures:

```sh
python3 scripts/check-page-units.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --qpdf /path/to/qpdf
```

Seven cases cover omitted/one/null/indirect-one units, rejected half/double/indirect
double units, unchanged input bytes, retained existing output, absent new output
on refusal, and an untouched custom-unit page beside a marked default-unit page.
The standard Fedora gate and both Windows lanes run this check. This establishes
safe refusal of unsupported mark sizing, not complete custom-unit support.

Native file-print suites also generate physically equivalent letter pages with
`/UserUnit 0.5` and `2`. Fit and Actual Size must retain the reference one-inch by
half-inch colored marker within three rendered pixels, with unchanged inputs.
Actual Size reads unit metadata through the configured qpdf helper; Fit does not
require that lookup. The generated failing GTK baseline is recorded in
[the native print-unit investigation](validation/native-print-units.md).

Engine discovery can be checked without a desktop:

```sh
python3 scripts/check-engine-paths.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so
```

The test uses isolated executable/library copies and a generated PDF. It verifies
absolute overrides, adjacent engines and refusal of relative overrides. Native
Windows additionally places a valid engine in the document directory and PATH,
and requires an unconfigured executable to refuse both. No system paths,
registry entries or normal application data are modified.

## Generated Rust block investigation

```sh
python3 scripts/check-block-rust.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --qpdf /path/to/qpdf --poppler \
  --output /tmp/rust-block-report.json
```

The default covers 32 generated geometry cases, one indirect-resource positive
case and fourteen refusals. `--case 18`
selects a focused case without claiming full-matrix coverage. `--poppler` requires
independent rendering/extraction tools. Standard Fedora patch gates run the suite;
native Windows service/desktop lanes run its headless checks and attach the exact
binary's report. Production secure export remains disabled.

The low-level `dpdf --probe-block NEW_DIRECTORY GENERATED_CASE` accepts only cases
0–31 or named refusal fixtures and refuses existing output directories. It never
accepts an input PDF/signature. `--render-page INPUT PAGE OUTPUT.ppm [WIDTH]`
accepts widths 32–3072 for bounded diagnostic print-resolution exports.
The probe now needs both PDFium and qpdf, including direct CLI use. Its initial
catalog gate accepts only the generated basic `/Type`/`/Pages` catalog; extended
catalogs are refused before page edits. This does not qualify arbitrary PDF inputs.

Physical reader dimensions use a separate real-window regression:

```sh
python3 scripts/check-physical-reader.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --qpdf /path/to/qpdf --scale 1
```

Run scales 1, 1.5 and 2. Fedora uses the same authenticated private X display as
the other GUI suites; native Windows runs only in the dedicated desktop lane.
The five generated cases cover mixed units 2/0.5/1, rotation/crop, broken review
metadata, missing qpdf and invalid units. They check the 96-DPI physical layout,
reading anchor, repeated facts, early mark input and background-tab isolation.
Input PDFs must remain unchanged.
The same suite also writes a custom-unit checkpoint in one process and restores
it in another, with a mark that exceeds raw bounds but fits the physical page,
then exports through the worker and checks its PDF matrix and unit scale.

## Signature/stamp backend diagnostics

The graphical studio uses the same backend. Use a temporary library for these
diagnostics without touching saved marks:

```sh
python3 scripts/check-marks.py --binary target/release/dpdf
./target/release/dpdf --marks clean scan.jpg /tmp/cleaned.png
./target/release/dpdf --marks import scan.jpg 'Office stamp' stamp /tmp/dpdf-marks
./target/release/dpdf --marks list /tmp/dpdf-marks
./target/release/dpdf --marks size ASSET_ID 45 20 /tmp/dpdf-marks free
```

Types are signature/stamp/initials/other. Import and clean accept a final optional
JSON settings path (import: after the library directory):

```json
{"crop":{"x":120,"y":20,"width":160,"height":150},"deskew_degrees":-3.0,"remove_white":true,"white_level":240,"black_level":10}
```

Use `crop: null` for the whole image. Deskew is limited to ±15 degrees. White level
must exceed black level; all-transparent results fail. Settings apply after phone
orientation. Source images are preserved; existing alpha is retained and colored
ink is reconstructed without a white fringe. Preferred size is independent of
source pixels. The default aspect lock requires a proportional width/height; use
`free` only when intentionally saving a different ratio. Omitting a library selects
the platform application-data directory; diagnostics should pass an explicit path.
The corpus verifies byte-identical originals, crop/deskew dimensions, PNG/thumbnail
output, physical-size persistence, aspect validation, failure safety and future
metadata preservation. It performs no signature placement or PDF modification.

## Simple-overlay engine diagnostics

The copy-export engine also accepts an
explicit placement recipe for testing cleaned assets at physical sizes:

```sh
dpdf --overlay original.pdf marked-copy.pdf placements.json
python3 scripts/check-overlays.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --qpdf /path/to/qpdf
```

Example `placements.json` (image paths are relative to this JSON file):

```json
[{"page":10,"image":"cleaned.png","geometry":{"x_mm":130,"y_mm":240,"width_mm":45,"height_mm":20,"quarter_turns":0},"opacity":1.0}]
```

Pages start at 1. The x/y position is the bounding box's top-left on the naturally
displayed page, including its intrinsic rotation and crop. Width/height describe
the image's local physical axes; a clockwise quarter turn swaps its bounding-box
dimensions. Opacity is 0–1. The complete mark must fit within the page.
The engine accepts up to 256 placements and 64 MiB of unique compressed images;
PNG/JPEG decoding retains the shared pixel/allocation limits. Repeated paths share
captured immutable image bytes. Mark rasters are downsampled toward 400 DPI with a
4096-pixel side cap; small originals are not upscaled. Outputs are validated before
atomic replacement; source PDF, image and recipe paths cannot be destinations.

The corpus checks all intrinsic quarter turns, crop offsets, mark rotations,
physical geometry, transparency and unchanged pixels outside placement bounds.
It also checks multiple marks/pages, search text, byte-identical untouched page
streams, input preservation and failure safety. `--render-page INPUT PAGE OUTPUT.ppm`
exports a 1000-pixel-wide diagnostic raster through the reader's render path.
These are ordinary editable PDF image objects. They provide no secure baking or
cryptographic seal, and modified output invalidates existing digital signatures.

## Release-file consistency

`PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-release-version.py` runs public
adapter unit tests using temporary generated project files and mocked controller
process results. CI runs this suite without private tooling. It checks adapter
arguments, synchronized versions, changelog/notes and licensing, resume, mismatches,
controller failures and refusal of stable/three-digit preview targets. These tests
do not implement or qualify the controller's version-allocation algorithm.

On the release authority, explicitly test the installed standard controller:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-release-version.py --integration-policy ops/release-policy.json
```

This requires the controller under `${ORIG_HOME:-/www/0rig}` and Ruby; missing
prerequisites exit with an error. It copies the policy and public shell adapter to
a temporary project and exercises real preflight/validation there. The two injected
process/result tests are skipped in integration mode and covered by the unit run.
No project release version, tag or published artifact changes. Retain the command,
source/controller identity and platform with the receipt; a local pass does not
qualify a different authority installation. The adapter uses the tracked
`ops/release-policy.json`; do not substitute an unverified policy schema.

## Graphical mark studio acceptance

Use an explicit empty test data directory and an original fixture:

```sh
DPDF_DATA_DIR=/tmp/dpdf-studio-test ./target/release/dpdf --smoke-marks scripts/fixtures/page-photo.jpg
```

The real-window diagnostic imports its own copy, changes that copy on disk after
loading, dispatches crop dragging, adjusts deskew and mm sizes, saves and reopens
the asset, verifies original bytes and settings, and checks image state is released
on close. No normal library is read/written. `DPDF_SMOKE_MARKS_SNAPSHOT` can capture
the studio window as PPM. The studio diagnostic needs no PDFium engine.

For native image paste with the desktop clipboard left untouched:

```sh
python3 scripts/check-clipboard.py --marks --xvfb /path/to/Xvfb --binary target/release/dpdf
```

This authenticated, TCP-disabled display hosts the generated fixture image, pastes
it through arboard, changes its isolated clipboard, and verifies the original
capture survives cleanup/save/reopen. `DPDF_SMOKE_MARKS_CLIPBOARD=1` explicitly
enables fixture clipboard writes; use the isolated script rather than setting it
on your normal desktop. Native Windows/Wayland image-paste acceptance remains
separate platform work.

## Graphical mark-placement diagnostic

Use generated fixtures and the private authenticated X display:

```sh
python3 scripts/check-clipboard.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --xvfb /path/to/Xvfb \
  --placement --qpdf /path/to/qpdf
```

This also dispatches a page click, image move and corner resize, applies physical
size/opacity, checks view rotation/tab isolation and guards against omitted marks,
dispatches Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y / Delete, and verifies native text-field
Undo. It forces a failed output, retries, changes a draft while saving, checks the
older export retains that newer edit/history, and finally saves/reopens the current
copy and checks saved history is released. Unit tests also cover bounded history
asset release, shared image masters, no-op edits, cancelled/zero-move gestures in
all view rotations, branch edits and generation guards after Undo. Native Windows
pointer input remains separate platform acceptance. DPDF_SMOKE_PLACEMENT_IMAGE
can enable the same extension in --smoke-ui; it must point to a generated fixture.
DPDF_SMOKE_PLACEMENT_SNAPSHOT optionally captures only dPDF's own marked window.
The studio diagnostic now also checks its captured cleaned-image placement payload
and preferred size before releasing images on close.

## Protected input aliases

Run scripts/check-aliases.py with --binary, --pdfium and --qpdf. It creates only
synthetic files and aliases in a private temporary directory. PDF/page/image/recipe,
mark-cleanup and diagnostic-raster outputs must refuse existing-file aliases and
leave all source bytes unchanged. Hard links run on both platforms; Fedora also
checks symlink directories. Windows checks a pre-existing loopback drive share
when accessible, without creating shares or changing security. File identities
are checked before work and again before PDF publication. Ordinary path equality
alone does not protect drive/share aliases. This is not a guarantee against a
separate process racing namespace changes during the final filesystem operation.

## Fedora native printing

```sh
python3 scripts/check-native-print.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --qpdf /path/to/qpdf --xvfb /path/to/Xvfb
```

Requires GTK 3 printing runtime, xdotool and the test Xvfb helper. Its explicit
file-only GtkSettings property excludes physical queues and lpr. GDK_BACKEND=x11
keeps GTK inside the private display; clearing WAYLAND_DISPLAY alone is insufficient
because GDK can still find wayland-0. Configuration/data/cache and GSettings are
isolated. Native dialog key events select and accept Print to File, and qpdf/PDFium
verify page counts, source-page colors/order and actual printed fit/size geometry.
The test cancels another real dialog, protects an input hard-link alias and uses
reader Ctrl+P plus tab switching while its print process is open. It uses only
generated fixtures. --stress adds all 50 pages and page 7 alone, preserving input
bytes. DPDF_NATIVE_PRINT_SNAPSHOT optionally captures the private native dialog
(with Pillow available); the normal desktop is never captured.

Add `--benchmark-report /path/to/print-benchmark.json` to save the generated
50-page and page-7 measurements (this implies `--stress`). The native Windows
`check-windows-print.py` suite accepts the same flags, with its existing explicit
`--allow-native-dialog` opt-in and Microsoft Print to PDF restriction. Linux CI
and the Windows desktop lane enable these cases and retain `print-benchmark.json`
beside the suite logs in their validation artifact. Windows service/PR lanes
do not open print dialogs; the desktop lane supplies the Windows measurements.

Each measurement records source/output byte counts, their ratio, SHA-256 hashes,
selected zero-based pages and scaling. Elapsed time includes dialog startup,
submission, spool completion and qpdf/page-count validation. It is not pure engine
time and has no fixed speed threshold across runners. A cancelled or invalid job
or a changed source produces no successful measurement. Reports can survive a
later suite failure; check the enclosing validation report before claiming a pass.
The output PDF size is not the physical printer's spool/transport size. Original
PDF size is a comparison reference, not a measured CUPS submission. Direct-PDF
native printing and real office-printer performance remain work for R-PRN-6.

The release pipeline includes these checks when DPDF_TEST_XVFB is supplied.
Physical printer counts/scaling and native Windows printing remain independent
acceptance, not inferred from print-to-file success.

## Interrupted-session draft recovery

```sh
python3 scripts/check-recovery.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --qpdf /path/to/qpdf --xvfb /path/to/Xvfb
```

This uses an isolated data directory and private authenticated display. A first
process places/edits a generated mark, waits for a durable idle checkpoint, then
exits without normal shutdown cleanup. A second process recovers the 50-page opened
snapshot even after its source was replaced with a three-page PDF or removed.
Physical dimensions, opacity, aspect lock and preferred size are checked, the
recovered copy is exported/validated with qpdf, and the source remains unchanged
or absent. A separate run checks explicit discard. Unknown recovery files remain
intact. Standard unit tests additionally cover atomic failure/full-store retention,
shared assets, truncated/oversized metadata and live-instance exclusion. These
checks do not establish power-loss durability or native Windows crash recovery.

## Protected PDFs

```sh
python3 scripts/check-passwords.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --qpdf /path/to/qpdf --xvfb /path/to/Xvfb
```

Generated AES-128/AES-256 fixtures, including Unicode and leading/trailing spaces
in passwords, run the real-window retry/read/search/placement/save-failure/export
workflow on a private display. Ordinary Save As must match encrypted bytes exactly;
marked output must be a valid unencrypted 50-page copy. Esc cancellation and queued
prompts preserve source bytes and an in-progress entry. Fixture encryption jobs use
an isolated temporary JSON file; application passwords use only in-memory/private
pipe transport. A protected-draft restart also requires password re-entry and checks the checkpoint
contains no synthetic password. The Unix helper unit test independently inspects child arguments,
stdin bytes and error redaction without real credentials. Native Windows password
entry, edited-copy confirmation and printing remain separate qualification.

## Review revisions and mixed workspace export

```sh
python3 scripts/check-review.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --qpdf /path/to/qpdf
python3 scripts/check-review-ui.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --qpdf /path/to/qpdf --xvfb /path/to/Xvfb
```

The native corpus checks Unicode notes, highlights/underlines, revision metadata,
all intrinsic rotations/crop offsets, pixel-identical hidden output and exact
page streams. Visible/current/clean exports, external popup/reply retention,
foreign-annotation preservation, clean thread removal, invalid recipes and source
aliases are covered. Backend diagnostics are `--review inspect INPUT` and
`--review export INPUT OUTPUT MODEL.json visible|current|clean`; page indexes and
rectangles in the model use zero-based pages and normalized rendered-page bounds.

Private-window tests dispatch actual click/rectangle gestures for three remark
kinds/revisions, update note text, switch global/current/selected visibility,
place a synthetic signature independently, rotate the view, force a failed save,
export clean without retiring drafts, edit during an older save, and reopen the
current reviewed copy. Separate interrupted-process runs recover mixed and
remarks-only drafts after the original was changed from 50 to three pages.
The resulting 50-page PDFs, native annotation metadata and image presence are
independently inspected. Inputs and recovery retirement are verified.

### Native Windows context for shared window suites

The recovery, password and review-window suites now support the native fixture
context directly; no external module-loader adapter is required:

```powershell
$env:DPDF_TEST_NATIVE_DISPLAY = '1'
python scripts/check-recovery.py --binary target/release/dpdf.exe --pdfium C:\deps\pdfium\bin\pdfium.dll --qpdf C:\deps\qpdf\bin\qpdf.exe
python scripts/check-passwords.py --binary target/release/dpdf.exe --pdfium C:\deps\pdfium\bin\pdfium.dll --qpdf C:\deps\qpdf\bin\qpdf.exe
python scripts/check-review-ui.py --binary target/release/dpdf.exe --pdfium C:\deps\pdfium\bin\pdfium.dll --qpdf C:\deps\qpdf\bin\qpdf.exe
```

These display real native test windows with private fixture config/data, not an
isolated Windows desktop. They do not establish OS pointer or physical-printer
acceptance. The Linux native-print and clipboard scripts remain platform-specific.
`python scripts/test-private-display.py` checks environment isolation without a GUI.
Review-window checks now include missing-helper retry, atomic invalid filters,
source mark controls and clearing a remark tool/text target on another tab.

## Windows native printing

Run on Windows/Windows test machine with Microsoft Print to PDF installed and a native MSVC build:

```powershell
python scripts/check-windows-print.py --binary target/release/dpdf.exe --pdfium C:\deps\pdfium\bin\pdfium.dll --qpdf C:\deps\qpdf\bin\qpdf.exe --allow-native-dialog
```

This explicitly opens generated fixture dialogs on the current Windows desktop,
with separate configuration/data. It invokes only the fixture process's standard
OK/Cancel controls, without clipboard writes or injected keystrokes in another app.
The print child refuses any diagnostic device other than Microsoft Print to PDF
before starting a job. A missing virtual printer is an unmet prerequisite, not a
reason to substitute a physical queue. The suite waits for completed valid PDF
output, then checks all/current/disjoint/overlapping ranges, original page colors
and order, fit/actual physical marker geometry, bounded rasters, cancellation,
source hard-link refusal and the explicit diagnostic opt-in.

The suite's Windows execution has not yet been established by the Linux checks.
Manually qualify native Ctrl+P, printer selection, copies/collation, orientation,
Unicode output paths, current/ranges, cancellation and navigation while printing.
Test reviewed/hidden remarks and protected snapshots as separate cases. Print to
PDF is raster output; unchanged Save As preserves original content/encryption.
Physical print counts/scaling, UNC paths and third-party driver prompts require
native acceptance. Record exact application commit and binary hash on the usual
validation/windows-vVERSION branch; Fedora development does not wait for this.

## Reader startup geometry and phase measurements

```sh
python3 scripts/benchmark-reader.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --repeat 3 --output /tmp/dpdf-reader-benchmark.json
```

The 32-case correctness matrix compares dictionary-only page sizes with loaded-page
sizes at every quarter turn, cropped/negative origins, inherited page boxes and
UserUnit 1/2. The standard release pipeline runs it with --geometry-only. A full
run also measures generated 50-page office, 1000-page text and 50-page vector-heavy
fixtures. Each run starts a fresh process; files and OS caches are warm. The
--benchmark-reader INPUT --compare-eager diagnostic follows the GUI's immutable
input/describe/render path and reports library load, file read, PDF load, metadata,
first page delivery, page 7/last page/zoom/thumbnail and cache revisit phases.
Fresh documents compare lazy/eager geometry gathering using one PDFium instance.

The times are headless engine delivery, not GUI presentation or hardware-cold
startup. Linux /usr/bin/time records complete process peak RSS including the
comparison; Windows RSS is left unreported by this script. The 64-MiB frame-cache
budget excludes PDFium, input bytes, UI copies and process overhead. No fixed
machine-dependent speed threshold substitutes for representative office acceptance.
Inputs remain unchanged. No benchmark result qualifies native printing or a stable
milestone by itself.

## Secure-region backend capability probe

```sh
python3 scripts/probe-redaction.py --mutool /path/to/mutool --qpdf /path/to/qpdf \
  --binary target/release/dpdf --pdfium /path/to/libpdfium.so \
  --output /tmp/dpdf-redaction-capability.json
```

This uses a separately supplied MuPDF engine on original generated fixtures; no
engine is installed, linked or bundled by dPDF. The probe checks crossing text,
crossing vectors, shared nested forms, coarse image pixels, and a surviving
ActualText payload. qpdf validates fresh output and exposes decoded objects;
PDFium checks extraction and outside-region pixels. Exit 3 means the probe ran
and found a concrete blocker. The JSON distinguishes known blockers from broader
unverified requirements. It is not a secure export implementation or a claim that
an engine is qualified if these few cases pass. See docs/validation/redaction-probe.md
for observed results and docs/LICENSING.md before any engine distribution choice.

## About window and attribution

About / F1 opens the information/support window without a network request.
Inspect description, Denis Zvegelj author credit, installed version, Apache license,
project/issues links, support contact, expandable local QR and AboutSlint widget.
Close with Esc or Close, then reopen; closing the main reader also hides About.
Public buttons open only their fixed target through the system browser after a
user action. No payment is initiated by opening About or expanding its QR code.

The own-window diagnostic --smoke-about OUTPUT.ppm [dark] needs no PDFium. Run it
on a private X display or explicitly permitted native Windows test desktop, then
inspect the light/dark screenshots. Diagnostic capture is of the app window, not
the user's desktop. QR regeneration uses scripts/generate-support-qr.py and the
pinned qrcode==8.2 developer tool; no generator/runtime download happens in dPDF.

## Independent export rendering with Poppler

On a prepared Fedora workstation with existing `pdftoppm`/`pdftotext` tools:

```sh
python3 scripts/check-poppler.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --qpdf /path/to/qpdf \
  --output /tmp/dpdf-poppler-report.json
```

This uses generated documents and synthetic marks. Poppler checks all four intrinsic
rotations with and without crop boxes: physical overlay placement/alpha, unchanged
text and outside pixels, visible native notes/highlights/underlines, hidden remarks,
and clean copies retaining signature overlays. A ten-page replacement case checks
untouched pages 1/9 and the changed last page independently of PDFium.

Set `DPDF_TEST_POPPLER=1` for the standard patch-release workflow to run this suite
and update `docs/validation/poppler-latest.json`. The evidence records the tested
binary's version and SHA-256 plus the installed Poppler version; it must not be
attributed to a different release. Poppler is a developer test tool, not an app
dependency or bundled engine. These generated-rendering checks do not qualify
interactive external-editor behavior, Windows Poppler or physical printer output.

## Modern Windows print-dialog diagnostics

Windows test machine v0.1.14 observed Explorer hosting the Windows 11 print pane, so a strict
child-PID/#32770-only locator could not qualify output. The diagnostic now follows
a bounded, cycle-checked GW_OWNER chain back to the exact fixture process. Classic
dialogs retain their standard command IDs; broker-hosted controls use the built-in
.NET UI Automation InvokePattern from Windows PowerShell. Ownership is rechecked
before invocation. No foreground-window guessing, global keyboard injection,
clipboard writes, or unrelated Explorer controls are used.

Automation IDs are preferred. A fallback supports the reported English Print/Cancel
labels; unknown/localized/ambiguous controls fail with fixture-only diagnostics.
A shared title or broker PID alone is insufficient. Native execution of this new
locator and printed output still require their own successful report; the earlier
Windows test machine dialog-opening/cancellation observation does not qualify output.

Release tags also start a separate hosted desktop lane on a fresh Windows VM. It
checks session, visible window station and input-desktop access before running UI
tests. Microsoft Print to PDF remains the only permitted print device. This does
not qualify physical paper output or manual pointer/clipboard behavior.

## Clipboard and placement scale matrix

`check-clipboard.py` now shares the private-display isolation helper and supports
`--scale 1`, `--scale 1.5` and `--scale 2`. Fedora release checks exercise reader
copy/placement/history/export and studio image paste at each override. The dedicated
Windows desktop lane runs the same six cases with `--allow-native-clipboard` and
`DPDF_TEST_NATIVE_DISPLAY=1`. Ordinary service-mode validation never enables them.

Windows has no private display supplied by this script: use only the dedicated
test desktop or fresh hosted VM. The explicit opt-in permits replacing that
session's clipboard with generated fixture content. The image test verifies that
a captured image survives later clipboard replacement. Text copy uses dispatched
Ctrl+C and an independent native clipboard instance. Scale overrides test coordinate
behavior; manual physical monitor/pointer qualification remains separate.

## Fedora desktop registration

`python3 scripts/check-desktop.py` runs on a private D-Bus session using generated
executables/documents and an isolated user-data directory. Standard
`desktop-file-utils` validate/install the entry and update its MIME cache. Gio
then launches the registered entry to verify the exact executable path and
multiple Unicode file arguments, including spaces and literal percent signs in
PDF names. Unsupported runtime paths must fail before writing registration data.
No real PDF is opened and the user's MIME defaults are untouched. This check is
part of the Fedora patch-release gate; it does not qualify a file manager's native
drag-and-drop behavior.

## Native operating-system file drops

```sh
python3 scripts/check-file-drop.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --xvfb /path/to/Xvfb
```

This drives a genuine XDND selection/URI-list exchange with dPDF's own native
window on a private authenticated X server. It does not invoke the Slint drop
callback directly. A cancelled hover leaves the reader empty. A multi-file drop
opens generated PDFs with Unicode, spaces, percent and hash characters in their
names, including a duplicate URI while opening; native Ctrl+Tab events prove
exactly two tabs exist, and input hashes stay intact.
The pre-fix v0.1.18 executable completed XDND but opened no document; the patched
native-event hook passes. The Fedora patch gate now runs this check.

The test isolates clipboard/selection and keyboard state on its own X server.
It does not qualify Wayland compositor drops.

On a dedicated Windows test desktop:

```powershell
$env:DPDF_TEST_NATIVE_DISPLAY = '1'
python scripts/check-windows-drop.py --binary target/release/dpdf.exe `
  --pdfium C:/deps/pdfium/bin/pdfium.dll --scale 1.5 --allow-native-input
```

This starts a WinForms OLE source with a real CF_HDROP payload and drives native
mouse events into the reader's registered drop target. It checks accepted hover
followed by cancellation, Unicode/special filenames, a multiple-file drop with a
duplicate entry, and native Ctrl+Tab cycling through exactly two documents. Input
hashes must remain unchanged. Configuration, files and windows belong to the
fixture; pointer events check target-process ownership and keyboard events require
reader focus. The test restores the pointer and terminates its own processes.
Cleanup retries Windows sharing/lock violations for up to five seconds; it fails
if a lock persists or another error occurs. Overall success is printed after cleanup.

The Windows desktop lane runs scales 1, 1.5 and 2; service lanes exclude this test.
Both opt-in settings above are required because it moves the desktop pointer and
sends keys. Use a fresh hosted VM or dedicated test session, never an active work
desktop. This qualifies the native OLE protocol when it passes; manual Windows
Explorer source behavior, Windows test machine and Wayland acceptance remain separate.

Generated headless performance reports now record the exact executable version and
SHA-256, engine hash, host platform and UTC measurement time. Set
`DPDF_TEST_READER_BENCHMARK=1` for a patch release to measure the existing 50-page,
1,000-page and vector fixtures and retain `reader-benchmark-latest.json`. These
numbers measure engine delivery/cache behavior, not GUI presentation or a real
company contract. The default release gate retains all geometry correctness cases.

## Saved-file sharing fallback

The clipboard/placement suite checks that sharing copies the exact saved path to
an independent native clipboard handle, refuses unsaved signature drafts without
changing the clipboard, and becomes available after a marked-copy export. The
review UI suite verifies that unsaved remarks also disable sharing. Clipboard
cases retain their private-X11 / explicit Windows desktop opt-in requirements.
Unit checks refuse missing files/directories, retain Unicode filenames, and drive
a controlled Fedora file-manager child with a single literal folder argument,
including spaces and shell characters; a failed child must report an error.
No email is sent and no real file manager is launched by those regressions.
Manual Windows Explorer folder opening remains separate desktop acceptance.

The reader's owned shared-buffer adapter is tested for random seeks, shared payload
identity and lifetime after the original owner is dropped. Existing protected-PDF,
source-change/recovery and exact Save As suites exercise snapshot integrity through
the actual engine and UI. Generated large-buffer probes are memory investigations,
not representative office-document timing claims.

## Windows user registration

`check-windows-registration.py --binary ... --pdfium ...` stages an actual build
under a path containing spaces, Unicode and punctuation. It uses a random dPDF
validation ProgID/executable alias and a temporary Start Menu folder. Native
Windows association lookup and command-line parsing verify the registered quoted
command, the shortcut target/working directory/icon, and adjacent-engine launch
of a generated PDF through the resolved executable. The native icon loader checks
the multi-size icon. Re-registration must be idempotent and alias collisions must
fail before changing the other registration. Cleanup removes its fixture keys/link.
The test compares PDF default and UserChoice state before/after without printing
those values. It never sets dPDF as the default or changes an existing dPDF install.

Both native service and desktop lanes run this case under their existing bounded
Job Object. The probe launches its own child directly through the resolved command,
so resource/timeout ownership stays intact; it does not launch an unbounded Explorer
broker child or claim that a manual Explorer Open With interaction was performed.
Production registration follows Microsoft's [application registration](https://learn.microsoft.com/en-us/windows/win32/shell/app-registration)
and [file type registration](https://learn.microsoft.com/en-us/windows/win32/shell/fa-file-types)
interfaces using the current-user hive.

## Windows MSI upgrade and user-data retention

Use the existing `packaging/windows/test-msi.ps1` in a fresh disposable Windows
account. It now refuses preexisting dPDF data as well as an existing installation,
seeds generated preferences and mark/recovery retention sentinels at the actual
APPDATA/LOCALAPPDATA locations, and checks exact bytes after install and uninstall.
The sentinels are not valid saved mark/recovery models; native recovery/studio
corpora still prove those models reopen. Success removes only fixture-created
data roots; failure retains them and MSI logs for diagnosis. Do not rerun against
retained failed-fixture state as though it were a clean account.

For major-upgrade coverage supply both optional parameters with an existing
qualified older package (no new version allocation or publication):

```powershell
.\packaging\windows\test-msi.ps1 -Msi C:\fixtures\candidate.msi `
  -Version 0.1.54 -Fixture C:\fixtures\generated-50-pages.pdf `
  -WorkDirectory C:\fixtures\upgrade-check `
  -PreviousMsi C:\fixtures\qualified-older.msi -PreviousVersion 0.1.53
```

Substitute actual candidate/previous versions; the previous version must be
strictly older. The harness checks the previous installed executable version,
then installs the candidate through the same MajorUpgrade path, checks the new
version, runtime/registration/default association and user-data retention, and
uninstalls the candidate. Both MSI hashes are printed for source-bound receipts.
A failed install retains state for diagnosis rather than retrying other packages.
Without the optional pair it runs fresh-install coverage only.

These candidate test additions have not run natively in the current restricted
checkout; PowerShell/native MSI and upgraded saved-data acceptance remain pending.


The graphical review suite also keeps a real review Redo available while Search
has native text focus. It dispatches both platform Redo combinations, then native
text Undo/Redo, and checks the review model, complete draft generation and Redo
availability remain unchanged. Escape must close Search and restore reader focus.
This guards against an unhandled text shortcut reaching document history.
