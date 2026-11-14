# Architecture

The reader controller lives in `src/app.rs`. Explicit development window checks
live in `src/app/ui_smoke.rs`, with reflow cases in `src/app/viewport_smoke.rs`.
Normal launches do not run these diagnostics; the existing `--smoke-ui` command
and isolated fixture environments select them for validation.

## Current reader

`src/main.rs` handles CLI startup and the headless `--probe` acceptance path.
`ui/workspace.slint` defines the compact desktop UI. `src/app.rs` owns tabs,
per-tab view state, UI models, visible render requests, and native file dialogs.
`src/document.rs` computes page geometry without depending on the UI or PDFium.

`src/pdf.rs` owns PDFium and all PDF documents on one dedicated worker thread.
The UI sends bounded commands and polls bounded responses; no PDF engine handles
cross the boundary. Render generations discard obsolete work after a tab, zoom,
rotation, or layout change. Search walks one page at a time between foreground
requests and can be cancelled by newer searches or tab switches, including within
pages containing many matches. Search uses PDFium's native case-insensitive search
and yields individual occurrences. PageToDevice converts bounds through crop boxes
and intrinsic rotation; normalized bounds then follow view rotation/zoom. The UI
renders only viewport-adjacent highlight rectangles. Results stop at 10,000 matches
and indicate truncation rather than growing memory without a bound.

After the first frame, the existing metadata request shares one immutable qpdf
snapshot between independent review and page-unit decoders. Each tab retains raw
engine sizes and applies verified physical units atomically, preserving the active
viewport anchor. Default-unit or repeated facts do not trigger another reflow;
background facts do not change the active tab. Review errors do not discard valid
unit facts. Production mark placement waits for verified dimensions, while reading
remains available if inspection fails. The generated checks in
`src/app/physical_smoke.rs` exercise this production metadata path explicitly.
Snapshot recovery preflights physical dimensions on the PDF worker before restored
mark bounds are validated. Protected snapshots use their unlocked working bytes;
the stored source and unchanged Save As retain the original encrypted bytes.

Ctrl+wheel uses the page point under the pointer as its anchor and clamps scroll
at document edges. Ordinary wheel input continues to scroll. Tabs retain horizontal
and vertical scroll; closing a tab restores the surviving tab's view rather than
writing the closed tab's coordinates into it.

Rendering is virtualized: only viewport-adjacent page controls and images exist in
the central view. Moving to another visible page window invalidates obsolete queued
renders while retaining still-visible frames. Sidebar thumbnails are requested lazily. Offscreen UI images are released. `src/cache.rs` supplies a
64-MiB byte-budgeted LRU of worker RGBA frames, keyed by document/page/width/rotation.
Images are capped at 3072 pixels wide and 4096 high; zoom above that resolution
magnifies the bounded bitmap. This is a preview tradeoff; tiled high-zoom rendering
remains a performance/quality gate. Document input bytes, PDFium allocations, UI
copies, and queued responses are outside the frame-cache budget.

Each open document retains its input snapshot alongside the PDFium document.
Save As writes the exact input bytes via a temporary sibling file and atomic
replacement. It does not serialize through PDFium. Print uses the same input
snapshot through a private temporary PDF on a separate submission thread, so a slow
spooler does not block rendering. Printer discovery also runs independently. View-only operations do not modify
content or invalidate existing signatures. Digital signatures are detected, not
cryptographically verified by this reader.

`src/platform.rs` validates bounded page ranges and submits argument-safe commands
without a shell. Linux Quick Print uses CUPS `lp`; Windows Quick Print uses an
explicitly configured SumatraPDF helper. Toolbar/Ctrl+P printing uses separate
native GTK or Windows system-dialog/GDI children. Slint uses its Winit desktop
backend and software renderer.

PDFium 8125 is dynamically loaded from an explicit absolute environment path or
the executable directory. Linux also supports the system library loader. Windows
requires the configured/bundled PDFium DLL and never requests a bare-name search
for it through the document directory or PATH. Distribution packages must carry matching engine
binaries and third-party notices. Runtime updates need security review and matching
bindings; the compatibility pin is not a promise that an old engine stays safe.

