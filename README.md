# dPDF

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/v1.18.1/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev)

A compact native PDF workspace for Fedora Linux and Windows. Rust core, Slint UI,
PDFium rendering, and qpdf page assembly. No Electron.

**Current version: 0.1.54 — reader development preview.** v0.2.0 is reserved for
the first stable reader milestone after Fedora and Windows acceptance, including
printing. The preview is not the completed v0.2.0 milestone.

dPDF-owned code is [Apache-2.0](LICENSE). Open-source application distributions
use Slint's GPL-3.0-only option with attribution; dependencies retain their own terms. See
[licensing](docs/LICENSING.md) and [third-party components](docs/THIRD_PARTY.md).

Implemented reader paths include multi-document tabs, lazy page/thumbnail rendering,
bookmarks, page jumps, single/continuous/facing layouts, zoom/fit, view rotation,
rectangular text selection/copy,
light/dark themes, occurrence search with temporary highlights, and unchanged-byte Save As. Fedora
printing uses a native GTK dialog and direct CUPS Quick Print; Windows has a native
system dialog/GDI print adapter tested with Microsoft Print to PDF on hosted Windows, plus an optional
explicit Quick Print helper.
See [milestones](docs/MILESTONES.md) for remaining acceptance gates and the
[delivery record](docs/RELEASE-STATUS.md) for source-bound v0.1.54 package and
hosted results; those receipts do not qualify later development changes.

The reader honors physical page-unit multipliers in 100% and fitted layouts,
including mixed-unit documents. Dimensions load after the first frame; mark
placement waits for verification. Mark export retains physical size on custom-unit
pages and refuses invalid units or insufficient coordinate precision.

## Build and run

On the prepared Fedora source workstation, use the **existing standard tools**:

```sh
cargo fmt --check
cargo check --locked
./build
cargo test --locked
./build --release
```

The user's Cargo wrapper and `./build` route compilation to remote build service with its existing
one-build/one-CPU/2-GiB limits. Do not bypass the wrapper to compile locally.
`build.sh` delegates to `./build`. Dependencies and compilation caches stay on remote build service;
final executables download to `target/` here.

PDFium is loaded at runtime. Use the matching **7881** distribution and set:

```sh
export DPDF_PDFIUM_LIBRARY=/path/to/pdfium/lib/libpdfium.so
./target/debug/dpdf /path/to/contract.pdf
```

Alternatively place `libpdfium.so` / `pdfium.dll` beside the executable. There is
no automatic runtime download. A missing/incompatible engine produces an error.
Overrides must be absolute paths. Windows requires the configured or bundled
PDFium DLL; Linux also supports the system library loader.

**Windows users:** each development-preview release on GitHub has
`dPDF-X.Y.Z-windows-x64.msi` (per-user install, no administrator rights, adds dPDF
to Start Menu and Open With without changing your default PDF app) and a portable
zip. The builds are unsigned previews, so SmartScreen may ask for confirmation.

On Windows/Windows test machine, use native Rust MSVC and `./build.ps1` or
`./build.ps1 -Release`, then provide the matching Windows PDFium DLL.
Every release tag also queues automatic hosted Windows and Windows test machine validation; see
[Windows CI and one-time runner setup](docs/WINDOWS-CI.md). Service and dedicated
desktop checks report their exact source commit separately.

After Fedora runtime staging, `python3 /app/dPDF/install-desktop.py` registers
dPDF in the application menu and Open With for the current user. The default PDF
viewer stays unchanged. Windows staging includes `register.ps1` for the same
per-user Start Menu/Open With workflow; use `-Unregister` to remove that registration.

See [building and packaging](docs/BUILDING.md) for runtime staging into `/app/dPDF`
or `C:\app\dPDF`, prerequisites, and the standard release-controller adapter.

## Use

**About** / F1 follows the RUWAC/GSE information and optional-support layout:
description, author, actual installed version, license, project/issue links,
developer contact and a local expandable QR code. Slint attribution appears in
that window. Opening About makes no network requests; links open only on action.
Support opens dPDF's dedicated donation page on idz.si; contributions are optional.

Open PDFs with Ctrl+O, command-line file arguments, or file drops into the document
area. Select tabs or press Ctrl+Tab / Ctrl+Shift+Tab to cycle forward/backward;
Ctrl+W closes the active tab. The selected tab scrolls into view, including after
window resizing or closing other tabs. Use the page box,
PgUp/PgDn, thumbnails, or bookmarks to navigate. Ctrl+F searches page text and steps
through individual matches, including multiple matches on a page. Enter advances an
existing search; F3/Shift+F3 select next/previous matches. Highlights are temporary
view decorations and are never saved as review remarks.

