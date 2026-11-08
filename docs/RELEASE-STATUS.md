# Development preview delivery status

## v0.1.54 — published and verified

Product versioning continued from 0.1.53 through the existing standard patch
controller. The public repository contains reviewed source without importing
prior private Git history or local agent state.

Source: `e06b05b5f1e91e9e9e99b97967842bad4996bc67`.

| Component | Result | Evidence |
|---|---|---|
| Release notes and downloads | PASS | [v0.1.54](https://github.com/w3fans/dPDF/releases/tag/v0.1.54) |
| Hosted Linux validation | PASS | [Actions run](https://github.com/w3fans/dPDF/actions/runs/42484352471) |
| Hosted native Windows validation | PASS | [Actions run](https://github.com/w3fans/dPDF/actions/runs/42484352484) |
| Windows MSI and portable ZIP | PASS | [Installer qualification](https://github.com/w3fans/dPDF/actions/runs/42487270092) |
| Fedora 44/45 source builds | PASS | [COPR build 11287475](https://copr.fedorainfracloud.org/coprs/zvegelj/dpdf/build/11287475/) |
| Fedora installed qualification | PASS | Each target: 34 application checks, nine controlled CUPS cases, upgrade/lifecycle and desktop checks |
| Fedora acceptance desktop | PASS | Upgraded to 0.1.54; package/source verification and native Wayland smoke passed; PDF default preserved |
| Published downloads | PASS | Actual Windows and Fedora binary downloads matched SHA256; portable executable matched exact-source native provenance |

The initial Windows packaging attempt failed because its legacy child host could
not resolve Get-FileHash. Packaging was recovered with the current PowerShell
host and the already validated immutable native executable. The tag and
application version were preserved; no replacement build was substituted.
Detailed initial failures remain in their workflow history.

Source/history secret scans and private-reference checks passed. The portable
package's 407 readable text files also passed the private-reference scan.

This is a development preview. Physical Windows desktop, physical-printer,
ruler and owner acceptance remain separate stable-reader gates. Production
Secure Signature Block remains disabled.

## Routine publication

Use the established `./release.sh patch` on the registered release authority.
The public workflows create notes and Windows assets; the existing local COPR
operator handles Fedora source builds and verified attachments. Keep one
publisher per component. Existing-tag recovery never allocates another version.
