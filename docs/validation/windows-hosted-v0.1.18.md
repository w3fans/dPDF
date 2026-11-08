# Hosted native Windows validation — v0.1.18

Exact source: `9c21d858a712475299bdbeff70b7949c0e27583a`.
[GitHub Actions run 40914877718](https://github.com/w3fans/dPDF/actions/runs/40914877718)
reported PASS for native MSVC service (14 checks) and desktop (25 checks) lanes.
The imported [compact JSON](windows-hosted-v0.1.18.json) retains exact identity and
check names; full reports/logs are on the corresponding
`validation/ci-windows-{hosted,desktop}-40914877718-1` branches.

The desktop lane passed text copy, on-page placement/move/resize, Undo/Redo,
numeric editing, source protection and export staleness, plus studio image paste
and captured-image persistence at 1×, 1.5× and 2×. It also passed About, review UI,
password/recovery suites and native Microsoft Print to PDF.

The explicit clipboard opt-in runs only after the dedicated desktop context is
verified; the service lane never enables it. No physical printer was used.
These results do not qualify [private] Windows 11, manual physical monitor/pointer
behavior, Windows Explorer OLE file drops, native installers/default associations,
or complete third-party distribution review. The native OS file-drop fix was
added after this release and is not covered by these results.
