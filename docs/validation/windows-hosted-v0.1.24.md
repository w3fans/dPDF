# Hosted native Windows validation — v0.1.24

Exact source: `262869937c6ab3aa2abf3ff913adb96199c3b29a`.
[GitHub Actions run 40968879126](https://github.com/w3fans/dPDF/actions/runs/40968879126)
passed all 15 MSVC service checks and 26 dedicated desktop checks. The
[compact report](windows-hosted-v0.1.24.json) records check names and exact identity;
full logs remain on the corresponding validation report branches.

This includes the new saved-asset usage checks: choosing/cancelling placement
leaves timestamps alone, placing records use while retaining the preferred size,
and concurrent metadata updates preserve asset files. Clipboard/placement/studio
checks run at 1×/1.5×/2×, alongside existing export, review/recovery, registration
and native Microsoft Print to PDF checks.

No physical printer, [private] Windows 11 session or Explorer drag/drop was qualified.
The generated block experiment remains separate and production secure baking is
disabled. This report does not establish a stable milestone.
