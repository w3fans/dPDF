# Milestones and release gates

Requirements: [PRODUCT_SPEC.md](PRODUCT_SPEC.md). Per-requirement status and evidence:
[REQUIREMENTS_MATRIX.md](REQUIREMENTS_MATRIX.md).

Development continues in **0.1.x** (published preview v0.1.54). The milestone numbers below
name stable outcomes. Standard release tools calculate development release versions;
0.2.0 is held until the first section passes acceptance. No milestone is complete
merely because its UI or build scaffold exists.

These are stable-milestone acceptance gates. Qualified development patch releases
continue through the existing release workflow while additional acceptance or
unrelated development remains outstanding; see
[development release cadence](DEVELOPMENT_WORKFLOW.md#when-to-cut-a-preview).
An audit recommendation to stabilize does not impose a blanket release freeze.

The active public repository uses public PR CI and the established controlled
build hosts. Published v0.1.54 evidence is in [RELEASE-STATUS.md](RELEASE-STATUS.md);
earlier hosted evidence retains its original version and scope
([build infrastructure](BUILD-INFRASTRUCTURE.md)). Automated synthetic evidence
never satisfies the physical-printer, Windows test machine or real-user gates below.
Open owner-side gates are tracked as GitHub issues with acceptance checklists:
physical printing (#3), Windows test machine Windows 11 (#4), Scenario A (#5), Scenarios B/C (#7);
development items: compact print jobs (#6), Secure Signature Block (#8).

## v0.2.0 — stable fast reader

Implemented in the initial preview: Rust/Slint shell, PDFium worker, tabs, page and
thumbnail rendering, bookmark destinations, page jumps, single/continuous/facing
layouts, fit/actual/zoom, view rotation, light/dark themes, occurrence search/highlights, pointer-anchored Ctrl+wheel zoom,
unchanged-byte Save As, CUPS submission, and an optional Windows print helper.

Remaining gates:

Installed Fedora preview evidence is recorded in
[reader desktop qualification](validation/reader-desktop-preview.md): native
Wayland diagnostics, normal-session clipboard interoperability, desktop-entry
launch, generated long-document measurements and GTK print-to-file. This does
not close the real compositor input, physical-printer or Windows gates below.

- Validate drag-and-drop, keyboard behavior, focus, scroll, tabs, malformed files,
  and resize/HiDPI behavior on Fedora and Windows. Native X11 multi-file drops,
  cancelled hovers and keyboard tab switching now have a protocol regression;
  Wayland drops are implemented in v0.1.51 and v0.1.54 native Wayland smoke is
  recorded; real compositor source behavior and Windows Explorer acceptance remain.
- Validate search occurrence highlighting and wheel zoom on Windows/HiDPI; improve text selection.
- Fedora native GTK printer discovery/dialog, ranges/current page and physical
  fit/actual-size output are implemented and tested to files. The Windows native
  system-dialog/GDI adapter passed native hosted Windows Print to PDF checks in
  v0.1.17 (all/current/ranges, fit/actual, cancellation and source protection).
  Qualify Windows 11 on Windows test machine and physical printer output on both platforms. The optional
  Windows Quick Print helper does not establish native acceptance.
- Benchmark startup/first frame/navigation/zoom/thumbnail memory on representative
  office PDFs; tune metadata loading, high zoom and very long documents as needed.
- Native Windows test machine builds and scripted UI checks have passed earlier previews; hosted
  Windows now validates exact release commits automatically, and pull requests run
  hosted Windows plus Linux CI with all corpora and private-Xvfb suites. The Windows test machine
  runner has never been online; the last native Windows test machine pass is v0.1.14. Complete remaining
  real desktop interactions and Windows test machine Windows 11 print qualification.
- Stage complete runtimes with matching PDFium and license notices; validate packaged
  launch/file association paths and document any security/compatibility constraints.
  Fedora user-level registration now has an isolated Gio launch/argv/MIME test;
  v0.1.54 Windows MSI/portable and Fedora installed qualification are recorded;
  manual installer/default-app and real file-manager acceptance remain.
- Record Scenario A results on both platforms, then qualify the stable release
  using the user's standard tools. No 0.2.0 tag before this.

## v0.4.0 — page operations and visual signing

The preview now has replace/insert/remove/extract/merge/rotate/reorder copy export
with qpdf and an immutable source-page plan, including PNG/JPEG scan inputs
and thumbnail drag ordering.
Contract replacement is
checked for exact untouched content-stream and render preservation. This does not
complete v0.4.0: cross-platform and external-viewer acceptance
and representative real-office qualification remain.

Custom PDF page scaling (`/UserUnit` other than 1) was a reproduced millimetre-size
defect (a 40-mm mark on a `/UserUnit 2` page became 80 mm; see
[the generated baseline](validation/pdf-user-units.md)). It is resolved: v0.1.35
prints, v0.1.36 replaces pages, v0.1.40 lays out the reader and v0.1.41 exports
marks at the requested physical size on custom-unit pages, with the 192-case
overlay corpus and native print checks on units 1/2/0.5. Printed-ruler acceptance
remains part of Scenario C.

The graphical mark studio now supports file/clipboard import, crop/deskew and
background cleanup, library browsing and millimetre-size persistence. On-page
placement/resize/opacity and simple overlay export are implemented and have passed
Fedora X11/Wayland diagnostics. Windows test machine v0.1.5 passes the prescribed placement/studio
checks; its newly reported alias-overwrite issue is corrected separately.
Per-tab draft placement Undo/Redo is now implemented; it does not edit an already
exported ordinary PDF image. Local idle checkpoints now recover placed drafts and
the immutable opened PDF after interruption; cross-platform recovery qualification
and real-office acceptance remain.

Remaining qualification and repairs (existing page/mark features above are implemented):

1. Add protected secondary-source prompts and preserve source identity on page edits (T07).
2. Qualify replacement, insert/remove/reorder/extract/merge/rotate with annotations,
   forms, bookmarks and links in external viewers (T07/T10).
3. Exercise real scanner/phone images, cleanup/transparency, library sizing and
   mixed recovery across platforms (T10).
4. Measure printed physical sizes and record owner Scenarios B/C (T09/T20).

Acceptance: Scenarios B/C pass in dPDF and common external viewers. Pages 1–9 retain
original vector/text content and resource fidelity when page 10 is replaced. Save
failure cannot damage the source; existing digital signatures receive a clear
invalidation warning before modification.

## v0.6.0 — secure signature blocks and review revisions

The preview now implements independent review notes/highlights/underlines, revision
sets and visibility/filter controls, native annotation/embedded-metadata copy
export, mixed/remarks-only crash recovery on Fedora, and a separate Undo/Redo
history for remark edits. Native Windows review input, printing and broader
interoperability remain qualification work.

The generated combined-block prototype now removes contained live labels/lines/date
and two synthetic assets. The historical Python prototype inserts one 350-DPI
image and records independent Poppler checks; the current Rust probe uses a
400-DPI constant (`src/block_probe.rs::BLOCK_DPI`). Historical evidence retains
its original settings and does not qualify a production backend. It refuses unsupported structures and remains
outside the production app; see docs/validation/combined-block-prototype.md.

First prove secure region replacement using the architecture's investigation gate.
Do not ship a misleading covering rectangle as redaction.

Deliver manual rectangle selection/move/resize, safety margins, anchors and saved
PZB/IDZ-style templates, print-quality baking, and explicit searchability/integrity
warnings. Plan a separate digital-seal interface without claiming cryptographic
signing is already available.

Retain the implemented native note/highlight/underline remarks, revision sets,
global visibility, all/current/selected filters, persistence and clean versus
remark-bearing exports. Qualify external-viewer round trips and native input
(T11), then verify independence from production baking (T18).

Acceptance: Scenarios D/E pass; extraction/editor checks show no independent live
signature/label content inside the replaced block; outside text is preserved;
remarks can be filtered/hidden/exported without affecting baked signatures.
