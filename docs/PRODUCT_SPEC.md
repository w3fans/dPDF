# dPDF Product Specification

Canonical product requirements for dPDF. This consolidates the owner's original
build specification ("dPDF Build Prompt v4", 2026-10-28) with later owner
decisions. Requirement IDs (`R-…`) are tracked in
[REQUIREMENTS_MATRIX.md](REQUIREMENTS_MATRIX.md); acceptance gates are in
[MILESTONES.md](MILESTONES.md), test procedures in [TESTING.md](TESTING.md) and
the implementation design in [ARCHITECTURE.md](ARCHITECTURE.md).

A requirement listed here is a product intent. It is not a claim that the feature
exists or is qualified; implementation status lives only in the matrix.

## 1. Product summary

dPDF is a lean, extremely fast desktop PDF workspace for **Fedora Linux** and
**Windows**. It is focused on:

- very fast opening, rendering, zoom, navigation and printing;
- practical page operations: insert, remove, reorder, replace, merge, extract;
- visual signature and stamp workflows for real contracts;
- a **Secure Signature Block** that bakes a small region so a signature cannot be
  trivially removed;
- separate review remarks with visibility control and revision sets.

Primary user goal: receive a contract by email, open it instantly, optionally
insert a stamp/signature, optionally replace pages, optionally add review remarks,
then save and send it back with minimal friction.

dPDF is **not** an Acrobat clone and **not** an enterprise document-management
system.

### Priorities, in order

1. **Speed**: open, render, zoom and print fast.
2. **Simplicity**: minimal, compact UI; no bloat.
3. **Practical editing**: just enough PDF manipulation for real office work.
4. **Tamper resistance for visual signatures**: make casual removal or
   replacement of a placed signature difficult.
5. **Cross-platform reliability**: Fedora and Windows first.

Prefer a solid, maintainable implementation and demonstrable working workflows
over feature count or speculative sophistication.

## 2. Platform and technology

| ID | Requirement |
| --- | --- |
| R-PLAT-1 | Native desktop app for Fedora (current release) and Windows 11, compiled natively on each. |
| R-PLAT-2 | Rust application core; **Slint** native UI. No Electron. |
| R-PLAT-3 | **PDFium** (via `pdfium-render`) for rendering and suitable page-level work; pinned engine version shipped with the app. |
| R-PLAT-4 | **qpdf** for page assembly, replacement, merge, extraction and preservation of untouched pages. |
| R-PLAT-5 | Rust image tooling for signature/stamp cleanup and transparency. |
| R-PLAT-6 | Native integrations: file dialogs, printing, clipboard, default-app registration, share/email where reliable. |
| R-PLAT-7 | Build scripts for both platforms (`build`/`build.sh`, `build.ps1`), a release script, and Fedora/Windows packaging/staging. |

## 3. User interface

The UI should resemble the old PDF-XChange Viewer more than Adobe Acrobat:
quick, clean, compact, keyboard-friendly and easy to understand. Restrained,
professional styling; no oversized controls or excessive whitespace.

| ID | Requirement |
| --- | --- |
| R-UI-1 | Layout: top toolbar, collapsible left sidebar, central document view, optional bottom status bar. |
| R-UI-2 | Toolbar: Open, Save/Save As, Print, Search, page-number jump, zoom out / current zoom / zoom in, Fit width, Fit page, Single page, Continuous, Two-page, Rotate view, Insert signature/stamp, Remarks toggle, Revision selector. |
| R-UI-3 | Sidebar sections: Pages/thumbnails, Bookmarks/outline, Review/remarks list, optional Signature/stamp library. |
| R-UI-4 | Status bar: current page/total, zoom level, view mode, remarks visible/hidden, active revision/review context. |
| R-UI-5 | Light and dark themes. |
| R-UI-6 | Shortcuts: Ctrl+O open, Ctrl+S save, Ctrl+Shift+S save as, Ctrl+P print, Ctrl+F search, Ctrl++ / Ctrl+- zoom, Ctrl+0 fit (convention chosen by the app), PgUp/PgDn pages, Ctrl+Tab next tab, F9 toggle sidebar. Comfortable for heavy desktop use. |

