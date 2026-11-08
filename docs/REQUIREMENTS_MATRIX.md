# Requirements and Implementation Matrix

Current preview: **v0.1.49** (commit `1c80fd7`, 2026-11-06), with
[targeted installed-package qualification](validation/fedora-copr-v0.1.49.md).
Additional [installed reader/desktop evidence](validation/reader-desktop-preview.md)
covers the Fedora 45 acceptance workstation, normal GNOME clipboard bridge,
generated 50/300/1000-page measurements and native print-to-file. It does not
grant current Windows, physical-printer or owner acceptance.
Full application/printing baseline remains **v0.1.48** (`dff7afb`); runtime source
and locked dependencies are unchanged except for dPDF's version. Unmerged work is explicitly
marked pending; earlier evidence remains qualified only for its recorded version. Requirement IDs and wording
are in [PRODUCT_SPEC.md](PRODUCT_SPEC.md) (source: owner Build Prompt v4 plus
later owner decisions). Update this file with every release that changes a
row's state or evidence.

## Status vocabulary

| State | Meaning |
| --- | --- |
| Not started | No implementation. |
| Prototype | Generated-fixture or diagnostic code only; not reachable as a product feature. |
| Implemented | Reachable in the app; partial or no automated coverage. |
| Automated tests passed | Implemented and covered by the release gate's automated checks. |
| Platform qualified | Automated plus real interactive/hardware checks on **both** Fedora and native Windows (Windows test machine). |
| User accepted | Owner accepted it in real use. |

Evidence columns:

- **Linux:** the normal Fedora release gate run by `./release.sh patch` passed
  68 Rust release tests, formatting, strict Clippy in both configurations and the
  CLI/PDF corpora for this baseline. Installed-package records separately cover
  the full application and real-window suites on a private X11 display at scales
  1/1.5/2; they do not qualify Wayland or physical printers.
- **Linux CI:** public repositories retain independent hosted validation. Private
  compilation and regressions use the existing controlled Linux adapter; private
  hosted compiler jobs are deliberately disabled. Local source-policy and
  release checks remain required. A skipped hosted job is not a PASS. See
  [BUILD-INFRASTRUCTURE.md](BUILD-INFRASTRUCTURE.md).
- **Hosted Win:** the earlier hosted service/desktop lanes passed for v0.1.42 and
  v0.1.43. Private hosted Windows compilation is now disabled. Historical Windows
  cells below retain their recorded evidence; they do not claim a current-tag
  native pass. Current acceptance uses the controlled native harness and remains
  PENDING until its exact-source receipt exists; see [WINDOWS-CI.md](WINDOWS-CI.md).
- **Windows test machine:** the owner's Windows 11 machine, tested natively. The last full native Windows test machine
  pass was **v0.1.14** (`docs/validation/windows-v0.1.14.md`); v0.1.16 was deferred.
  Current private previews use the standalone controlled harness; no current-tag native receipt has been verified.
- **User:** no row is owner-accepted yet. Real-user acceptance is pending throughout.

## Platform and UI

