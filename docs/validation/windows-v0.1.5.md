# Windows validation — dPDF v0.1.5

Validation session: 2026-10-28, Europe/Ljubljana. Validator: Codex on [private].

**Result: both v0.1.3 blockers are fixed with the official qpdf executable. All prescribed automated suites and scale diagnostics pass. A new P1 source-preservation failure remains: drive/UNC aliases allow page and overlay exports to overwrite their input. Printing remains unqualified.**

## Exact application and setup

- Tag: v0.1.5. Application commit: **3601ba3db5c715c941595f42d76e75d7318a3609**.
- Upstream main still pointed to this commit at the final check. v0.1.4 placement changes were tested cumulatively through v0.1.5; its older binary was not separately qualified.
- Native MSVC build in D:\zDev\dPDF on [private]. No Linux worker, cross-build, source patch, or qpdf normalization wrapper was used.
- Existing Windows 11 Pro x64 10.0.22631 setup; Ryzen 7 5700G, 6 logical CPUs presented, approximately 8 GiB RAM.
- Rust/Cargo 1.99.1, x86_64-pc-windows-msvc; Visual Studio Build Tools 2022 17.14.43, compiler 19.44.35229.0, SDK 10.0.26100.0; PowerShell 7.6.5; Python 3.12.14.
- Matching PDFium Windows x64 chromium/7881 and official qpdf 12.4.2 MSVC64. The existing dGK GNU toolchain was retained.
- Repository single-job configuration retained. Preferences, mark libraries and outputs were isolated under the validation runtime directory.

Release executable SHA-256:

```text
948772bd42f791c597c21de8544dba43cd71d11e840940ebc7cfb5e9a401bfcc
```

## Passed checks

| Check | Result |
| --- | --- |
| build.ps1 -Release | PASS; native optimized build, 2m 06s |
| cargo fmt --check | PASS |
| cargo check --locked | PASS |
| cargo test --locked | PASS: 27 passed, 0 failed, 0 ignored |
| cargo clippy --locked -- -D warnings | PASS |
| Unmodified check-reader.py | PASS: search cases/counts, rotated/cropped geometry and selection, invalid rectangles, 10,000-match cap, missing/corrupt PDFs |
| Unmodified check-marks.py | PASS: cleanup, exact original bytes, cleaned PNG/thumbnail, mm persistence, invalid dimensions/aspect, source refusal and future metadata |
| Unmodified check-pages.py, official qpdf directly | PASS: page replacement, untouched streams/pixels, extraction, remove/reorder, insert/merge, page rotation, PNG/JPEG/alpha/phone orientation, centering/aspect, original JPEG stream, mixed input, invalid-operation safety and ordinary same-path source refusal |
| Unmodified check-overlays.py, official qpdf directly | PASS: untouched streams/pixels, affected-page text, crop/page/mark quarter turns, mm geometry, alpha, multiple marks/pages, downsampling, JPEG orientation and invalid/output-input safety cases in the corpus |
| Separate local Unicode/space filename exports | PASS: generated files in “Žig čšž test”, including Pogodba ŽČŠ.pdf and Stran žiga.pdf |
| Separate UNC Unicode page/overlay exports | PASS through the existing loopback administrative share to the same isolated directory; the original 50-page input hash remained unchanged. No share or security settings were changed |
| Staged launch/probe | PASS with DPDF_PDFIUM_LIBRARY unset: adjacent PDFium loaded and rendered 50 pages, checksum b82745673e81dddf; notices staged |
| Native Windows Open/tab close | PASS: Ctrl+O, accessible filename entry and Alt+O opened the generated 3-page fixture; Ctrl+W returned to the original 50-page document, verified by window titles |
| Native Windows Save As | PASS: Ctrl+S, native filename entry and Alt+S produced a byte-identical fresh copy |
| Native Windows clipboard copying | PASS: Ctrl+A/Ctrl+C from page 1; independent Windows clipboard read matched full-page extraction after whitespace normalization. Unrelated clipboard contents were not printed or saved |
| Native Windows page navigation | PASS: Page Down reached page 2, independently verified from copied generated text |

Source and native Save As output SHA-256:

```text
c6327e578b0acda7e5279551c4889b0059c01581ff7534710af4cfc1dffa5a86
```

### Scale diagnostics

| Application scale | Reader + page assembly + graphical placement | Studio fast size/save/placement |
| --- | --- | --- |
| 1 / 100% | PASS | PASS |
| 1.5 / 150% | PASS | PASS |
| 2 / 200% | PASS | PASS |

The reader smoke used both DPDF_SMOKE_ASSEMBLY_SOURCE and DPDF_SMOKE_PLACEMENT_IMAGE with generated fixtures and official qpdf. It passed rendering, thumbnails/bookmarks, dispatched text selection, thumbnail reorder/cancel, navigation/search, continuous/facing/rotated views, wheel/zoom separation, tab restoration and print-panel discovery. Placement assertions passed page-click placement, drag movement, corner resize, numeric 35x25 mm / 70% opacity, rotated preview, tab isolation, print/page guards for unsaved marks, worker export/reopen and clearing source drafts only after successful export.

