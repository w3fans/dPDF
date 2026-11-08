# Windows validation — dPDF 1c57871

Date: 2026-10-28 (Europe/Ljubljana). Validator: Codex on [private].

**Result: Windows compilation and core reader checks pass; Windows acceptance is incomplete. Page exports fail. No application fixes or main-branch changes were made.**

## Exact source and environment

- Tested application commit: `1c578715bc574ce9c6d162769436672edb2b582f`; development version `0.1.0`.
- Native checkout: `D:\zDev\dPDF`. No cross-compilation or Linux worker was used.
- Windows 11 Pro x64, version `10.0.22631`; Ryzen 7 5700G, 6 logical CPUs presented, approximately 8 GiB RAM.
- PowerShell `7.6.5`; Python `3.12.14`.
- Rust `1.99.1 (58cf8f9c3 2026-10-13)`; Cargo `1.99.1 (5f94df478 2026-08-27)`; host `x86_64-pc-windows-msvc`.
- Visual Studio Build Tools 2022 `17.14.43`; MSVC toolset directory `14.44.35207`, compiler `19.44.35229.0`; Windows SDK `10.0.26100.0`.
- PDFium Windows x64 `chromium/7881`, matching the pinned pdfium-render API.
- qpdf `12.4.2` Windows MSVC64; existing SumatraPDF `3.5.2` for the supplemental print check.
- Cargo retained the repository's single-job configuration.

Initially only the existing dGK Windows GNU Rust toolchain was available. The initial build correctly refused that toolchain; a subsequent MSVC attempt failed because link.exe was absent. An isolated MSVC Rust toolchain and Microsoft's signed C++ Build Tools/SDK were then installed. The first C++ installer attempt failed on a temporary channel-manifest path; retrying with a local official channel manifest completed with exit 0. Final build/check results below are after setup was resolved. Rust paths were selected per process; the dGK GNU toolchain was retained.

Release executable SHA-256:

```text
f08429d8dd08a910b70fa6ae3386ec7093f0ead50e7e47c82eb2dac81a910d91
```

Downloaded archive hashes matched the publishers' GitHub release-asset SHA-256 metadata:

| Archive | SHA-256 |
| --- | --- |
| pdfium-win-x64.tgz, chromium/7881 | 73cc0de638ac2095e7445bf56a38200a5b7c7ca0e9f4ba144598f2457377ac08 |
| qpdf-12.4.2-msvc64.zip | db87077e683630c1217e0e8f9a20a9749d952ab676e881c3689187763a5de25d |

## Rust checks

| Command | Result |
| --- | --- |
| `.\build.ps1 -Release` | PASS; native optimized build, 14m 35s on the first dependency build |
| `cargo fmt --check` | PASS |
| `cargo check --locked` | PASS |
| `cargo test --locked` | PASS: 22 passed, 0 failed, 0 ignored |
| `cargo clippy --locked -- -D warnings` | PASS |

These commands ran against the pinned, unchanged source. Passing unit tests do not qualify page exports or printing.

## Executable and Windows checks

Only generated fixtures were used: scripts/make-sample.py produced 1-, 3- and 50-page PDFs; qpdf generated a password-protected copy; a malformed input was generated locally. The repository's page-photo.jpg is documented as an original synthetic test image. Private documents, credentials, normal dPDF preferences, and saved mark libraries were excluded.