## Reader text selection

Rectangular selections are normalized before view rotation and converted with
PDFium DeviceToPage, so crop offsets and intrinsic page rotation are respected.
Extraction stays on the PDF worker; tokens discard results from an older drag or
closed/switched document. Selection geometry survives zoom/view rotation but is
cleared when switching tabs. It never modifies PDF annotations or review data.
Ctrl+A targets the current page; text fields retain their own native copy/select
behavior. `arboard` supplies the native clipboard, retained for the application
lifetime so Linux selection ownership is not lost immediately after copying.
Normal diagnostics do not write clipboard contents. The explicit clipboard test
starts an authenticated, local-only Xvfb display and copies only generated fixture
text there; this is Linux X11 coverage, not Windows/Wayland clipboard qualification.

## PDF page operations (v0.4.0 groundwork)

`src/pages.rs` defines source-page references and validates replace/insert/remove/
extract/merge/rotate/reorder plans before mutation. `src/assembly.rs` snapshots all
inputs, submits a private qpdf job JSON file, preserves stream/object compression,
validates output structure and page count, then atomically replaces the selected
destination. qpdf 11+ is required; Fedora's 12.3.2, the Windows CI 12.4.2 and the Linux CI
Ubuntu 11.9 are tested. qpdf 11 job JSON takes one rotation string, so mixed rotation
angles in a single export are refused there with a clear qpdf 12 requirement.
qpdf warning exit codes fail the export, leaving prior output intact.

`src/pdf.rs` hands the opened document's byte snapshot to a background assembly
thread; changing its file on disk does not change the edited copy. The GUI saves
to a separate PDF and opens it as a tab. Source paths cannot be destinations. The
current workflow exports a copy per operation; it is not yet an in-memory multi-step
edit/undo workspace. PDF and PNG/JPEG page sources are supported. Reorder Pages creates an output-order
preview with native Slint DragArea/DropArea and typed internal source-document/page
payloads. Drops from another document or external applications cannot change it.
Thumbnail rendering, eviction and current-page following map output slots back to
source page IDs. Closing the panel discards the preview; Reset or a validated typed
order rebuilds it. The document view retains original page order until copy export. `src/image_pages.rs` applies EXIF orientation
and fits replacement scans to the target page; inserted/merged images use A4.
Image pixel density never defines physical page size. Large inputs are resampled
toward 300 DPI with a 4096-pixel side cap; unchanged JPEGs retain their DCT stream,
and PNG transparency is retained. Conversion uses the existing PDF worker to make
a one-page PDF snapshot for qpdf. A second PDFium instance must not be initialized
beside live reader documents. Conversion can briefly delay rendering during large
imports; qpdf export stays on its own thread. Image limits are 64 MiB compressed,
32 megapixels, and a bounded decoder allocation. Crop/deskew/page-size choice
belongs to future image UX.

qpdf retains page content/resources where possible; it may renumber/reorganize PDF
objects. Preservation is tested using exact content-stream bytes and PDFium render
checksums, not an identical whole-file serialization. Extraction uses an empty
catalog to avoid carrying unrelated outlines/attachments. Other operations preserve
the primary catalog where possible. Page slots retain their original primary-page
identity through replacement and reordering. `src/page_bookmarks.rs` uses bounded
qpdf metadata to restore affected primary outline destinations after assembly,
preserving bookmark zoom, styling and GoTo action chains. Named bookmarks become
direct destinations; general named-destination trees and page links are outside
this repair. Complex forms, bookmarks, attachments and
existing annotations still need broader compatibility checks. Page assembly is
not secure redaction. Cryptographic signature invalidation is explicit in the UI,
with an additional confirmation for signatures detected in the primary PDF.

