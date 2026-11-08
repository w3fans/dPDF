# Windows validation — dPDF v0.1.8

Date: 2026-10-29 (Europe/Ljubljana). Validator: Codex on [private].

**Result: the v0.1.5 source-alias overwrite is fixed in the tested cases. Native build, 32 unit tests, all five executable corpora, and the reader/placement/studio scale matrix pass. No new blocker was found in these checks. Native Windows printing and full pointer-driven acceptance remain open.**

## Exact application and environment

- Tag: v0.1.8. Application commit: **f9fead4d923d459ff8b0d1ab3c102374607bb4f8**.
- Upstream main still pointed here at the final check. v0.1.6 identity protection and v0.1.7 changes were tested cumulatively through v0.1.8; older binaries were not separately qualified.
- Native checkout/build: D:\zDev\dPDF on [private], x86_64-pc-windows-msvc. No Linux worker, cross-compilation, application patch or qpdf wrapper was used.
- Existing Windows 11 Pro x64 10.0.22631 setup; Ryzen 7 5700G with 6 logical CPUs presented and approximately 8 GiB RAM.
- Rust/Cargo 1.99.1; Visual Studio Build Tools 2022 17.14.43, compiler 19.44.35229.0, Windows SDK 10.0.26100.0; PowerShell 7.6.5; Python 3.12.14.
- PDFium Windows x64 chromium/7881 and official qpdf 12.4.2 MSVC64. Locked dependency same-file 1.0.6 compiled natively. The dGK GNU toolchain was retained.
- Single-job repository build settings retained. Config/data/libraries and outputs were isolated outside source control.

Release executable SHA-256:

```text
9df928659dc5cb1726ac750cd2b0b6b1267690225710f31470b3753fed894030
```

## Build and executable results

| Check | Result |
| --- | --- |
| build.ps1 -Release | PASS; optimized native build, 2m 50s |
| cargo fmt --check | PASS |
| cargo check --locked | PASS |
| cargo test --locked | PASS: 32 passed, 0 failed, 0 ignored |
| cargo clippy --locked -- -D warnings | PASS |
| Unmodified check-aliases.py | PASS: hard links and the existing loopback UNC share; protected PDF, replacement PDF, page/overlay image, placement recipe, mark cleanup and raster outputs refused aliases; original bytes unchanged |
| Unmodified check-reader.py | PASS: search cases/counts, crop/rotation geometry, full/bounded text selection, malformed rectangles, 10,000-match cap and missing/corrupt input refusal |
| Unmodified check-pages.py | PASS with official qpdf: replacement/untouched streams and pixels, extraction/catalog cleanup, remove/reorder, insert/merge/rotate, PNG/JPEG/alpha/phone orientation, geometry, mixed inputs, invalid operations and ordinary overwrite/input-preservation cases |
| Unmodified check-marks.py | PASS: cleanup, original bytes, PNG/thumbnail, physical-size persistence, invalid dimensions/aspect and future metadata protection |
| Unmodified check-overlays.py | PASS with official qpdf: exact untouched streams/pixels, affected-page text, page/crop/mark quarter turns, physical geometry, opacity, multiple marks/pages, downsampling/JPEG orientation and failure/input protection |
| Separate Unicode/space UNC page export | PASS: generated files in “Žig čšž test”, including Pogodba ŽČŠ.pdf and Nova pogodba ŽČŠ.pdf |
| Separate Unicode/space UNC overlay export | PASS: a distinct marked copy was created through the existing loopback administrative share |
| Staged launch/probe | PASS with DPDF_PDFIUM_LIBRARY unset: staged adjacent DLL loaded, 50 pages rendered, checksum b82745673e81dddf; PDFium notices staged |

No shares, security settings or physical printer queues were created or changed.

## Original P1 reproductions — now refused

The exact drive/UNC identity distinction from the v0.1.5 report was repeated with disposable generated PDFs.

| Case | v0.1.8 observation |
| --- | --- |
| Local-drive source; its UNC alias as page-operation destination | Exit 1: “Output identifies a protected input file through another path; choose a separate file” |
| Same local-drive source; its UNC alias as overlay destination | Exit 1 with the protected-input message |
| Source preservation after both refusals | Hash unchanged; source remained 3 pages |
| Distinct UNC page/overlay destinations | Exit 0; separate copies created successfully |