The studio smoke passed crop/deskew and immutable imported-source preservation, same-event-turn width change plus Save Default Size, locked-ratio persistence through reopen, another fast width edit before placement, correct immutable cleaned placement payload, and releasing working images on close. The old phase-5 aspect-ratio failure did not reproduce.

App-owned studio and placement snapshots were visually inspected. The studio showed the saved 72x89.714 mm proportional size; the placement snapshot showed the selected mark, numeric controls and the unsaved-mark print/page guard. These are diagnostic render snapshots, not live Windows screen capture.

## P1 — drive/UNC aliases bypass source-overwrite protection

**Actual source overwrite occurred only in disposable generated fixtures. No private document was involved. This is a separate failure from the fixed qpdf path parsing.**

Setup: the same test folder was accessible as a local C: path and through the existing \\localhost\C$ share. Both paths referred to the same underlying file.

### Page-operation reproduction

With the official qpdf and PDFium configured, use a writable generated-only directory accessible through both names:

```powershell
$env:DPDF_PDFIUM_LIBRARY = 'C:\deps\pdfium\bin\pdfium.dll'
$env:DPDF_QPDF_BINARY = 'C:\deps\qpdf\bin\qpdf.exe'
New-Item -ItemType Directory -Force C:\Temp\dpdf-v015-alias | Out-Null
python scripts/make-sample.py C:\Temp\dpdf-v015-alias\source.pdf --pages 3
Get-FileHash C:\Temp\dpdf-v015-alias\source.pdf
.\target\release\dpdf.exe --assemble remove 'C:\Temp\dpdf-v015-alias\source.pdf' '\\localhost\C$\Temp\dpdf-v015-alias\source.pdf' 1 1
Get-FileHash C:\Temp\dpdf-v015-alias\source.pdf
qpdf --show-npages C:\Temp\dpdf-v015-alias\source.pdf
```

Expected: nonzero exit, source bytes unchanged, source remains 3 pages.

Observed: exit **0**, saved_pages=2, the local source hash changed, and independently reading the local source reported **2 pages**. The console still stated that source files were preserved.

Control: giving exactly the same local path as input/output returned exit 1 (“Choose a different output file to preserve the source PDF”), and its source hash remained unchanged.

### Overlay reproduction

A separate generated 3-page source and generated PNG/placement recipe were used. The input was its local-drive path; output was the corresponding UNC alias of that same PDF:

```powershell
.\target\release\dpdf.exe --overlay 'C:\Temp\dpdf-v015-alias\overlay-source.pdf' '\\localhost\C$\Temp\dpdf-v015-alias\overlay-source.pdf' 'C:\Temp\dpdf-v015-alias\placements.json'
```

The recipe used one valid generated image on page 1 at x=25, y=40, width=40, height=20 mm, opacity=0.5. Expected: refuse replacing the source. Observed: exit **0**, saved_pages=3 and the original local PDF hash changed. A separate-output Unicode UNC overlay export passed and preserved its original.

### Relevant source and follow-up

- src/assembly.rs:307–309 compares canonical destination and source path values.
- src/overlay.rs:119 and :139 similarly compare canonical path values to protect the PDF, recipe and images.

The tested local-drive and UNC aliases remain different canonical path spellings even though they identify the same existing file. Path-value equality therefore does not establish distinct file identity. Coordinate an existing-file identity check or a conservative refusal for unresolved aliases before publication, and add regression cases for page and overlay exports. Review the related protected-input checks as well. No application fix was made by this validator.

The passing ordinary corpus safety cases do not cover this alias case and must not be interpreted as universal input-preservation proof.

## Remaining qualification limits

- Printing remains incomplete. The Windows printing backend is unchanged from the earlier tested version; native discovery remains unimplemented and the Sumatra helper is interim. Smoke verified print-panel discovery and preventing printing of unsaved marks, not actual printer output. No new print job was submitted. Current/range/all-page counts and physical fit/actual-size measurements remain open.
- Live Computer Use capture again failed with FrameArrived timed out on initial and refreshed attempts. The custom main window exposes only its frame through accessibility. Native dialogs/keyboard checks were possible, but OS pointer placement, resize, drag/drop, image paste, visual focus, native monitor/DPI changes and complete studio interaction remain unverified. This tool limitation is not attributed to dPDF rendering.
- Placement cancellation, failed-export UI recovery, discard confirmation and edits while a placement export is running were not independently driven through native Windows input. The successful-export and guard assertions above are diagnostic dispatch.
- Continue broader password/signed/malformed-document UI compatibility, unwritable saves and representative long-document performance qualification.
- Ordinary overlays remain editable image objects; secure baking/redaction, cryptographic seals and revisioned remarks are not qualified.

Only generated PDFs, the repository's documented synthetic page-photo.jpg and a PNG derived from it were used. Normal preferences/signature assets, private documents and credentials were excluded.

Report branch: validation/windows-v0.1.5. Only this document is added; the application commit and executable hash remain separate from the report commit. Both earlier reported blockers are resolved, but the alias overwrite and unresolved platform gates prevent complete Windows/stable-release qualification.
