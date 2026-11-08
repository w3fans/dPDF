# Hosted native Windows validation — v0.1.27

Exact source: `56ccb5c6fabae0036572cc3704897ce73869b1f0`.
[GitHub Actions run 40974026874](https://github.com/w3fans/dPDF/actions/runs/40974026874)
passed all 15 native MSVC service checks and 32 dedicated desktop checks. The
[compact report](windows-hosted-v0.1.27.json) retains exact identity and check names;
the corresponding validation branches retain full logs.

Desktop coverage now includes twelve-document tab overflow, keyboard wrapping,
resize/closure/reopen and reading-point preservation at 1×/1.5×/2×. Reflow checks
exercise single/continuous/facing views, fixed/fit-width zoom and rotation, with
sidebar/search visibility and physical resizing in both directions. Existing
clipboard, placement/history, review/recovery, password and Microsoft Print to
PDF checks also passed.

[private] Windows 11, physical printers, Explorer drag/drop, manual pointer interactions
and complete distribution review remain qualification work. Production secure
signature baking remains disabled. No stable milestone follows from this report.