Resizing or showing/hiding panels retains a page point near the center of the
reading view, with scrolling clamped at document edges.

Toolbar zoom and Ctrl++/Ctrl+- retain the reading point near the view center;
enter a whole percentage from 10% to 600% in the zoom field and press Enter.
Escape or leaving the field cancels an uncommitted percentage.
Ctrl+wheel zooms around the pointer over a page; Ctrl+0 fits width. Fit Page, 100%, view rotation and layout
controls are on the toolbar. F9 toggles the sidebar. The theme button switches
light/dark. Ctrl+S or Ctrl+Shift+S opens Save As; this reader preview saves an exact
copy of the bytes opened, without rewriting the PDF. View rotation is not saved.

On Fedora, **Print / Ctrl+P** opens the native print dialog. Choose all pages,
Current Page or ranges, printer, paper setup and copies. Actual-size native printing
requires qpdf to read physical PDF page units. The reader stays available
while that dialog is open. **Print options…** provides Fit to Paper / Actual Size
and a range hint for **System dialog…**, plus direct **Quick Print** through CUPS.
Quick Print accepts `1-3,7`, Current Page, a discovered printer or an entered name.
On Windows, Print / Ctrl+P opens the standard system dialog with current/all/range,
printer, copies and driver page setup. Print options sets Fit / Actual Size and a
range hint before System dialog; Windows Quick Print still needs its configured
SumatraPDF helper. Microsoft Print to PDF uses dPDF's save-file picker before
submission and refuses the input PDF, including filesystem aliases.

Native print output is rendered at about 300 DPI with bounded per-page memory.
Native Print to File therefore creates a PDF of rendered page images; Save As and
Save Copy retain document content. GTK 3 printing libraries are loaded only in the
print process. Cancellation closes the job without changing the source. Job
submission and tested print-to-file output do not establish physical printer
output; physical paper-output qualification remains milestone work. Windows driver-owned file prompts outside the controlled
Print-to-File/Microsoft Print to PDF path need separate qualification.

**Share…** offers **Copy file path** and **Open folder** for attaching the saved
PDF in your email app. Save a copy first when signatures or review edits are
unsaved. These actions refer to the saved file on disk; dPDF does not claim to
attach a file to every platform's email client automatically.

Use **Pages…** to replace, insert, remove, extract, merge, rotate or reorder PDF
pages into a separate edited copy, which opens in a new tab. Page assembly uses
qpdf 11+ (11.9 and 12.3.2/12.4.2 tested; rotating pages by different angles in one export needs 12+); provide it on PATH, beside dPDF, or via `DPDF_QPDF_BINARY`.
For the contract workflow, choose Replace Page, target `10`, source page `1`, pick
the signed-page PDF or PNG/JPEG scan, and save the new copy. Scans fit the target
page without cropping or stretching; phone-photo orientation is respected. Images
inserted or appended through Merge Files fit A4. Each image is one source page.
Image imports accept up to 32 megapixels / 64 MiB compressed, use a bounded decode,
and resample large sources to approximately 300 DPI (4096-pixel side cap). Original
files are preserved. Choose Reorder Pages to drag thumbnails into the output order. Drop above or
below a thumbnail; insertion lines show the target. The sidebar labels original
page numbers and output positions. Reset restores the original order; closing
the panel discards the preview. Save edited copy applies the order in a new tab.
Hold a dragged thumbnail near the list's top or bottom edge to scroll through a
long document. Moving away from the edge stops scrolling; releasing outside the
thumbnail list cancels that drag without changing the output order.
The order field also accepts an explicit sequence; press Enter to preview it. Existing cryptographic signatures are invalidated in edited output.

**Marks…** opens the signature/stamp studio. Import PNG/JPEG or Paste Image, drag a
crop on the original (or edit pixel bounds), adjust deskew/white/black levels, and
compare the cleaned transparent result on white or paper backgrounds. Set its name,
type, notes and preferred width/height in mm, then Save New Asset. Choose a saved
item to reopen its original and cleanup settings; Save Default Size updates its
preferred dimensions. Lock Ratio keeps sizing proportional. Image processing runs
off the UI thread, and closing the studio releases its working images.
Saved assets record their last-used time when placed on a page. Choosing an asset
or cancelling placement does not count as use; Undo does not erase usage history.
Usage bookkeeping runs in the background and preserves saved default dimensions.