`src/marks.rs` implements development asset storage and cleanup, shared bounded
image decoding lives in `src/imaging.rs`. Imports keep exact source bytes, clean
colored ink off a white matte, preserve alpha, crop and deskew with premultiplied
bilinear sampling, and write a thumbnail. Completed asset directories are published
by rename; partial imports remain hidden and are removed on failure. Metadata
updates are atomic. Corrupt/future schemas are rejected without rewriting them.
`src/mark_studio.rs` and `ui/marks.slint` provide a separate compact graphical
studio: file/clipboard import, crop gesture/numeric bounds, deskew and white-matte
controls, previews, library browsing and preferred physical sizes. IO/cleanup
requests run off the UI thread, previews coalesce over 150 ms, and generation
tokens discard replies from a closed window. `MarkSource` keeps original bytes
and the oriented image together; saving uses that captured source instead of
rereading a changed file/clipboard. Closing releases source/preview images.
Library scanning retains valid items and reports unavailable ones without changing
them. The strict diagnostic list command still fails on invalid metadata.
The studio can capture an immutable cleaned PNG and preferred physical size for
placement. Per-tab Drafts store physical geometry, aspect lock, opacity and four
bounded preview orientations. A view rectangle is mapped back through the view
rotation for movement/resizing; the PDF worker alone prepares affected pages.
Compressed images and preview rasters each have a 64-MiB per-document limit,
including history. Snapshots share immutable PNG masters and Slint image buffers;
retention deduplicates assets by master identity. Undo/Redo retain at most 64
changes, evicting oldest undo states when either image budget is exceeded. A whole
move/resize gesture commits once; pointer cancellation restores its starting state.
Cosmetic view changes and selections do not enter document history. New edits
clear Redo; no-op edits preserve it. Focused text input owns its native Undo/Redo and does not forward unhandled
editing or navigation keys into document history. Global save/open/search/tab
actions and Escape still run through the capture handler. Linux text Redo uses
Ctrl+Shift+Z; Windows text Redo uses Ctrl+Y, following the native widget mapping.
Exports capture a generation; success clears drafts only when that generation is
still current. Tab/window close confirms discarding drafts, and printing/page
operations require saving the marked copy first. A matching successful save clears
both histories; failed or stale saves retain them. Generations are monotonic even
through Undo, preventing a visually restored state from accepting an older export.
`src/recovery.rs` writes bounded local snapshots on a dedicated thread after a
one-second idle debounce. Snapshots include the immutable opened PDF bytes and
unique cleaned PNG masters, plus physical placement metadata; they do not reread
the potentially changed source. PDF bytes are shared with the reader only on demand.
The stream format checks metadata, lengths, counts and geometry before allocating;
restored PNG decoding and preview sizes share existing image budgets. Restored
geometry is validated against the actual opened PDF before entering a new tab.

The store uses standard [Rust file locks](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock)
to exclude competing processes. File data is synchronized before an atomic rename;
interrupted temporary writes are discarded while malformed/future checkpoints are
preserved. A recovered checkpoint is retired only after its replacement is written,
or after explicit discard/current-state PDF export. Save generations remain
independent from disk acknowledgements. At most four I/O requests are queued, and
UI submissions are nonblocking. Explicit close/discard flushes ordered removal.
A second process remains usable with recovery disabled. Limits are 128 MiB/PDF,
64 MiB/compressed images and 512 MiB/store; full stores retain prior checkpoints.
The files are local unencrypted document/image data, with a private Unix directory;
Windows inherits the user-profile ACL. Undo history and pending studio placements
are not persisted; recovering creates a new history. No power-loss guarantee is
claimed. The asset root is XDG_DATA_HOME/dPDF/marks (or ~/.local/share/dPDF/marks)
on Linux and LOCALAPPDATA/dPDF/marks on Windows; DPDF_DATA_DIR overrides the data
directory. Private asset data is never synchronized into source or Git.

