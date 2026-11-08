# Windows validation: dPDF v0.1.14

Tested application commit: `0d75b8c9fd92e9d231bd44428b1c8609edd2438b`.
Date: 2026-10-29. Native build and tests ran on [private] Windows, not [private] or [private].

## Environment

Windows 11 Pro x64 10.0.22631; Ryzen 7 5700G (six logical CPUs presented), approximately 8 GiB RAM. Rust/Cargo 1.99.1, stable-x86_64-pc-windows-msvc; Visual Studio Build Tools 2022 17.14.43, MSVC 19.44.35229.0, Windows SDK 10.0.26100.0; Python 3.12.14. Official PDFium chromium/7881 and qpdf 12.4.2. Microsoft Print to PDF is the only installed printer.

Release executable SHA-256: `f04cc2f5f55362a312989a1d793fc28988f7a7a555d9ccd8478dd3d67cc5f91a`.

One Cargo job and sequential suites were used. Generated fixtures, isolated configuration/data and a separate MSVC toolchain were used; dGK defaults and private documents were not used or changed.

## Results

| Check | Result |
| --- | --- |
| `build.ps1 -Release` | PASS, native MSVC release build |
| `cargo fmt --check`, `cargo check` | PASS |
| `cargo test` | PASS, 45 tests |
| `cargo clippy -- -D warnings` | PASS |
| Unmodified reader/pages/marks/overlays/aliases corpora | PASS |
| Revision/remark backend corpus | PASS |
| Review UI, protected-file and recovery corpora | PASS |
| About light/dark and scroll snapshots | PASS |
| Private-display isolation contract | PASS |
| Lazy/eager page geometry corpus | PASS, 32 cases |
| Reader/assembly/placement/history at scales 1, 1.5 and 2 | PASS, three runs |
| Studio at scales 1, 1.5 and 2 | PASS, three runs |
| Native Windows print diagnostic | FAIL in dialog locator; printed output unqualified |

The UI corpora now run directly with the repository's `DPDF_TEST_NATIVE_DISPLAY=1` option. The external Windows-display adapter required for v0.1.10 was unnecessary. The display-isolation invocation was rerun successfully after correcting a local PowerShell runner's scalar-array argument bug; this was not an application defect.

Review coverage includes Unicode notes and revision metadata, rotated/cropped pages, hidden/current/clean export, preservation of foreign popups/replies and protected-source aliases. Review UI helper-unavailable/retry, edit, mixed recovery and remarks-only restart checks passed. Alias coverage includes hardlinks and an existing local-loopback UNC path.

## Printing blocker and reproduction

With the built executable and the official helper paths:

```powershell
python scripts/check-windows-print.py --binary target/release/dpdf.exe --pdfium <pdfium.dll> --qpdf <qpdf.exe> --allow-native-dialog
```

The first case raises `AssertionError: No unambiguous native fixture print dialog:`. The diagnostic enumerates visible `#32770` dialogs owned by the child dPDF PID. On this Windows installation, the visible native print UI is hosted by `C:\Windows\explorer.exe`, titled `Printing from Win32 application. - Print`. Its accessibility tree exposes a Windows Print pane, Microsoft Print to PDF, Print and Cancel controls. This ownership/class mismatch prevents the diagnostic from finding the dialog.

A separate generated three-page `--test-native-print` probe reached the same Explorer-hosted UI. Cancelling it produced `{"metrics":[],"pages_drawn":[],"status":"cancelled"}`. Both fixture dialogs were closed and no dPDF process remained. No hardware printer was selected or used.

This establishes dialog opening/cancellation only. The first failing locator prevents this run from qualifying all/current/disjoint/overlapping ranges, color/order, fit/actual geometry or PDF output. Fix the diagnostic's modern Windows print-host association without blindly acting on arbitrary Explorer dialogs, then rerun. Application printing success is not claimed.

## Performance observation and limits

One warm headless run on generated PDFs reported first-page readiness of 8.91 ms (50-page office), 28.18 ms (1,000-page text) and 16.96 ms (50-page vector). These are diagnostic observations, not cold GUI benchmarks. Windows RSS was not reported.

Live Computer Use screen capture on this host continues to time out; automated application snapshots and native-dialog accessibility were available. Physical printing, installer/default-app integration, external About links and secure redaction baking are not qualified. No application fixes or main-branch changes were made.

Evidence retained locally under the isolated `dPDF-runtime/v0.1.14` artifact directory: Rust/corpus/smoke result JSON, per-suite logs, benchmark JSON and generated snapshots. Reports contain no private PDFs or credentials.