| ID | Milestone | Status | Main files | Automated tests | Linux | Hosted Win | Windows test machine | User | Remaining / blocker |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| R-PLAT-1 native Fedora + Windows | all | Automated tests passed | `build`, `build.ps1`, `Cargo.toml` | gate build, fmt, Clippy (default + Windows print feature) | ✓ | ✓ MSVC | ✓ v0.1.14 | – | Windows test machine retest of current tag |
| R-PLAT-2 Rust + Slint | all | Implemented | `ui/*.slint`, `src/app.rs` | — | ✓ | ✓ | ✓ | – | — |
| R-PLAT-3 PDFium pinned | all | Automated tests passed | `src/pdf.rs` | `check-engine-paths.py` | ✓ | ✓ | ✓ v0.1.14 | – | PDFium 7881 pin; security update policy |
| R-PLAT-4 qpdf assembly | v0.4.0 | Automated tests passed | `src/assembly.rs` | `check-pages.py`; qpdf 11 job-syntax unit test | ✓ (12.3.2 gate; 11.9 Linux CI) | ✓ (12.4.2) | ✓ v0.1.14 | – | Current Windows packaging stages the pinned qpdf helper and its notices; native current-tag acceptance is pending. qpdf 11 support was broken (job JSON `rotate` list) until the PR-CI fix; mixed rotation angles in one export need qpdf 12 |
| R-PLAT-6 native integrations | v0.2.0 | Implemented | `src/platform.rs`, `packaging/` | registration checks | ✓ user-level launcher | ✓ registration (15 checks) | – | – | Installer; default-app UX |
| R-PLAT-7 packaging | v0.2.0 | Implemented | `scripts/stage-linux.sh`, `packaging/windows/*.ps1`, `packaging/windows/dpdf.wxs`, `.github/workflows/windows-installer.yml` | `check-windows-stage.py`, `check-desktop.py`, `test-msi.ps1` (install/run/uninstall) | ✓ source-built F44/F45 RPMs; installed preview qualification | v0.1.45 MSI + portable ZIP published; current tag pending | – | – | Code signing; current native Windows/user install acceptance; [qualified Fedora preview](validation/fedora-copr-v0.1.48.md); private GitHub history/visibility review remains separate |
| R-UI-1 layout | v0.2.0 | Implemented | `ui/workspace.slint` | `--smoke-ui` | ✓ | ✓ | ✓ v0.1.14 | – | Confirm status bar against R-UI-4 |
| R-UI-2 toolbar | v0.2.0 | Implemented | `ui/workspace.slint` | `--smoke-ui` | ✓ | ✓ | – | – | Remarks toggle and revision selector live in the Review panel rather than the main toolbar; signatures under "Marks…" |
| R-UI-3 sidebar | v0.2.0 | Automated tests passed | `src/app.rs` | thumbnails, bookmarks, remarks list in smoke | ✓ | ✓ | – | – | Library is a separate window |
| R-UI-4 status bar | v0.2.0 | Automated tests passed | `ui/workspace.slint` (`status-detail`), `src/app.rs` | smoke asserts page/total, view mode and zoom; review UI suite asserts hidden remarks and active revision | ✓ installed Fedora UI | current native result pending | – | – | Owner visual acceptance; current native Windows qualification |
| R-UI-5 themes | v0.2.0 | Automated tests passed | `ui/*.slint`, `src/about.rs` | dark smoke, About light/dark | ✓ | ✓ | ✓ v0.1.14 | – | — |
| R-UI-6 shortcuts | v0.2.0 | Implemented | `ui/workspace.slint` | partial (wheel zoom, keyboard tab switch, Undo/Redo) | ✓ partial | ✓ partial | – | – | Systematic shortcut test; Ctrl+Shift+S = same Save As path |

## Reading (v0.2.0)

| ID | Status | Main files | Automated tests | Linux | Hosted Win | Windows test machine | User | Remaining / blocker |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| R-READ-1 fast open / CLI / Open with | Automated tests passed | `src/main.rs`, `src/pdf.rs` | `check-reader.py`, `benchmark-reader.py`, registration | ✓ | ✓ | ✓ v0.1.14 | – | Long/very large document benchmarks |
| R-READ-2 drag-and-drop | Automated tests passed | `src/app.rs` | `check-file-drop.py` (X11 XDND), Windows OLE drop diag | ✓ X11 | ✓ desktop lane, 3 scales | – | – | Wayland compositor and real Explorer drops |
| R-READ-3 tabs | Automated tests passed | `src/app.rs` | `check-tabs.py`, smoke | ✓ | ✓ | ✓ v0.1.14 | – | — |
| R-READ-4 smooth scroll | Automated tests passed | `src/document.rs`, `src/cache.rs` | `check-viewport.py` | ✓ | ✓ | – | – | Subjective acceptance |
| R-READ-5 single/continuous/facing | Automated tests passed | `src/document.rs` | unit tests, smoke | ✓ | ✓ | ✓ v0.1.14 | – | — |
| R-READ-6 fit/actual/zoom/wheel | Automated tests passed | `src/app.rs`, `src/document.rs` | smoke Ctrl+wheel, editable zoom field | ✓ | ✓ | – | – | Tiled high-zoom rendering (bitmap capped at 3072×4096) |
| R-READ-7 thumbnails | Automated tests passed | `src/app.rs` | smoke | ✓ | ✓ | ✓ v0.1.14 | – | — |
| R-READ-8 bookmarks | Automated tests passed | `src/pdf.rs`, `src/app.rs` | smoke bookmark navigation (v0.1.43) | ✓ | ✓ | – | – | Pointer click on a row is manual |
| R-READ-9 page entry | Automated tests passed | `src/app.rs`, `ui/workspace.slint` | smoke slow entry, invalid/clamped (v0.1.43) | ✓ | ✓ | – | – | v0.1.43 fixed resets while typing |
| R-READ-10 search | Automated tests passed | `src/pdf.rs` | `check-reader.py` (10,000-match cap) | ✓ | ✓ | ✓ v0.1.14 | – | HiDPI acceptance |
| R-READ-11 rotate view | Automated tests passed | `src/document.rs` | unit tests, smoke | ✓ | ✓ | ✓ v0.1.14 | – | — |
| R-READ-13 text selection/copy | Automated tests passed | `src/pdf.rs`, `src/app.rs` | `check-reader.py`, `check-clipboard.py` (X11), native Wayland reader + independent wl-paste | ✓ X11 and GNOME's normal Xwayland clipboard bridge | ✓ historical | – | – | Current Windows clipboard; GNOME without the bridge unsupported by pinned backend |
| Physical page units (`/UserUnit`) | Automated tests passed | `src/pdf_units.rs` | `check-physical-reader.py`, `check-page-units.py` | ✓ | ✓ | – | – | Fixed in v0.1.40; earlier docs describe it as a defect |