Signature assets store original and cleaned images, type/name/notes, preferred
physical width/height, aspect lock, and last-used time in local application data.
A bounded I/O worker records usage only after a saved asset enters a page draft.
It captures the placement timestamp and merges it into freshly loaded metadata
under a per-asset file lock shared with preferred-size writes. Queued updates
cannot roll the timestamp backwards or overwrite newer sizes.
Metadata lock acquisition retries contention for at most five seconds on the I/O
workers; other lock errors fail immediately. A persistent lock still reports an
error, and the UI does not wait for this bookkeeping. Arming/cancelling,
Undo/Redo and recovery do not rewrite this history. Failed bookkeeping reports a
status message without discarding the placed mark; future-schema metadata is
refused without replacement.
The simple-overlay engine stores physical placement in mm (25.4/inch), normalizes
against physical page dimensions, and maps the native-rendered-page rectangle to
raw PDF coordinates through DeviceToPage, retaining intrinsic rotation and crop
offsets. Saved
custom units and physical image-vector dimensions are checked before assembly;
insufficient coordinate precision is refused without replacing the destination.
It copies only affected pages into
PDFium documents using the existing engine instance, adds ordinary alpha images,
then passes immutable single-page snapshots to qpdf with the untouched source
pages. Untouched pages preserve streams; affected pages are reserialized.
The diagnostic JSON export and later GUI placement share this implementation.
Pixel dimensions are source information, never the intended print size. Library
storage, placement undo, and cleanup are separate from PDF page assembly.

## v0.6.0 secure baking: mandatory investigation gate

`src/block_probe.rs` ports the generated experiment into Rust. Its explicit
`--probe-block` command generates its own fixtures and has no PDF/image input
argument. It owns the sole engine in that CLI process, using the same
library-path selection as the reader. Borrowed inputs, pages, temporary bitmaps
and transferred objects have explicit lifetimes; the synchronous save callback
is capped at 32 MiB and region rasters at eight megapixels/4096 pixels per axis.
It is separate from the reader worker and production export. See
[the Rust probe](validation/rust-block-probe.md) for coverage and remaining gates.

qpdf page assembly and drawing a raster rectangle are not a redaction engine.
PDFium object removal by bounding box is insufficient for partially intersecting
text/images, shared form XObjects, nested transforms, clipping, transparency,
annotations, and content extending both inside and outside the block.

Before enabling Secure Baked Region, prove a real region-replacement implementation:

1. Define the rectangle and safety margin in canonical PDF coordinates; normalize
   page crop boxes, inherited rotation, and screen-to-page transforms.
2. Render original region plus chosen signature/stamp placements at 300–400 DPI.
3. Remove the original live region through a reliable redaction/content rewriting
   backend, including intersecting nested content. Do not just cover it or delete
   whole crossing objects. Select the backend after a focused capability/license
   spike, not by assuming PDFium or qpdf supplies secure redaction.
4. Insert the combined raster at exact page coordinates, serialize to a fresh file,
   and remove unreachable old object content (no incremental-save remnants).
5. Reopen and test visual fidelity, inside-region text/object extraction, outside
   text preservation, image/form extraction, and common-editor behavior.

Fail closed if genuine region replacement cannot be established. Do not offer a
"secure" mode that simply paints over live content. A baked block can still be
edited/replaced by a determined editor: this is resistance to casual removal, not
proof of identity or cryptographic integrity. A later certificate-based seal is a
separate step; later modifications should invalidate it.

The proposed [production secure-region contract](planning/SECURE-REGION-CONTRACT.md)
defines T12's support/refusal table, source-bound preflight plan, geometry,
publication/cancellation and independent verification obligations. It is awaiting
experienced PDF review; no backend or production capability follows from it.
Initial annotation refusal explicitly leaves independent-review acceptance open.

## Review revisions remain separate

Use standard PDF note/highlight/underline annotations for interoperability, plus
versioned dPDF metadata mapping stable annotation identifiers to revision sets.
New remarks target the active revision. Global visibility and revision filters
are view state; export policy is explicit (embedded visible, retained hidden, or
clean copy). Never treat signatures as remarks or include remarks in a secure bake
implicitly. Reopening must restore revision IDs, labels, and membership.

Prototype embedded metadata and annotation flags with multiple external viewers
before selecting persistence details. A sidecar, if used temporarily, must be tied
to a document hash and never silently applied to a different document. Clean export
must remove both native remarks and associated app metadata while keeping signatures.

## Small local preferences

`src/preferences.rs` stores versioned JSON for theme, sidebar, current layout and
printer/scaling choices. Missing fields default; unknown future schemas or corrupt
files fall back without overwriting the original. UI changes are debounced and
flushed at exit. Document paths and search strings are not stored. Tests and
explicit `DPDF_CONFIG_DIR` overrides use isolated temporary directories.