## 4. Reading and viewing (v0.2.0)

| ID | Requirement |
| --- | --- |
| R-READ-1 | Open PDFs quickly, including from the CLI and the OS "Open with". |
| R-READ-2 | Drag-and-drop open. |
| R-READ-3 | Multiple documents in tabs, with independent view state. |
| R-READ-4 | Smooth scrolling. |
| R-READ-5 | Single-page, continuous and two-page facing views. |
| R-READ-6 | Fit width, fit page, actual size; zoom via toolbar, shortcuts and Ctrl+mouse wheel. |
| R-READ-7 | Page thumbnails. |
| R-READ-8 | Bookmarks/outlines navigation. |
| R-READ-9 | Page-number entry and jump. |
| R-READ-10 | Text search. |
| R-READ-11 | View rotation. |
| R-READ-12 | Light/dark theme (see R-UI-5). |
| R-READ-13 | Text selection and copy. *(Owner-accepted addition; not in the original list.)* |

## 5. Printing (v0.2.0)

| ID | Requirement |
| --- | --- |
| R-PRN-1 | Ctrl+P printing through the platform's native print dialog. |
| R-PRN-2 | Print current page and page ranges; a selection of pages if practical. |
| R-PRN-3 | Standard scaling options: fit to paper and actual size, with correct physical scale. |
| R-PRN-4 | Remember recent printer settings if practical. |
| R-PRN-5 | Printing must work **reliably on Linux and Windows**; this is part of the v0.2.0 acceptance. |
| R-PRN-6 | Printing must be fast. A known pain point is printers whose PostScript path is slow; prefer sending compact jobs (for example the original PDF where the destination accepts PDF). *(Owner observation, 2026-11-03.)* |

## 6. Page operations (v0.4.0)

| ID | Requirement |
| --- | --- |
| R-PAGE-1 | Insert pages from a PDF or an image. |
| R-PAGE-2 | Remove selected pages. |
| R-PAGE-3 | Reorder pages by drag-and-drop in the thumbnail view. |
| R-PAGE-4 | Extract selected pages into a new PDF. |
| R-PAGE-5 | Merge one or more PDFs. |
| R-PAGE-6 | Replace a page with another PDF page or an image-derived page (scan/photo). |
| R-PAGE-7 | Rotate selected pages. |
| R-PAGE-8 | **Contract page replacement:** in a 10-page contract, replace page 10 with a scanned/photographed signed page while preserving pages 1–9 exactly where possible (qpdf-style preservation). Must be easy. |

## 7. Signatures and stamps (v0.4.0)

Two modes are supported: **simple overlay placement** (v0.4.0) and the **Secure
Signature Block** (v0.6.0, section 8).

| ID | Requirement |
| --- | --- |
| R-SIG-1 | Import from PNG, JPG/JPEG and clipboard image; camera/photo import is an optional later path. |
| R-SIG-2 | Local reusable library of signatures, stamps and initials. Each item stores name, type (signature/stamp/initials/other), preview thumbnail, original imported source, cleaned transparent version, optional notes and last-used time. |
| R-SIG-3 | Lightweight cleanup "mini-studio" on import: crop, small rotate/deskew, white-background removal that **preserves ink and stamp colours**, contrast/threshold adjustment, transparency, preview on white and on a PDF-like background. Not a full image editor. |
| R-SIG-4 | Size before insertion/baking: drag-resize on the page, numeric width/height, aspect lock, reset to original, remembered preferred default size per asset. Dimensions shown in **millimetres** (pixels optional). Opacity where useful (stamps). |
| R-SIG-5 | Imported pixel dimensions never define the printed size. Placed items have physical on-page dimensions; resample for high-quality output. |
| R-SIG-6 | Movable, resizable on-page preview before saving; simple overlay export. |
| R-SIG-7 | The exported PDF works in common PDF viewers. |