## Printing (v0.2.0)

| ID | Status | Main files | Automated tests | Linux | Hosted Win | Windows test machine | User | Remaining / blocker |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| R-PRN-1 native dialog | Automated tests passed | `src/native_print.rs`, `src/windows_print.rs` | `check-native-print.py` (GTK), `check-windows-print.py` | ✓ GTK, print to file | ✓ GDI, Microsoft Print to PDF | – | – | **Gate:** Windows test machine Windows 11 print qualification |
| R-PRN-2 current/range | Automated tests passed | `src/print_plan.rs`, `src/platform.rs` | unit tests; smoke invalid-range refusal (v0.1.43) | ✓ | ✓ | – | – | — |
| R-PRN-3 fit/actual physical scale | Automated tests passed | `src/print_plan.rs` | native print geometry incl. UserUnit | ✓ to file | ✓ to PDF | – | – | **Gate:** physical paper output on both platforms |
| R-PRN-4 remember printer | Automated tests passed | `src/preferences.rs` | preferences smoke | ✓ | – | – | – | — |
| R-PRN-5 reliable on Linux + Windows | Implemented | — | — | partial | partial | – | – | **v0.2.0 blocker:** real printers on both platforms |
| R-PRN-6 fast/compact jobs | Implemented on eligible Linux CUPS queues; overall qualification incomplete | `src/native_print/direct_pdf.rs`, `scripts/check-native-cups.py` | [CUPS/file regressions](validation/native-cups-preview.md), [published RPM qualification](validation/fedora-copr-v0.1.48.md) | COPR F44/F45: 34 app + 9 CUPS checks each in v0.1.48; current private checks use local gates | baseline measurements only | – | – | Direct PDF for all/single/contiguous pages and private vector subsets for disjoint ranges on configured PDF queues; preparation failures/advanced layouts retain renderer and confirmation. Virtual-file output remains rasterized. Windows optimization and physical-office-printer measurement remain |

## Page operations (v0.4.0)

| ID | Status | Main files | Automated tests | Linux | Hosted Win | Windows test machine | User | Remaining / blocker |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| R-PAGE-1 insert PDF/image | Automated tests passed | `src/assembly.rs`, `src/image_pages.rs` | `check-pages.py` | ✓ | ✓ | ✓ v0.1.14 | – | External-viewer check |
| R-PAGE-2 remove | Automated tests passed | `src/pages.rs` | `check-pages.py` | ✓ | ✓ | ✓ v0.1.14 | – | — |
| R-PAGE-3 drag reorder | Automated tests passed | `src/app.rs` | smoke thumbnail drag/edge scroll | ✓ | ✓ | – | – | Real pointer acceptance |
| R-PAGE-4 extract | Automated tests passed | `src/assembly.rs` | `check-pages.py` (clean catalog) | ✓ | ✓ | ✓ v0.1.14 | – | — |
| R-PAGE-5 merge | Automated tests passed | `src/assembly.rs` | `check-pages.py` | ✓ | ✓ | ✓ v0.1.14 | – | — |
| R-PAGE-6 replace with PDF/scan | Automated tests passed | `src/assembly.rs`, `src/image_pages.rs`, `src/page_bookmarks.rs` | `check-pages.py` (EXIF, alpha, JPEG passthrough, UserUnit, bookmarks) | ✓ | ✓ | ✓ v0.1.14 | – | Representative real-office files |
| R-PAGE-7 rotate pages | Automated tests passed | `src/pages.rs` | `check-pages.py` | ✓ | ✓ | ✓ v0.1.14 | – | — |
| R-PAGE-8 contract page 10 | Automated tests passed | as above | exact raw streams and pixels on pages 1–9 | ✓ | ✓ | ✓ v0.1.14 | – | **Gate:** Scenario B in external viewers |