With a PDF open, choose **Place on PDF** in the studio, then click a page. Drag
its image to move it, or drag the selected corner to resize. Use the compact mark
bar for printed width/height in mm, aspect lock, opacity, Reset Size and Remove.
Press Apply after numeric edits. Undo/Redo buttons and Ctrl+Z / Ctrl+Shift+Z (or
Ctrl+Y) recover placement, move, resize, numeric edits and removal; Delete removes
the selected mark. Text fields retain their own Undo. Esc cancels a pending
placement or restores the start of a drag. A whole drag is one history action.
History stays with each tab and retains at most 64 actions within the image budgets;
a new edit after Undo discards Redo. Marks remain
with their source tab and follow zoom/view rotation. A `*` on the tab means unsaved
marks; closing asks before discarding them. Preview rasters and compressed draft
images each have a 64-MiB per-document budget, including retained history.
History covers unsaved drafts and clears after their successful export. Failed
saves retain drafts/history; a save finishing after another edit preserves the
newer drafts.

Unsaved placed marks also receive a local recovery checkpoint after about one second
of inactivity. The mark bar shows **Recovery pending / saved / off**. On restart,
**Recover draft** restores the opened PDF snapshot and placed marks, including mm
size, opacity and aspect lock; it works if the original was changed or deleted.
**Discard draft** removes the offered checkpoint. Recovery restores the latest
checkpoint, with a fresh Undo history. Successful export of the current draft or
confirmed discard retires its checkpoint; failure or an older export retains it.

Recovery files contain PDF and signature/stamp images, are unencrypted local data,
and stay under the dPDF data directory's `recovery` folder. On Fedora that folder is
private to the current user. They are excluded from builds and Git. Limits are
128 MiB per PDF snapshot, 64 MiB of mark images and 512 MiB for the recovery store;
exceeding a limit leaves existing checkpoints intact. One app instance owns the
recovery store at a time; another can still read/edit PDFs with recovery off.
Recovery is a crash safeguard, not a replacement for saving the finished PDF.

**Privacy…** controls recovery. Clear **Keep local draft recovery** to stop writing
checkpoints; the choice persists in preferences, and drafts saved earlier are kept
(not offered) until deleted. **Delete all recovery drafts…** asks for confirmation,
then removes every checkpoint, including other sessions' and unreadable drafts, whether
recovery is on or off. It refuses while another dPDF instance owns the store. Open
tabs keep their unsaved work; with recovery on, the next edit writes a fresh
checkpoint. Saved PDFs, signature/stamp assets and preferences are not affected.
Empty session folders left by interrupted instances are removed on the next start.

**Save copy** / Ctrl+S exports ordinary editable image overlays to a separate PDF,
then opens that copy at the saved view. Untouched pages use the original qpdf
sources. Existing digital signatures in a modified copy become invalid. Print or
change pages from the saved marked copy; those actions are blocked while the
source tab has unsaved marks. These overlays do not provide Secure Signature Block
baking or a cryptographic seal.

The local library uses Linux XDG data storage (normally `~/.local/share/dPDF/marks`)
or Windows `%LOCALAPPDATA%\dPDF\marks`. `DPDF_DATA_DIR` overrides the data directory.
Originals, cleaned PNGs and thumbnails remain local; asset folders are excluded
from Git and release synchronization. Backend diagnostics are in
[testing](docs/TESTING.md). The simple-overlay engine also accepts diagnostic placement recipes; see
[testing](docs/TESTING.md). Secure baking,
seals remain future milestone work. Saved-file sharing uses the practical fallback above. Review revisions are available as
development functionality described below.
**Review…** opens compact remark controls. Choose a revision, create a new label
(or leave it blank for the next numbered revision), then choose **Place note**,
**Highlight area** or **Underline area**. Enter the note text before clicking a page;
for highlights/underlines, drag a rectangle over the relevant text. Esc cancels the
tool. The **Review** sidebar lists visible remarks: select one to read/edit its
text with **Update text**, or use its × button to remove it.

The **Remarks** checkbox hides/shows dPDF remarks independently of signatures and
stamps. Filter all revisions, the current revision, or selected revision positions
such as `1,3`. Current follows the active revision; Selected retains its chosen
sets even when the active revision changes. Invalid filters leave visibility
unchanged. These view controls do not delete remarks. **Export copy…** can show
all remarks, preserve current visibility/filter, or omit dPDF remarks entirely.
Clean export retains the source drafts; saving a current/visible reviewed copy
retires matching drafts only after success and opens the copy. Signatures and
remarks can be saved together; remarks remain annotations rather than baked content.

