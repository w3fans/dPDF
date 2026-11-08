# Hosted native Windows validation — v0.1.23

Exact source: `917367588efc01b043091180669970d9e43f3cb2`.
[GitHub Actions run 40966149182](https://github.com/w3fans/dPDF/actions/runs/40966149182)
passed 15 native MSVC service checks and 26 dedicated desktop checks. The
[compact report](windows-hosted-v0.1.23.json) records identity and check names;
full logs remain on the corresponding `validation/ci-windows-*` report branches.

The lanes include native per-user registration, icon and association resolution,
preservation of PDF defaults/UserChoice, PDF rendering and page exports, overlay
and review/recovery suites. The desktop lane additionally covers generated text/
image clipboard, placement/history/export at 1×/1.5×/2× and native Microsoft Print
to PDF. The generated block-baking experiments are separate Fedora investigations
and are not qualified by these Windows results.

[private] is queued. Physical printers, Windows 11 on [private], Explorer drag/drop and manual
desktop/installer acceptance remain unqualified. No stable milestone is claimed.
