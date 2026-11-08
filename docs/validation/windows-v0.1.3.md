# Windows retest — dPDF v0.1.3

Date: 2026-10-28 (Europe/Ljubljana). Validator: Codex on [private].

**Result: native build and reader acceptance checks pass. Windows page/overlay exports and the studio size-update diagnostic still have blockers. Printing remains incomplete. No application fixes were made.**

## Exact application and environment

- Tag: **v0.1.3**, application commit **b82046579627a94a26b3b59135254cde91548447**. The annotated tag resolves to this commit; upstream main still pointed here at the final check.
- v0.1.1/v0.1.2 changes were tested cumulatively through v0.1.3; their older binaries were not separately qualified.
- Native Windows checkout: D:\zDev\dPDF. MSVC compilation ran on [private], without a Linux worker or cross-compilation.
- Existing setup: Windows 11 Pro x64 10.0.22631; Ryzen 7 5700G with 6 logical CPUs presented and approximately 8 GiB RAM; PowerShell 7.6.5; Python 3.12.14.
- Rust 1.99.1 / Cargo 1.99.1, x86_64-pc-windows-msvc; Visual Studio Build Tools 2022 17.14.43, compiler 19.44.35229.0, SDK 10.0.26100.0.
- PDFium Windows x64 chromium/7881; official qpdf 12.4.2 MSVC64. The existing dGK GNU toolchain was retained.
- Repository single-job settings were retained. All generated test data, configuration, and mark libraries were isolated outside source control.

Release executable SHA-256:

```text
2da84bec977e0d4f6b435ee3b64e65cb52185a3129eb2692f6615fbed55505e4
```

## Results

| Check | Result |
| --- | --- |
| build.ps1 -Release | PASS; optimized native build, 4m 29s |
| cargo fmt --check | PASS |
| cargo check --locked | PASS |
| cargo test --locked | PASS: 25 passed, 0 failed, 0 ignored |
| cargo clippy --locked -- -D warnings | PASS |
| Unmodified check-reader.py | PASS: search counts/case/empty/absent queries, rotated/cropped geometry and selection, malformed selections, 10,000-match cap, missing/corrupt inputs |
| Unmodified check-marks.py | PASS: its Windows filename is fixed; cleanup, original bytes, PNG/thumbnail output, mm persistence, invalid sizes/aspect, source-overwrite refusal and future-schema preservation |
| Unmodified check-pages.py with official qpdf | FAIL at first replacement: assembled-file validation cannot find the verbatim temporary path |
| Unmodified check-overlays.py with official qpdf | FAIL at first export, same validation error |
| All eight direct page-operation checks | FAIL at validation: replace, insert, remove, extract, reorder, merge, rotate-right and rotate-left. Generated input and an existing destination sentinel remained unchanged |
| Reader-only --smoke-ui, scales 1 / 1.5 / 2 | PASS at all three scales; the prior scale-1 thumbnail-drag failure no longer reproduces |
| --smoke-ui with assembly enabled and official qpdf, scales 1 / 1.5 / 2 | FAIL at stage 17 on the export validation error; earlier reader/drag/navigation/search/print-panel stages passed |
| --smoke-marks, scales 1 / 1.5 / 2 | FAIL at phase 5 after first successful asset save, with the locked-aspect preferred-size error |
| Staged CLI launch | PASS: stage.ps1 copied the executable, PDFium and notices; with DPDF_PDFIUM_LIBRARY unset, the staged executable loaded the adjacent DLL and rendered the 50-page fixture |
| Native Windows Open / tabs | PASS: Ctrl+O and native picker confirmation opened the generated 3-page PDF; Ctrl+Tab switched to the original 50-page document, confirmed by window title |
| Native Windows Save As | PASS: Ctrl+S, native filename entry, Alt+S. The new PDF is byte-identical to its source |
| Native Windows text copying | PASS: Ctrl+A/Ctrl+C; independent Windows clipboard read matched full-page extraction after whitespace normalization. No unrelated clipboard contents were printed or saved |
| Native Windows page navigation | PASS: Page Down moved from page 1 to page 2, independently verified from copied generated fixture text |
| Native printing | INCOMPLETE. Diagnostic print-panel opening/discovery completed, with two destination choices. No new actual print job was submitted; current/range/all output counts and physical scaling remain unverified |

Source and native Save As output SHA-256 both equal:

```text
c6327e578b0acda7e5279551c4889b0059c01581ff7534710af4cfc1dffa5a86
```

Reader-only smoke exercised rendering/thumbnails/bookmarks, dispatched rectangular selection and Ctrl+A, thumbnail reorder/cancellation, navigation, 100 search occurrences, continuous/facing/rotated views, Ctrl+wheel versus ordinary wheel, second-tab close/scroll restoration, fit/dark mode, and print-panel discovery. This is application diagnostic dispatch, not native mouse acceptance. Application scale factors do not establish Windows monitor/DPI-transition behavior.

The app-owned 200% reader snapshot was visually inspected: toolbar, search results, sidebar and rotated facing pages were visible. Live Windows capture failed again with FrameArrived timed out on initial and refreshed attempts; the main accessibility tree exposes only its frame. Native picker fields and keyboard shortcuts remained usable. Capture-tool failure is not evidence of a dPDF rendering defect. Native pointer/drop/image-paste/custom-control acceptance remains open.

## Blocker 1 — qpdf validation of a Windows verbatim path

The previous open-output-handle problem has progressed: the qpdf assembly invocation now succeeds, but validation of its output fails.