Revision sets, including empty sets, persist in embedded dPDF metadata alongside
standard PDF notes/highlights/underlines. Common viewers can display those native
annotations. External changes to their text/position and deletions are read back;
linked popup/reply comments are retained for existing remarks. Clean export removes
owned review threads/metadata while preserving unrelated annotations. It is not
redaction of page content or embedded attachments. Duplicate native identifiers
are refused for review editing rather than silently dropping comments.

Review loading requires qpdf and begins after first-page display. Ordinary PDF
viewing stays available if review loading is unavailable. The review panel shows
the error and offers **Retry review**. Switching tabs clears the armed remark tool
and text-edit target before another document becomes active. Unsaved review edits,
including remarks-only drafts, share local crash recovery with placed marks;
Remark edits have their own Undo/Redo: the Review panel's Undo/Redo buttons, or
Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y while the Review panel is open (otherwise these keys
undo signature/stamp edits). Adding, removing and re-texting remarks and creating a
revision are undoable; visibility and filters are view settings and are kept.
Successful export of the current draft clears the history. Native
Windows review input, mixed recovery, external-viewer and printing qualification
remain separate acceptance work. Secure Signature Block baking remains unfinished.

Drag a rectangle over page text, then press Ctrl+C or Copy. Ctrl+A selects the
current page when the document has keyboard focus; Esc clears the selection.
Selections survive zoom/rotation and are temporary, independent of saved remarks.
Text follows PDF extraction order, which can differ from visual order in complex
layouts. Image-only pages have no selectable text. Password-protected PDFs open through a masked prompt; incorrect passwords can be
retried, and Esc cancels. Multiple protected files queue their prompts without
clearing an entry in progress. Ordinary Save As preserves the original encrypted
bytes. Page edits and marked copies warn before creating an **unencrypted** result;
qpdf prepares an unlocked working snapshot on demand for those operations and
printing. Passwords stay in process memory until the tab closes and never enter
command arguments, settings or recovery metadata. Recovery of a protected draft
requires its password again. Single-line UTF-8 passwords up to 4096 bytes are
accepted; pending protected files are limited to 16 / 256 MiB, and unlocked working
snapshots to 256 MiB. Password prompts for additional page-operation input files
remain future work; open those separately and extract an appropriate copy first.

Theme, sidebar visibility,
view layout, printer name, and print scaling persist in `preferences.json` under
`$XDG_CONFIG_HOME/dPDF` (or `~/.config/dPDF`) on Linux and `%APPDATA%\dPDF` on Windows.
`DPDF_CONFIG_DIR` can override that directory. PDF paths and search text are not
stored there. Invalid or newer-format preferences are left intact and defaults
are used for the session.

## Architecture and validation

- [Product specification](docs/PRODUCT_SPEC.md)
- [Current-state audit and implementation plan](docs/planning/CURRENT-STATE-2026-11-13.md)
- [Development and release workflow (pull requests, CI, release checkout)](docs/DEVELOPMENT_WORKFLOW.md)
- [Requirements and implementation matrix](docs/REQUIREMENTS_MATRIX.md)
- [Local handoff audit / working without the owner's machines](docs/LOCAL_HANDOFF_AUDIT.md)
- [Architecture and integrity boundaries](docs/ARCHITECTURE.md)
- [Milestone plan: v0.2.0 / v0.4.0 / v0.6.0](docs/MILESTONES.md)
- [Repeatable sample workflows](docs/TESTING.md)
- [Third-party runtime and license notes](docs/THIRD_PARTY.md)

The owner repository is [w3fans/dPDF](https://github.com/w3fans/dPDF).
Editable source stays in `/zDev/dPDF`; Git commits and pushes run only from the
configured `/app/dPDF/source` build/release checkout. Development also happens through
focused GitHub pull requests validated by hosted Linux and Windows CI; see
[the development workflow](docs/DEVELOPMENT_WORKFLOW.md). `./release.sh upstream` brings
merged pull requests into the editable source and release checkout; checkpoints and
previews refuse to run until it has. When a meaningful batch of work is ready,
`./release.sh patch` calculates the
next 0.1.x patch with the installed standard controller, synchronizes version files,
runs checks and an optimized build, and atomically pushes main plus its annotated
tag. GitHub Actions publishes a development preview with source archives. Stable
v0.2.0 remains gated on reader acceptance.

For intermediate work, `./release.sh checkpoint` synchronizes through the installed standard `rsyn`
transport, runs standard validation/hooks and lease guards, then commits/pushes
without changing a version or creating a release tag. The SSH origin uses
`github-w3f` and the owner's `w3fans` key from `${HOME}/.ssh/config`.
