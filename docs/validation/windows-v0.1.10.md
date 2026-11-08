# Windows validation — dPDF v0.1.10

Date: 2026-10-29 (Europe/Ljubljana). Validator: Codex on [private].

**Result: native build, 37 unit tests, five standard corpora, two Windows-adapted password/recovery corpora, and all six scale diagnostics pass. Native unprotected-document checks pass. No new blocker was found in these tested paths. Native Windows password entry, encrypted-edit confirmation, printing and full pointer acceptance remain open.**

## Exact application and resource control

- Tag: v0.1.10. Application commit: **dd967660cc7a72c107b03c6cf1dd536f552640a2**.
- Main still pointed to this commit at the final upstream check. v0.1.9 recovery changes were tested cumulatively through v0.1.10; its older binary was not separately qualified.
- Native MSVC build at D:\zDev\dPDF. No Linux worker, cross-build, application patch or qpdf wrapper was used.
- Existing Windows 11 Pro x64 10.0.22631; Ryzen 7 5700G with 6 logical CPUs presented, approximately 8 GiB RAM; Rust/Cargo 1.99.1, x86_64-pc-windows-msvc; compiler 19.44.35229.0 / Build Tools 2022 17.14.43 / SDK 10.0.26100.0; PowerShell 7.6.5, Python 3.12.14.
- Matching PDFium chromium/7881 Windows x64 and official qpdf 12.4.2 MSVC64. dGK's existing GNU toolchain was retained.
- Before building, the one older fixture-only dPDF instance was identified by PID/path/command line and closed.
- Cargo retained one build job. Corpus suites ran sequentially; scale diagnostics ran sequentially; the final native checks used one test instance.
- The final test window was closed with Alt+F4. A process inventory afterward confirmed **zero dpdf.exe processes running**. Older binaries/reports were kept on disk as evidence; no application instance was left consuming resources.

Release executable SHA-256:

```text
970ce87c3a341096fc4b5608369dd2429833e73e55401b0f809440e8d8de4e39
```

## Build and standard corpora

| Check | Result |
| --- | --- |
| build.ps1 -Release | PASS; native optimized build, 2m 24s |
| cargo fmt --check | PASS |
| cargo check --locked | PASS |
| cargo test --locked | PASS: 37 passed, 0 failed, 0 ignored |
| cargo clippy --locked -- -D warnings | PASS |
| Unmodified check-aliases.py | PASS: hard links and existing loopback UNC aliases refused; protected PDF/page/image/recipe/cleanup/raster bytes unchanged |
| Unmodified check-reader.py | PASS: search cases/counts, crop/rotation geometry and selection, malformed rectangles, 10,000-match cap and missing/corrupt input |
| Unmodified check-pages.py | PASS: replacement/untouched streams and pixels, extraction/catalog cleanup, remove/reorder, insert/merge/rotate, images/alpha/orientation, geometry, mixed sources and failure/input protection |
| Unmodified check-marks.py | PASS: cleanup, original bytes, PNG/thumbnail, physical sizes, invalid dimensions/aspect, source refusal and future metadata |
| Unmodified check-overlays.py | PASS: untouched streams/pixels, affected-page text, rotations/crops, physical geometry/opacity, multiple marks/pages, downsampling/orientation and failure/input protection |
| Staged CLI launch | PASS with DPDF_PDFIUM_LIBRARY unset: adjacent DLL loaded, 50-page render checksum b82745673e81dddf; license notices staged |

The prior drive/UNC overwrite blocker did not recur in the alias regression suite, including the actual accessible loopback share.

## New recovery/password checks on Windows

The upstream scripts require Linux's private Xvfb display context. They cannot run unchanged as complete scripts on Windows. A diagnostic adapter outside the repository replaced only their private-display module loader with a native Windows environment context. The original fixture generation, subprocess commands, timeouts and assertions were loaded unchanged from the pinned checkout.

The adapter gave the tests isolated config/data directories, removed Linux display and clipboard-diagnostic variables, and used native test windows. It did not provide a private Windows desktop. All tests used generated documents and synthetic passwords; no real credentials or normal libraries were used. The exact adapter is included below for review/reproduction.

| Windows-adapted suite | Result |
| --- | --- |
| check-recovery.py | PASS: interrupted-process restart with changed source, removed source, and explicit discard |
| check-passwords.py | PASS: AES-256, AES-256 with Unicode/leading/trailing spaces, and AES-128 scenarios |

Recovery assertions verified:

- A durable checkpoint existed before the writer's deliberate exit without normal cleanup.
- Restart recovered the opened 50-page snapshot and its mark geometry/opacity/aspect/default sizing even after the source became a 3-page PDF or was removed.
- Recovered output passed qpdf validation and had 50 pages.
- Changed sources stayed unchanged; removed sources stayed absent.
- Explicit discard retired the recognized draft; an unknown future recovery file remained byte-identical.

Password assertions verified:

- Incorrect-password retry, reader/search/placement, and failed-save/retry/export behavior.
- Ordinary Save As retained encrypted source bytes exactly.
- Edited output was valid, unencrypted, and 50 pages; the encrypted original stayed unchanged.
- Escape cancellation and queued prompts preserved sources and an in-progress entry.
- Protected-draft restart required password re-entry, exported successfully, retired its draft, and the checkpoint contained no literal synthetic test password.
- Test output did not contain the synthetic password values.

Fixture encryption used temporary JSON jobs removed by the test. Application code was not changed or instrumented. The Unix-only helper argument/pipe mock test was not run on Windows. These controlled restart and diagnostic-dispatch results are not power-loss qualification or a substitute for native user-entered password acceptance.