## Signatures and stamps (v0.4.0)

| ID | Status | Main files | Automated tests | Linux | Hosted Win | Windows test machine | User | Remaining / blocker |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| R-SIG-1 import file/clipboard | Automated tests passed | `src/mark_studio.rs` | `check-marks.py`, `check-clipboard.py --marks` | ✓ X11 | ✓ | ✓ v0.1.14 (file) | – | Windows/Wayland clipboard by hand; camera import later |
| R-SIG-2 library + metadata | Automated tests passed | `src/marks.rs`, `src/storage.rs` | `check-marks.py`, asset lock tests | ✓ | ✓ | ✓ v0.1.14 | – | — |
| R-SIG-3 cleanup studio | Automated tests passed | `src/mark_studio.rs`, `src/imaging.rs` | studio smoke (crop drag, deskew, save) | ✓ | ✓ | ✓ v0.1.14 | – | Real phone/scanner images |
| R-SIG-4 sizing mm/aspect/opacity/default | Automated tests passed | `src/placement_ui.rs`, `src/marks.rs` | smoke placement, Undo/Redo | ✓ X11 + Wayland diag | ✓ | – | – | Printed-ruler check |
| R-SIG-5 physical size, not pixels | Automated tests passed | `src/overlay.rs`, `src/pdf_units.rs` | `check-overlays.py` (192 cases incl. UserUnit 1/2/0.5) | ✓ | ✓ | – | – | — |
| R-SIG-6 preview + simple overlay export | Automated tests passed | `src/overlay.rs`, `src/app/ui_smoke.rs` | `check-overlays.py`, smoke export | ✓ | ✓ | ✓ v0.1.14 | – | **Gate:** Scenario C |
| R-SIG-7 works in common viewers | Implemented | — | Poppler cross-check for some cases | partial | – | – | – | External-viewer qualification |

## Secure Signature Block (v0.6.0)

Production baking is **disabled**. No row below may be read as production-ready.

| ID | Status | Main files | Automated tests | Linux | Hosted Win | Windows test machine | User | Remaining / blocker |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| R-SSB-1 modes | Simple overlay: Automated tests passed. Secure baked region: **Prototype**. Digital seal: Not started | `src/overlay.rs`, `src/block_probe.rs` | `--probe-block` generated cases | ✓ prototype | ✓ prototype | – | – | Production worker/UI after preflight audit |
| R-SSB-2 true content removal | **Prototype** | `src/block_probe.rs`, `scripts/probe-combined-block.py` | `check-block-rust.py`: live label/line/date and assets removed; outside pixels unchanged (PDFium and Poppler) | ✓ generated only | ✓ generated only | – | – | Arbitrary real contracts unqualified |
| R-SSB-3 outside/other pages untouched | **Prototype** | as above | exact untouched-page streams/pixels, 32 geometry cases | ✓ | ✓ | – | – | — |
| R-SSB-4 region UI, margin, anchors | Not started | — | — | – | – | – | – | Design after the preflight is complete |
| R-SSB-5 print-quality bake | **Prototype** | `src/block_probe.rs` | 350 DPI quality probe | ✓ | – | – | – | — |
| R-SSB-6 templates | Not started | — | — | – | – | – | – | — |
| R-SSB-7 conservative refusal | **Prototype** | `src/block_probe.rs` | 14 generated refusal identities (catalog, page tree, resources, stream sharing, aliases) before mutation | ✓ | ✓ | – | – | Graph preflight not a general proof; clips/masks/marked content unqualified |
| R-SSB-8 bake warnings | Not started | — | — | – | – | – | – | Signature-invalidation warnings exist for other edits (see R-INT-4) |
| R-SEAL-1/2 digital seal | Not started | — | — | – | – | – | – | Separate interface; no legal claims |

## Remarks and revisions (v0.6.0)

