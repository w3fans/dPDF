# Fedora v0.1.49 targeted installed qualification

Development preview `v0.1.49`, source
`1c80fd7df3e3844eae3e48cfe1fd272bb874e5a0`, passed its normal release gates
and targeted installed-package checks on Fedora 44/45 x86_64 on 2026-11-06.
Both source builds succeeded in [COPR build 11270028](https://copr.fedorainfracloud.org/coprs/build/11270028/).
The local publisher made one submission for both targets and verified each
binary/source pair and its embedded source revision.

## Scope and results

The application runtime source, locked dependencies, native build recipe,
packaging and license inputs are unchanged from v0.1.48, except for dPDF's own
package version. An exact Git/Cargo comparison established this before testing.
The [full 34-application/9-CUPS qualification](fedora-copr-v0.1.48.md) belongs to
v0.1.48. Those complete suites were not rerun or claimed for v0.1.49.

For each actual signed RPM, a disposable Fedora root passed:

- Exact-version upgrade from v0.1.48 through the real COPR feed.
- Installed launcher version, source revision, 50-page PDF rendering and text search.
- One disjoint-page controlled CUPS regression, checking vector/text output and order.
- Uninstall/reinstall with all eight seeded per-user files and PDF association preserved.
- Actual installed desktop-menu and Open-chooser launches.

| Target | Installed package | Installed executable SHA256 |
| --- | --- | --- |
| Fedora 44 x86_64 | `dpdf-0.1.49-1.fc44.x86_64` | `967042b12507577b2a627e03538607be4ddcd0e814ef4b14ca2331722c8d2b8a` |
| Fedora 45 x86_64 | `dpdf-0.1.49-1.fc45.x86_64` | `052d8aacb48b5589b7e07a6d6c8cd9d233cef5dd461493348ae3e4433d7cac8b` |

The normal release controller also passed 68 Rust release tests, strict Clippy
in both configured modes and CLI/PDF gates. The new source-export guard passed
four regression tests, including binary filenames and directory names. The
immutable tagged product export passed the private-reference scan (551 files).

## Source and artifact identity

PDFium 7881 and the locked Rust application were built from source; Cargo inputs
were vendored for offline compilation. The package uses system qpdf. Combined
product licensing remains GPL-3.0-only via Slint 1.18.1; dPDF-owned source retains
Apache-2.0, and source/license notices are included. See [LICENSING.md](../LICENSING.md)
and [FEDORA_RPM.md](../FEDORA_RPM.md).

- `fedora-44-x86_64/dpdf-0.1.49-1.fc44.src.rpm`: 235,442,924 bytes; SHA256 `7084ee899dbae4da6d1e7f362dda4f1beabea87ad41d9ed4209efa3372414ba2`.
- `fedora-44-x86_64/dpdf-0.1.49-1.fc44.x86_64.rpm`: 11,321,799 bytes; SHA256 `0683a7c145191e6462b70b9543bf2835b61426dd0f0f5f99103fa3e8bc6e39e0`.
- `fedora-45-x86_64/dpdf-0.1.49-1.fc45.src.rpm`: 235,442,920 bytes; SHA256 `945120cc03201ce3e15c24220cfa01985efbb034e8b2319045c737d761d46b17`.
- `fedora-45-x86_64/dpdf-0.1.49-1.fc45.x86_64.rpm`: 11,355,450 bytes; SHA256 `225b3420cb5023606798b2b7d0f692f107f35a6e76fbadff6419b3e8d867d910`.

## Install or update

```sh
sudo dnf copr enable zvegelj/dpdf
sudo dnf install dpdf
```

For an existing installation: `sudo dnf upgrade --refresh dpdf`.
Launch `/usr/bin/dpdf` if a development launcher shadows it. Package source
identity is installed at `/usr/share/dpdf/SOURCE-REVISION`.

COPR instructions were updated only after both installed qualifications passed
and were read back through the project API. GitHub Release pages and attachments
remain a separate pending publication stage; a tag is not treated as a page.

This is a v0.1.x development preview. Physical printing/ruler measurements,
Wayland, current native Windows and owner acceptance are not claimed. Production
Secure Signature Block remains disabled. GitHub visibility remains private by
owner instruction; current-source checks do not clear historical refs/assets.