`src/storage.rs` owns the shared atomic sibling-file replacement used for PDFs
and preferences. The ordinary UI smoke run disables persistent preferences;
`DPDF_SMOKE_PREFERENCES=1` enables a persistence check only with an explicit config
directory. Printer process tests use controlled mock executables, never a real queue.

## Native Fedora print process

Ctrl+P sends the opened immutable PDF bytes over stdin to a dPDF print child.
That child owns its PDFium instance and dynamically loads the system GTK 3/Cairo
printing APIs. No engine handles cross the process boundary and no GTK library is
loaded during reader startup. Rendering, printer dialog and spooler latency stay
off the reader and PDF worker. Source/path hints are passed as literal arguments;
PDF contents and passwords are never command arguments. Native printing currently
accepts snapshots up to 256 MiB, with direct CUPS available for larger inputs.

GTK's print operation owns ranges/current page, copies, orientation and page setup.
Its page callback rasterizes only requested pages at approximately 300 DPI at the
printed size, capped at 16 million pixels with 4096/8192 side limits. Cairo owns the
pixel surface so delayed PDF serialization cannot retain freed Rust buffers.
Callbacks contain Rust panics and errors; cancellation returns a cancellation
status. Library handles remain loaded until child exit because GObject registers
process-global types. A draw/source-identity guard checks a print-to-file target
before submission. Host printing uses direct GTK to keep that output URI visible;
a future sandbox package needs a separate portal-aware preservation design.

The native test forces GtkSettings' gtk-print-backends property to `file` (the
similarly named environment variable alone does not configure GTK), and forces
GDK's X11 backend on a private authenticated X display. No hardware or lpr backend
is available. Real dialog key events produce PDF files for count/color/geometry
checks and cancellation; reader Ctrl+P and tab navigation test the process boundary.

## Protected PDF opening

The PDF worker keeps a bounded queue of immutable encrypted byte snapshots when
PDFium reports a password error. A masked workspace prompt submits one password
at a time to the worker, retries incorrect values, and clears the UI field after
submit/cancel. Pending files do not reread their source on retry. Plain Save As
and recovery continue using the original encrypted bytes. Passwords are held only
in worker memory while the tab is open; recovery requires re-entry.