Reproduction with the unchanged executable and official helper:

```powershell
$env:DPDF_PDFIUM_LIBRARY = 'C:\deps\pdfium\bin\pdfium.dll'
$env:DPDF_QPDF_BINARY = 'C:\deps\qpdf\bin\qpdf.exe'
python scripts/make-sample.py C:\temp\contract-50.pdf --pages 50
python scripts/make-sample.py C:\temp\page.pdf --pages 1
.\target\release\dpdf.exe --assemble replace C:\temp\contract-50.pdf C:\temp\edited.pdf 50 1 C:\temp\page.pdf
```

Actual: dPDF exits 1, qpdf validation exits 2:

```text
Error: Validate assembled PDF: qpdf exited exit code: 2:
qpdf: open \\.tmpXXXXXX: No such file or directory

```

Expected: a validated, atomically published 50-page edited copy preserving the input.

A control using the same generated PDF isolates the helper's command-line path handling:

```powershell
$normal = 'C:\temp\contract-50.pdf'
& $env:DPDF_QPDF_BINARY --check $normal
# exit 0
& $env:DPDF_QPDF_BINARY --check ('\\?\' + $normal)
# exit 2; the error names \\contract-50.pdf
```

src/assembly.rs canonicalizes the destination parent. Its closed TempPath therefore has a Windows verbatim prefix; that path is placed in the qpdf job JSON and later passed directly to --check/page counting. Job execution succeeds, whereas the CLI check fails. Investigate consistent Windows path handling at these helper boundaries; retain source preservation and validated atomic publication.

### Supplemental isolation — external normalization wrapper

A small Rust wrapper **outside the repository** delegated to the same official qpdf executable, changing only command-line arguments beginning with the local-drive verbatim prefix by removing that prefix. DPDF_QPDF_BINARY selected this wrapper. Application source and release executable were unchanged. This is a diagnostic workaround, not a shipped fix; only generated local-drive paths were tested.

With that wrapper:

- Full, otherwise unmodified **page corpus PASSED**: contract replacement with exact untouched streams/pixels, extraction/catalog cleanup, removal/reordering, insertion/multi-PDF merge, page rotation, PNG/JPEG/alpha/phone orientation, aspect/centering, unchanged JPEG bytes, mixed sources, invalid-operation safety, prior-output preservation, input preservation and overwrite refusal.
- Full, otherwise unmodified **overlay corpus PASSED**: exact untouched-page streams/pixels and affected-page text; crop/page/mark quarter turns, physical geometry, opacity and unchanged outside pixels; multiple marks/pages, bounded downsampling/JPEG orientation, invalid placement/corrupt image safety and protected PDF/image/recipe inputs.
- **Reader-plus-assembly real-window smoke PASSED at scales 1, 1.5 and 2**, including export from the retained source snapshot after its diagnostic-owned disk copy was changed.

These supplemental passes isolate the remaining normal-configuration integration failure. They do not qualify unmodified Windows exports.

## Blocker 2 — studio preferred-size update

With a fresh empty diagnostic data directory:

```powershell
$env:DPDF_DATA_DIR = 'C:\temp\dpdf-v013-empty-studio'
$env:SLINT_SCALE_FACTOR = '1'
.\target\release\dpdf.exe --smoke-marks scripts/fixtures/page-photo.jpg
```

Repeat with fresh data directories at 1.5 and 2. All three runs reached:

```text
studio smoke: crop drag, deskew and captured-source save passed
Error: Studio smoke timed out at phase 5:
Preferred size must preserve the image aspect ratio while locked
```

The first generated asset was saved successfully, preserving its captured original and crop/deskew settings. The scale-1 asset had cleaned pixels 126x157, preferred size 60x74.762 mm, aspect_locked=true. The diagnostic then changed width to 72 mm and invoked Save Default Size, but never observed a successful size update.

Inference from src/mark_studio.rs: the diagnostic changes width and invokes saving on a later diagnostic tick, while a separate 40-ms State::tick synchronizes the locked height. State::save reads both dimensions without synchronizing them first. Timer ordering / stale height appears involved. The backend corpus accepts valid proportional dimensions, so this result concerns studio integration/diagnostic timing; a normal-speed native user reproduction has not been established. Verify synchronization before saving and make the diagnostic wait for the intended ratio. No application change was made.

## Remaining gates and handoff

- Windows printing source is unchanged from the original tested commit. Its native printer discovery remains unimplemented and the Sumatra helper is interim. The prior bounded Print-to-PDF attempt stalled; it was not repeated without a source/helper change. Actual output/count/scale qualification remains open.
- Studio native Import/Paste Image, library browsing, manual crop/deskew/levels and fast mm edits still require native pointer/visual acceptance. Normal image clipboard contents and saved signature assets were not used.
- Complete Windows display-scale/monitor changes, OS mouse drag/drop/selection, slow page entry/search focus, broader malformed/password/signed-file UI handling, unwritable saves and representative long-document performance.
- Overlay placement remains a diagnostic recipe exporter; graphical on-page placement, secure baking/redaction, cryptographic seals and revisioned remarks are not qualified by these tests.
- All fixtures were generated or the repository's documented synthetic page-photo.jpg. No private PDFs, credentials, normal preferences/libraries, or unrelated clipboard contents were published.
- Report branch: validation/windows-v0.1.3. Only this report is added. Keep application fixes on the implementation side and send the next exact application commit for retesting. No patch release, main update, or stable milestone qualification was performed by this validator.