| ID | Status | Main files | Automated tests | Linux | Hosted Win | Windows test machine | User | Remaining / blocker |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| R-REM-1 note/highlight/underline | Automated tests passed | `src/review.rs`, `src/app.rs` | `check-review.py`, `check-review-ui.py` (incl. remark Undo/Redo) | ✓ X11 + Wayland diag | ✓ | ✓ v0.1.14 (backend) | – | Callout marker not started. Remark Undo/Redo (separate history, Review-panel focus rule) added after v0.1.44 |
| R-REM-2 global on/off | Automated tests passed | `src/review.rs`, `src/app.rs` | `check-review-ui.py` | ✓ | ✓ | – | – | — |
| R-REM-3 revision sets + filters | Automated tests passed | `src/review.rs` | `check-review.py` | ✓ | ✓ | ✓ v0.1.14 | – | — |
| R-REM-4 annotations + metadata | Automated tests passed | `src/review.rs` | native annotations, catalog metadata, foreign/thread preservation | ✓ | ✓ | – | – | External-viewer interop |
| R-REM-5 export variants | Automated tests passed | `src/review.rs` | clean/current/mixed exports | ✓ | ✓ | – | – | Flatten-remarks copy is a future option |
| R-REM-6 independence | Automated tests passed | `src/review.rs`, `src/overlay.rs` | independent marks + remarks in review UI suite | ✓ | ✓ | – | – | Re-check once baking exists |

## Sharing, performance, integrity

| ID | Status | Main files | Automated tests | Linux | Hosted Win | Windows test machine | User | Remaining / blocker |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| R-SHR-1 email with attachment | Not started | — | — | – | – | – | – | Deliberately deferred: no reliable cross-platform attach |
| R-SHR-2 reveal / copy path | Automated tests passed | `src/app.rs`, `src/platform.rs` | smoke share panel, unsaved refusal | ✓ | ✓ | – | – | — |
| R-PERF-1/2 launch, zoom, scroll | Automated tests passed | `src/pdf.rs`, `src/cache.rs`, `src/document.rs` | `benchmark-reader.py`, viewport virtualisation | ✓ | – | – | – | Representative office-PDF and long-document benchmarks |
| R-PERF-3 bounded memory | Automated tests passed | `src/cache.rs` | `benchmark-buffers.py` (shared reader) | ✓ | – | – | – | — |
| R-INT-1 preserve untouched pages | Automated tests passed | `src/assembly.rs` | exact streams/pixels | ✓ | ✓ | ✓ v0.1.14 | – | — |
| R-INT-2 atomic, source never damaged | Automated tests passed | `src/storage.rs`, `src/pdf.rs` | `check-aliases.py` (hard links, symlinks, UNC), failed-save smoke, unwritable Save As (v0.1.43) | ✓ | ✓ | ✓ v0.1.14 | – | — |
| R-INT-3 recovery | Automated tests passed | `src/recovery.rs`, `src/snapshot.rs`, `src/app.rs` (Privacy panel) | `check-recovery.py` (incl. disable/keep and delete-all while off and on); recovery unit tests | ✓ (+ Linux CI) | ✓ | ✓ v0.1.14 (adapted) | – | User-facing disable/delete-all controls added after v0.1.43: native Windows desktop-lane result and owner check of the Privacy panel pending. Drafts remain unencrypted local files by design |
| R-INT-4 warnings | Implemented | `src/app.rs`, `src/overlay.rs` | — | ✓ | – | – | – | Dialogs for review/assembly/mark exports on signed PDFs; flatten warnings wait for SSB |
| R-INT-5 view-only / exact Save As | Automated tests passed | `src/pdf.rs` | unit test; byte-identical Save As smoke (v0.1.43); encrypted Save As | ✓ | ✓ | – | – | — |
| Password-protected PDFs | Automated tests passed | `src/encrypted.rs` | `check-passwords.py` (AES-256/128, retry, queue) | ✓ | ✓ | ✓ v0.1.10 (adapted) | – | Native password entry by hand on Windows |

## Milestone summary

| Milestone | State | Open gates |
| --- | --- | --- |
| v0.2.0 | Reader features implemented with current Fedora and historical Linux CI/hosted-Windows evidence; status bar complete | Physical printer output on Fedora and Windows (#3); Windows test machine Windows 11 desktop and print qualification (#4); Wayland/Explorer drops; installer qualification (Windows MSI published in v0.1.44; Fedora 44/45 source-built preview installation qualified; see the linked package record); long-document performance; Scenario A recorded on both platforms (#5); owner acceptance |
| v0.4.0 | Page operations and simple-overlay signatures implemented and tested; qpdf 11 compatibility restored | Scenarios B/C in external viewers; real scans and printed-ruler size check (#7); Windows test machine retest |
| v0.6.0 | Remarks/revisions implemented and tested, including remark Undo/Redo; Secure Signature Block is a prototype only | Production preflight/worker; region UI, margins, anchors, templates; warnings; Scenario D in independent editors (#8); digital seal design |