Reproduction after saving the adapter below:

```powershell
python windows-display-adapter.py recovery --binary target/release/dpdf.exe --pdfium C:\deps\pdfium\bin\pdfium.dll --qpdf C:\deps\qpdf\bin\qpdf.exe
python windows-display-adapter.py passwords --binary target/release/dpdf.exe --pdfium C:\deps\pdfium\bin\pdfium.dll --qpdf C:\deps\qpdf\bin\qpdf.exe
```

## Reader/placement/studio scale matrix

| Application scale | Reader + assembly + placement/history | Studio |
| --- | --- | --- |
| 1 / 100% | PASS | PASS |
| 1.5 / 150% | PASS | PASS |
| 2 / 200% | PASS | PASS |

The reader diagnostics retained the earlier rendering, text selection, thumbnail arrangement/cancel, navigation/search, continuous/facing/rotation, wheel/zoom, tabs, print-panel and immutable assembly-snapshot checks.

Placement passed Esc cancellation, click/move/corner resize, numeric size/opacity, Undo/Redo/Delete and text-field Undo separation, tab isolation, unsaved-mark print/page guards, failed-save retention/retry, preserving newer edits/history after an older export, and successful current-copy export/reopen.

Studio passed captured original preservation, crop/deskew, immediate locked-size save, reopen, fast preferred-size placement payload and image release on close. Each run used a fresh test library. Ordinary scale smoke intentionally has recovery disabled; the separate recovery scenarios above enabled it.

The app-owned 200% placement snapshot was visually inspected. These are renderer snapshots and diagnostic event dispatch, not Windows OS pointer or monitor/DPI-transition acceptance.

## Native Windows checks

| Check | Result |
| --- | --- |
| Ctrl+O/native picker | PASS: generated 3-page document opened, title verified |
| Ctrl+W | PASS: fixture tab closed and original 50-page document restored |
| Ctrl+S/native Save As | PASS: fresh output byte-identical to source |
| Ctrl+A/Ctrl+C | PASS: independent Windows clipboard read matched generated full-page extraction after whitespace normalization |
| Page Down | PASS: reached page 2, independently verified from copied generated text |
| Alt+F4 | PASS: final fixture window closed; process inventory then showed zero dPDF processes |

The source and ordinary native Save As output both had SHA-256:

```text
c6327e578b0acda7e5279551c4889b0059c01581ff7534710af4cfc1dffa5a86
```

Unrelated clipboard contents were not printed or saved.

## Remaining limits

- Live capture again returned FrameArrived timed out on initial and refreshed attempts. Native picker accessibility and keyboard input worked; the custom main window exposed only its frame. Native pointer selection/placement/resize, image paste, visual focus and full Windows display-scale/monitor transitions remain open. This tooling failure is not attributed to dPDF rendering.
- Native Windows masked password entry, queued-prompt usability and the edited-copy unencrypted-output confirmation were not manually driven. Their diagnostic behavior passed; that does not complete native user acceptance.
- Windows printing remains unverified. No new print job was submitted; Fedora GTK printing was not run or inferred on Windows. Current/range/all output, printer selection and physical scale remain qualification work.
- Controlled restart tests do not establish power-loss durability, full native Windows crash recovery or hostile concurrent filesystem-race guarantees.
- Broader signed/password/malformed-document UI compatibility, unwritable-save UX and representative long-document performance remain open. Editable overlays do not qualify secure baking/redaction, cryptographic seals or revisioned remarks.

Only generated fixtures and the repository's synthetic image were used. No application fixes, main updates, patch release or stable qualification were performed. Report branch: validation/windows-v0.1.10; only this report is added. The application hash/commit are separate from the report commit.

## Exact Windows display adapter

This file is a validation-only artifact outside the application checkout. ROOT identifies the pinned Windows checkout. Only the Linux display context is replaced; the test bodies are not edited.


```python
"""Run upstream password/recovery assertions with a Windows display adapter.

Only the Linux private-display context is replaced. Application code, fixtures,
subprocess checks and assertions are loaded unchanged from the pinned checkout.
The native diagnostic windows use isolated config/data and never write clipboard.
"""
from contextlib import contextmanager
import importlib.util
import os
from pathlib import Path
import sys
from types import SimpleNamespace

ROOT = Path('D:/zDev/dPDF')


@contextmanager
def windows_display(folder, _xvfb):
    env = dict(os.environ,
               DPDF_CONFIG_DIR=str(folder / 'windows-config'),
               DPDF_DATA_DIR=str(folder / 'windows-data'),
               SLINT_SCALE_FACTOR='1')
    for key in ('DISPLAY', 'WAYLAND_DISPLAY', 'XAUTHORITY', 'GDK_BACKEND',
                'DPDF_SMOKE_CLIPBOARD', 'DPDF_SMOKE_MARKS_CLIPBOARD',
                'DPDF_SMOKE_PREFERENCES', 'DPDF_SMOKE_SNAPSHOT',
                'DPDF_SMOKE_PLACEMENT_SNAPSHOT', 'DPDF_SMOKE_ASSEMBLY_SOURCE'):
        env.pop(key, None)
    yield env


suite = sys.argv.pop(1)
if suite not in ('passwords', 'recovery'):
    raise SystemExit('Choose passwords or recovery')
source = ROOT / 'scripts' / f'check-{suite}.py'
spec = importlib.util.spec_from_file_location('windows_acceptance', source)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
original_loader = module.module


def loader(name, filename):
    if filename == 'private-display.py':
        return SimpleNamespace(display=windows_display)
    return original_loader(name, filename)


module.module = loader
module.main()
```
