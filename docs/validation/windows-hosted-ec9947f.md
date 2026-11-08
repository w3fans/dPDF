# Hosted native Windows validation — ec9947f

Date: 2026-10-29. Automatic GitHub-hosted Windows lane, native x64 MSVC.

**PASS** for exact source commit `ec9947fe8da69624b32bfbf33fd0b3a4d2d11a11`.
This is the v0.1.15 application plus CI/bootstrap fixes, not a separate stable release.

Run: https://github.com/w3fans/dPDF/actions/runs/40868169653

Full automatic JSON/Markdown report and job metadata are on
`validation/ci-windows-hosted-40868169653-1` under
`docs/validation/automated/`. The development agent fetched these directly using
the configured SSH Git identity. No owner relay or HTTP credential was needed.

| Check | Result |
| --- | --- |
| Native CI process-tree/isolation contracts | PASS |
| PowerShell syntax | PASS |
| Standard build.ps1 -Release | PASS; 596.7 s cold build |
| Optimized locked Rust tests | PASS; 35.6 s |
| Strict Clippy | PASS; 178.8 s |
| rustfmt | PASS |
| Reader/search/selection | PASS |
| PDF/image page assembly | PASS |
| Mark cleanup/library/physical sizes | PASS |
| Overlay export/preservation | PASS |
| Review revisions/visibility/export | PASS |
| Protected-file aliases | PASS |
| 32-case geometry comparison | PASS |

Rust/Cargo 1.99.1, x86_64-pc-windows-msvc, LLVM 23.1.1;
Windows 10.0.26100 (Windows 11 ServerDatacenter), 64-bit.
PDFium chromium/7881 and qpdf 12.4.2 matched the pinned archive digests.
Every compiler/test child tree used the one-CPU-equivalent/2-GiB Job Object;
Cargo ran one build job. No application code was changed by validation.

Unqualified: [private] machine/service setup, interactive Windows desktop/input,
password/recovery windows, native print dialogs, physical printer output,
installer/default-app integration, and the later relocated-package staging test.
A service/headless PASS is not desktop or paper-output acceptance.