## 8. Secure Signature Block (v0.6.0, critical)

Goal: make it difficult to casually remove or replace an inserted visual
signature/stamp from the final PDF.

Concept: the user places a stamp and/or signature that often overlaps printed
labels, a signature line and a date line, then defines a **rectangular block** and
chooses **Bake Secure Signature Block**. Everything inside the rectangle (original
text, lines, stamp and signature) becomes **one combined baked graphic**. The rest
of the page stays normal, searchable PDF content.

Example: the bottom-right of a page contains a company name and "Authorized
person:", a signature line below, an overlapping stamp and signature. After
baking, that block is one image; the rest of the page is unchanged.

| ID | Requirement |
| --- | --- |
| R-SSB-1 | Export modes: **Simple Overlay** (independent placed elements; fastest, least secure); **Secure Baked Region** (selected rectangle flattened into one element; the recommended default for real use); **Secure Baked Region + Digital Seal** (optional cryptographic seal after baking; later changes invalidate it). |
| R-SSB-2 | **True content removal:** inside the rectangle, the original live content is removed/replaced and substituted by the baked result. Painting a bitmap over still-live content is **not acceptable** and must never be described as secure or as redaction. |
| R-SSB-3 | Outside the rectangle, page content is left untouched; other pages are preserved. |
| R-SSB-4 | Region definition: manual drag rectangle, resize handles, move, configurable safety margin, optional anchor presets (bottom-right, bottom-left, centre-right). |
| R-SSB-5 | Print-quality baking, around 300–400 DPI equivalent; crisp when printed. |
| R-SSB-6 | Saved templates (examples: "PZB Authorized Signature", "IDZ Signature Block") storing anchor, default width/height, margin, whether stamp/signature are included, future date/printed-name options, and default export mode. |
| R-SSB-7 | **Conservative failure:** refuse PDFs or structures whose content cannot be proven removed (shared/retained content, unsupported resources, clips/masks, metadata or catalog references). Refuse before writing anything. |
| R-SSB-8 | Clear warnings before baking that the region is flattened and loses searchability, and that existing digital signatures in the original are invalidated. |

Suggested pipeline from the original specification: define the block rectangle →
rasterize the original page region at print resolution → composite the
signature/stamp → remove or replace the original live content in that rectangle →
insert the baked image at exact coordinates → save. The reliable PDFium/qpdf
approach is an engineering investigation; see ARCHITECTURE.md and
`docs/validation/` for the current prototype and its limits.

### Digital seal (optional, later)

| ID | Requirement |
| --- | --- |
| R-SEAL-1 | Optionally apply a standards-based cryptographic signature/seal after baking; later modifications invalidate it. |
| R-SEAL-2 | Clearly distinguish a digital seal from a visual signature. Do not overpromise legal validity: present it as a document-integrity seal subject to certificate and recipient validation. |

## 9. Remarks and revisions (v0.6.0)

Remarks are a **separate feature family** from signatures, stamps and baking.

| ID | Requirement |
| --- | --- |
| R-REM-1 | Remark types: text note, highlight, underline; optional callout/comment marker. |
| R-REM-2 | Clear global **Remarks On/Off**: hide all remarks without deleting them. |
| R-REM-3 | Revision sets (Revision 1, 2, 3 …): create a new revision, assign new remarks to the current one, filter by all / current / selected revisions. |
| R-REM-4 | Persistence: standard PDF annotations for interoperability, plus app metadata for revision grouping, visibility and filtering. Reopening in dPDF restores revision context; the file stays useful in ordinary viewers. |
| R-REM-5 | Export: save with remarks visible and embedded; save remarks but hidden by the current filter; export a clean copy without remarks; flatten selected remarks into a copy is an optional future. |
| R-REM-6 | Independence: remarks never alter baked signature content, and baking never alters remarks. |

## 10. Sharing and send-back