This closes the previously reproduced source-overwrite blocker in these cases. The new alias corpus also checks hard links and protection of other input types; all Windows cases ran, including the loopback UNC cases rather than skipping them.

To repeat the broader regression:

```powershell
python scripts/check-aliases.py --binary target/release/dpdf.exe --pdfium C:\deps\pdfium\bin\pdfium.dll --qpdf C:\deps\qpdf\bin\qpdf.exe
```

## Reader, placement and studio scale matrix

All runs used generated fixtures, isolated config/data directories, official qpdf, DPDF_SMOKE_ASSEMBLY_SOURCE and DPDF_SMOKE_PLACEMENT_IMAGE. Each studio run used a fresh empty library.

| Application scale | Reader + assembly + placement/history | Studio fast size/save/placement |
| --- | --- | --- |
| 1 / 100% | PASS | PASS |
| 1.5 / 150% | PASS | PASS |
| 2 / 200% | PASS | PASS |

The reader smoke passed rendering/thumbnails/bookmarks, dispatched text selection, thumbnail reorder/cancel, navigation/search, continuous/facing/rotated views, wheel versus Ctrl+wheel, tab restoration and print-panel discovery.

New placement assertions passed:

- Esc cancels a pending placement.
- Dispatched page click, drag movement and corner resize update physical geometry.
- Ctrl+Z, Ctrl+Shift+Z, Ctrl+Y and Delete restore placement/opacity and recover removal.
- Numeric text-field Undo retains text-editing behavior without changing mark history.
- Numeric size/opacity, rotated preview and tab isolation remain correct; print/page operations do not silently omit unsaved marks.
- A forced failed save retains drafts/history for retry.
- An older export completing after a newer edit retains the newer draft/history.
- Current-copy export/reopen clears saved drafts/history only after success.

Studio smoke passed immutable source capture, crop/deskew, a same-event width edit followed by Save Default Size, proportional size persistence through library reopen, a fast width edit before placement, correct cleaned placement payload/target/physical size, and working-image release on close. The earlier phase-5 size failure did not recur.

App-owned 200% placement and 100% studio snapshots were visually inspected. These are renderer diagnostic snapshots, not native desktop capture. Application scale factors do not establish real Windows monitor/DPI-transition acceptance.

## Native Windows keyboard/dialog checks

| Case | Result |
| --- | --- |
| Ctrl+O and native picker | PASS: generated 3-page PDF opened; active title confirmed |
| Ctrl+W | PASS: fixture tab closed and original 50-page document became active |
| Ctrl+S and native Save As | PASS: fresh PDF created; bytes identical to source |
| Ctrl+A / Ctrl+C | PASS: independent Windows clipboard read matched full-page extraction after whitespace normalization |
| Page Down | PASS: moved from page 1 to page 2, independently verified from copied generated text |

Source and Save As output SHA-256:

```text
c6327e578b0acda7e5279551c4889b0059c01581ff7534710af4cfc1dffa5a86
```

Unrelated clipboard contents were not printed or saved.

## Remaining gates

- **Windows printing remains incomplete.** The new GTK native-dialog/file-output work is Linux-specific. Fedora's native-print script was not run on Windows, and its results are not inferred as Windows acceptance. This retest verified diagnostic panel discovery and unsaved-mark guards, not actual current/range/all-page output, physical scale or printer selection. No new print job was submitted.
- Live Windows capture still failed with FrameArrived timed out on initial and refreshed attempts. The custom main window exposes only its frame through accessibility. Native dialog fields and keyboard shortcuts worked, but OS pointer placement/resize, drag/drop, native image paste, full studio interaction and visual focus remain unqualified by this tooling.
- Undo/Redo and export-race assertions above use Slint diagnostic event dispatch. Native pointer gestures, keyboard/layout variations and real Windows display-scale/monitor changes remain separate acceptance.
- Broader password/signed/malformed-file UI compatibility, unwritable-save UX, representative long-document performance and draft recovery across restart remain qualification work.
- Editable image overlays do not establish secure baking/redaction, cryptographic seals or revisioned remarks. No stable milestone is claimed.

Only generated PDFs and the repository's synthetic page-photo.jpg were used. Normal preferences/signature libraries, private documents and credentials were excluded.

Report branch: validation/windows-v0.1.8. Only this report is added. Application fixes were not made, and the tested application commit/executable hash are separate from the report commit.