Editing/printing lazily obtains a qpdf-decrypted snapshot. The helper receives its
password through [`--password-file=-`](https://qpdf.readthedocs.io/en/stable/cli.html#option-password-file)
on a private stdin pipe, with bounded unencrypted PDF output read from stdout and
cached in memory. Helper diagnostics are discarded so they cannot echo a password
into logs/status. The temporary input contains only the encrypted original. No
password or decrypted working PDF is written by this preparation step. Downstream
page-edit/print jobs retain their usual private scratch/output handling. Native
printing uses that unlocked snapshot through the existing child-process boundary.
Modified copies require confirmation that their output is unencrypted; they also
retain existing digital-signature invalidation warnings. Extra encrypted merge or
replacement inputs do not yet have a password prompt. Permission/certificate and
non-password security handlers remain separate compatibility work.

## Independent review implementation

`src/review.rs` validates bounded revision/remark models with a UUID workspace
namespace. Standard Text/Highlight/Underline annotations use normal PDF coordinates
and quad points; a catalog string stores versioned revision/filter metadata,
including empty revision sets. qpdf's [partial JSON object updates](https://qpdf.readthedocs.io/en/stable/json.html)
modify annotation/catalog dictionaries while preserving page content streams.
Retained comment references stay stable so popup/reply links survive. Removal
retires owned threads and nulls their dictionaries, while unrelated annotations
remain. Metadata reads synchronize native text, position and deletion; duplicate
owned names fail safely rather than silently choosing one. Counts, strings,
metadata and subprocess output have explicit limits.

The reader first displays its page, then loads review data on the PDF worker.
For dPDF-reviewed files it prepares a private reading base with owned annotations
removed and renders separate Slint review decorations. Signatures/stamps use their
own model and layer. Review changes do not enter the PDF search/selection layer.
Current visibility is applied to prepared print snapshots. Copy export composes
ordinary marks first, then adds review annotations using geometry captured on the
single PDFium worker; background qpdf work never owns PDFium handles.

A combined monotonic draft generation guards mixed saves and recovery. Clean
export preserves the source drafts; matching current/visible exports retire them
and restore the source tab's original review baseline. Older/failed exports retain
newer drafts. Recovery metadata optionally carries reviews, including remarks-only
sessions; old mark-only checkpoints remain readable. View-only filters are separate
from remark edits. Native Windows review UI and PDF accessibility/tagging,
real-office interoperability and physical print output remain qualification work.

## Review state hardening

Current-revision filtering is an explicit model variant, distinct from a fixed
selected set. Choosing it advances review metadata to version 2; version-1 All/
Selected models remain readable. Older previews leave an unsupported version intact
and can still display its standard native annotations. Filter parsing validates
revision positions before applying visibility or metadata changes. Tab activation
clears the armed review tool and editing selection/text. Review-loading errors
are stored separately from readiness and expose a retry without disabling reading.

The shared diagnostic display helper uses isolated fixture config/data and removes
inherited DPDF_SMOKE variables before each suite supplies its own inputs. Fedora
keeps its private authenticated X display. Native Windows tests explicitly set
DPDF_TEST_NATIVE_DISPLAY=1, use native fixture windows and provide no private desktop.
Neither context enables clipboard diagnostics through inherited environment state.

## Native Windows print process

Windows toolbar/Ctrl+P follows the same bounded stdin snapshot boundary as Fedora.
`windows_print.rs` initializes COM in its print child, creates a valid hidden owner
window, and uses [PrintDlgExW / PRINTDLGEXW](https://learn.microsoft.com/en-us/windows/win32/api/commdlg/ns-commdlg-printdlgexw)
for printer/range/current-page/copy/driver choices. Driver-managed copies and
collation are enabled; dPDF renders each selected source page once to avoid
multiplying copies. Overlapping ranges are deduplicated in document order. Apply
without Print and Cancel do not submit a job.

GDI StartDoc/StartPage/StretchDIBits/EndPage/EndDoc consume top-down BGRX page
rasters at about 300 DPI, with a 16-million-pixel / 4096×8192 bound. Geometry uses
[GetDeviceCaps](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-getdevicecaps)
printable pixels and separate X/Y DPI: actual size preserves physical dimensions
and may clip; fit centers in the printable area. Halftone stretching sets its brush
origin explicitly. RAII frees dialog allocations/device contexts and aborts a
started job if rendering or spooling fails. Submission does not establish paper
output, and users can cancel submitted jobs in the Windows print queue.

[DEVNAMES](https://learn.microsoft.com/en-us/windows/win32/api/commdlg/ns-commdlg-devnames)
uses validated UTF-16 code-unit offsets. Controlled Microsoft Print to PDF,
PORTPROMPT and Print-to-File output uses dPDF's native save-file picker, followed
by an input-identity check immediately before StartDoc. Driver-owned output prompts
outside these paths are not covered by that guard; qualify such drivers separately.
Spooler file output is not dPDF's atomic Save As and can remain incomplete after a
driver failure. The diagnostic requires DPDF_TEST_NATIVE_PRINT=1, selects Microsoft
Print to PDF, and verifies that exact device before StartDoc even if the user
changes the native selection. It never submits a diagnostic to physical hardware.

The check-windows-print feature compiles this adapter during Linux metadata-only
checks with the published Win32 bindings, but does not link or run Windows APIs.
Native Windows execution, input, driver and paper-output acceptance remain open
until an exact-version Windows test machine report establishes them.

## Native file-drop adapter

Slint and winit remain pinned. In Slint 1.18.1, the winit backend does not map OS
`DroppedFile` events into `DropArea` data transfers. The main reader therefore
uses the public `slint::winit_030::WinitWindowAccessor` event hook, enabled by
`unstable-winit-030`, to queue owned paths for the existing UI timer/open flow.
It consumes each native drop once and propagates unrelated window events. Opening
is deferred to avoid re-entering `Application` borrows from a native event filter.
Existing in-app page drags and Slint file callbacks remain independent. The narrow
version-specific hook must be revisited with a Slint/winit upgrade; the native
XDND regression checks real backend delivery rather than a simulated app callback.