| Area | Result and evidence |
| --- | --- |
| Opening/rendering | PASS: 50-page CLI probe; 1000x1294 first frame, 154 text characters, checksum b82745673e81dddf. One warm probe reported open_ms=2 / first_frame_ms=10; this is a synthetic-fixture measurement, not an office-document benchmark. |
| Reader corpus | PASS: check-reader.py against the release executable. Case-insensitive/empty/absent search, 100 occurrences, all intrinsic rotations with/without crop offsets, text selection, invalid selection rectangles, explicit 10,000-match cap, and malformed/missing inputs. |
| Native Open dialog | PASS for the tested path: OS Ctrl+O opened the Windows picker; its File name field was set to the generated 3-page PDF and Alt+O confirmed it. The reader title changed to contract-3.pdf. Escape cancellation also worked. File-drop opening remains unverified. |
| Native tabs/navigation | PASS: Ctrl+Tab switched from the 3-page document to the 50-page document, verified by window title. Page Down moved page 1 to page 2, verified independently from copied fixture text. Slow multi-digit page entry, every bookmark, and close/restore behavior through OS input remain unverified. |
| Native text copying | PASS: OS Ctrl+A / Ctrl+C copied page 1 text. An independent Windows clipboard read matched the executable's full-page extraction after whitespace normalization; copying after Page Down identified page 2. Unrelated clipboard contents were not printed or saved. Copy/paste into another editor and pointer-selection behavior through OS input remain unverified. |
| Native Save As | PASS: Ctrl+S, native filename entry and Alt+S created a fresh PDF. Source and output SHA-256 both c6327e578b0acda7e5279551c4889b0059c01581ff7534710af4cfc1dffa5a86. |
| Real-window reader smoke, 150% application scale | PASS: SLINT_SCALE_FACTOR=1.5 with --smoke-ui completed. Rendering/thumbnails/bookmarks, dispatched text selection, thumbnail reorder/cancel, navigation, 100 search occurrences, continuous/facing/rotated views, Ctrl+wheel versus ordinary wheel, second-tab close/scroll restoration, fit/dark mode, and print-panel discovery were exercised. This uses the app's diagnostic event dispatch, not OS-delivered mouse input. |
| Reader smoke, 100% application scale | FAIL: SLINT_SCALE_FACTOR=1 timed out at stage 24 during the synthetic thumbnail drag. A reader-only run without an explicit scale also stopped there. The first assembly-enabled smoke passed that stage and later failed at export. Do not report universal drag/scaling acceptance. |
| Visual inspection | The app-owned 150% smoke snapshot was inspected: toolbar, search results, sidebar and rotated facing pages were visible. Live Windows screen capture was unavailable; this does not establish full native HiDPI/input acceptance. |
| Page operations | FAIL: replace, insert, remove, extract, reorder, merge and rotate-right all failed with the same qpdf temporary-output permission error. Generated inputs and a pre-existing destination sentinel stayed unchanged. See reproduction below. |
| Mark backend | The unmodified check-marks.py FAILS before invoking dPDF because its filename contains double quotes. A diagnostic copy outside the repository, changing only that fixture filename, PASSED crop/deskew, original-byte preservation, cleaned PNG/thumbnail output, physical-size persistence, invalid-size/aspect refusal, source-overwrite refusal and future-schema preservation. |
| Error handling | CLI probes correctly returned failure for missing/corrupt PDFs, password-protected input, and an explicitly missing PDFium DLL. Messages exposed FormatError, PasswordError, NotFound, and Load DPDF_PDFIUM_LIBRARY. Password entry/recovery and native error-dialog clarity remain unverified. |
| Staging | PASS for stage.ps1 plus staged CLI launch/probe with DPDF_PDFIUM_LIBRARY unset: the side-by-side DLL loaded and the rendered checksum matched. PDFium license notices were staged. Staged GUI launch, installer/file associations and a complete qpdf/helper deployment remain unverified. |
| Printing | INCOMPLETE. The reader smoke opened the custom print panel and completed discovery with two destination choices. No dPDF print-panel job was submitted. A supplemental driver compiled the pinned src/platform.rs unchanged and requested page 7, actual-size, on Microsoft Print to PDF through SumatraPDF. It stalled without an observable output dialog, completed PDF, or queued job during inspection; only the test-owned helper was stopped. This was a backend diagnostic, not full application printing acceptance. All/range/current-page output counts and physical scale remain unverified. |

The Computer Use app-access prompt was approved after an initial timeout. Live capture then failed with `FrameArrived timed out: timed out waiting on channel` on both the initial attempt and recovery attempt. The main-window accessibility tree exposed only its frame; control clicks reported `coordinate input geometry is unavailable`. Native picker fields could still be set through accessibility and confirmed/cancelled with keyboard shortcuts. Custom-control visual tests and print-panel mouse interaction were therefore limited by the validation tooling; this alone is not evidence of a dPDF rendering defect.