| ID | Requirement |
| --- | --- |
| R-SHR-1 | Practical send-back: "Send via Email" with the saved PDF attached where the platform supports it reliably. |
| R-SHR-2 | Honest fallbacks where direct attachment is unreliable: reveal the saved file, copy its path, copy to clipboard if practical. No broken pseudo-feature. |

## 11. Performance

| ID | Requirement |
| --- | --- |
| R-PERF-1 | Fast launch and instant-feeling first-page display for ordinary office PDFs. |
| R-PERF-2 | Smooth zoom and scroll; responsive thumbnails; practical multi-page contracts. |
| R-PERF-3 | Sensible caching without memory bloat. |
| R-PERF-4 | Untouched pages are preserved rather than regenerated. |

## 12. Save model and document integrity

| ID | Requirement |
| --- | --- |
| R-INT-1 | Reordering/replacing pages preserves untouched pages wherever possible; a baked region modifies only the affected page content. |
| R-INT-2 | Save failure must never damage the source; edits are exported as copies with atomic replacement. |
| R-INT-3 | Backups or autosave recovery where feasible. |
| R-INT-4 | Warn clearly when an operation flattens content, reduces searchability inside a block, or invalidates existing digital signatures in the original. |
| R-INT-5 | View-only operations never modify the file; an unchanged Save As writes the original bytes. |

## 13. Out of scope (for now)

OCR suite; full text editing of arbitrary PDF content; form designer; enterprise
document management; cloud sync; multi-user collaboration server; heavy image
editor; mobile app.

## 14. Milestones and acceptance

Version numbers are fixed by the owner:

| Milestone | Focus | Acceptance (owner) |
| --- | --- | --- |
| **v0.2.0** | Fast reader: open, tabs, thumbnails, bookmarks, navigation, single/continuous/two-page, zoom, search, printing, themes, rotate view | Opens ordinary PDFs quickly; **printing works reliably on Linux and Windows**; UI is lean and responsive. Scenario A passes on both platforms. |
| **v0.4.0** | Page operations and signature/stamp basics: insert/remove/reorder/extract/merge/replace, import, cleanup studio, placement, sizing before insertion, per-asset size memory, simple overlay, library | Replace page 10 of a contract preserving pages 1–9; place a cleaned signature and stamp at the correct visual size; exported PDF works in common viewers. Scenarios B and C. |
| **v0.6.0** | Secure Signature Block, region selection and templates, secure baked region, digital-seal planning hooks; remarks, on/off, revision sets, filters, clean vs remark-bearing export | Signature/stamp overlapping printed text and lines can be baked as one block; the block is not trivially removable in common editors; the rest of the page stays searchable/selectable; remarks are shown/hidden independently of signatures and organised by revision. Scenarios D and E. |

Versioning rules (owner):

- Development proceeds as **0.1.x patch previews**. v0.2.0 is **reserved** for the
  first stable reader and is tagged only after its acceptance passes; v0.4.0 and
  v0.6.0 keep the meanings above.
- Patch numbers stay **two-digit**: no 0.1.100+. The release tooling refuses it.
- No milestone is complete because code or UI exists; it needs the acceptance
  evidence in MILESTONES.md, including real-user acceptance.

## 15. Acceptance scenarios

- **A — quick reading:** open a 50-page PDF, navigate by thumbnails, zoom, search,
  print page 7. Expanded step list in TESTING.md.
- **B — replace signed last page:** a 10-page contract; replace the last page with a
  scanned signed page; save.
- **C — direct visual signing:** import saved signature and stamp, adjust signature
  size, place both in the signature area, save as simple overlay.
- **D — secure signature block:** place stamp and signature overlapping a printed
  label and line, define the block rectangle with a small margin, use Secure Baked
  Region, save. In a common PDF editor the signature must not remain a trivially
  separable object.
- **E — review revisions:** add remarks in Revision 1, more in Revision 2; switch
  visibility and filter which revisions are visible or exported.
