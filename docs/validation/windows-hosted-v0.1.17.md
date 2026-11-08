# Hosted native Windows validation — v0.1.17

Exact source: `858725774f52b7290257c1ce1799803b12944cfe`.
[GitHub Actions run 40883190056](https://github.com/w3fans/dPDF/actions/runs/40883190056)
reported PASS for both native MSVC lanes. The compact imported
[JSON summary](windows-hosted-v0.1.17.json) retains exact identity and check names.
Full generated reports/logs remain on the diagnostics-only branches:

- `validation/ci-windows-hosted-40883190056-1`: 14 checks passed, including
  optimized tests, strict Clippy, all headless PDF corpora and relocated runtime.
- `validation/ci-windows-desktop-40883190056-1`: the same checks plus About,
  review UI, protected documents, crash recovery and native printing: 19 passed.

The desktop lane verified session 2, a visible window station and accessible input
desktop before running. Native Microsoft Print to PDF passed all/current/disjoint
and overlapping range cases, output colors/order, fit/actual-size geometry,
cancellation and input-file protection. This is an actual Windows execution result,
separate from the Linux Win32 metadata compile check.

These fresh GitHub-hosted Windows VMs do not establish [private] Windows 11 qualification,
physical printer output, manual pointer/monitor behavior, Explorer file drops,
installer/default-app integration or complete dependency distribution review.
The [private] job remains queued until its separately authorized runner is installed.
The new six-case text/image clipboard and placement scale matrix was added after
this release and is not covered by this v0.1.17 report.