## Finding 1 — Windows page export cannot open its temporary output

Reproduce with matching PDFium and qpdf installed:

```powershell
$env:DPDF_PDFIUM_LIBRARY = 'C:\deps\pdfium\bin\pdfium.dll'
$env:DPDF_QPDF_BINARY = 'C:\deps\qpdf\bin\qpdf.exe'
python scripts/make-sample.py C:\temp\contract-50.pdf --pages 50
python scripts/make-sample.py C:\temp\page.pdf --pages 1
.\target\release\dpdf.exe --assemble replace C:\temp\contract-50.pdf C:\temp\edited.pdf 50 1 C:\temp\page.pdf
```

Expected: a 50-page edited copy, preserving pages 1–49 and the source.

Actual: exit 1; qpdf exits 2 with:

```text
Assemble pages: qpdf ... open \\?\C:\...\.tmpXXXXXX: Permission denied
```

All seven operations listed above reproduced this error with valid, Windows-compatible output names. src/assembly.rs creates a NamedTempFile beside the destination and keeps its handle open while qpdf writes that path (around lines 288–305). A separate Windows control reproduced qpdf failure while a read/write file handle was open, then success after closing it. qpdf also successfully wrote fresh normal and extended-length paths. This isolates the open-output-handle sharing problem rather than a missing qpdf installation or general output-directory denial.

Coordinate a Windows-safe temporary-output handoff while retaining private staging, source preservation, validation and atomic final publication. No fix was applied by this validator.

The assembly-enabled --smoke-ui run reached stage 17 and timed out after this export error; earlier reader stages passed.

## Finding 2 — two corpus filenames are invalid on Windows

- scripts/check-marks.py:31 uses `original "photo".jpg`; Python fails at write_bytes with OSError [Errno 22].
- scripts/check-pages.py:76 uses `contract "final" copy.pdf`. Its unmodified first export fails. A diagnostic copy changing only this filename still fails at the application temporary-output error above.

Use legal Windows filenames while retaining spaces/punctuation coverage. The diagnostic adaptations were outside the repository and were not application fixes.

## Finding 3 — thumbnail-drag smoke is scale-sensitive

With isolated preferences and the generated 50-page PDF:

```powershell
$env:SLINT_SCALE_FACTOR = '1'
.\target\release\dpdf.exe --smoke-ui C:\temp\contract-50.pdf
```

The reader-only smoke timed out at stage 24. Repeating with `SLINT_SCALE_FACTOR=1.5` completed. The diagnostic uses fixed logical drag coordinates in src/app.rs around stages 23–24. Investigate diagnostic coordinates versus layout/input handling; OS mouse behavior was not established because capture/coordinate automation was unavailable. Changing application scale is not a substitute for testing Windows display-scale changes and monitor transitions.

## Remaining functionality and acceptance

- Windows printer discovery explicitly reports unimplemented in src/platform.rs; the Sumatra helper remains interim rather than native spooler/dialog support.
- Password-protected input fails without a password-entry flow in the tested CLI path.
- Graphical signature/stamp cleanup/library browsing, on-page placement, overlay export, secure baking/redaction, review revisions and digital seals remain unfinished as documented in the pinned milestone files.
- Complete actual printer/PDF-printer output verification, current/range/all-page counts, fit versus actual-size measurements, native print-panel interaction, real Windows display-scale/monitor changes, OS pointer drag/drop/selection, search focus and slow page-entry checks, signed-PDF warnings, unwritable-save handling, broader compatibility and representative long-document performance.
- Rebuild and rerun on the exact fix commit from [private]. Keep code fixes separate from this report branch; no stable-release qualification is claimed.

## Repeatable handoff

[private] supplies an exact commit ID. [private] checks out that commit, runs the native MSVC build and Rust checks, then generated-fixture executable/Windows tests with isolated data/config directories. Report the application commit and executable hash separately from the documentation commit. Publish findings on a validation branch tied to the application commit. A build or diagnostic pass alone never substitutes for the still-open acceptance items.
