# Fedora v0.1.48 installed qualification

Development preview `v0.1.48`, exact source `dff7afba9d4c49619a2a5e90d4c4e3604a5f1767`,
was published by the local tag watcher and qualified on 2026-11-06.

Both targets succeeded in [COPR build 11269416](https://copr.fedorainfracloud.org/coprs/build/11269416/).
The publisher verified both binary/source pairs, embedded source revisions and
package checksums. One source submission served both distribution targets; the
worker resumed no competing build.

| Target | Installed RPM | Application | Controlled CUPS | Upgrade / lifecycle |
| --- | --- | --- | --- | --- |
| Fedora 44 x86_64 | `dpdf-0.1.48-1.fc44.x86_64` | 34 PASS | 9 PASS | PASS; all eight seeded user files preserved |
| Fedora 45 x86_64 | `dpdf-0.1.48-1.fc45.x86_64` | 34 PASS | 9 PASS | PASS; all eight seeded user files preserved |

Each signed package was upgraded from v0.1.46 through the real COPR feed in a
disposable Fedora root. Tests exercised the installed launcher and engine,
reader/search/selection, page replacement and assembly, marks, physical PDF units,
review, aliases, independent PDF checks, X11 UI at scales 1/1.5/2, passwords,
recovery and controlled print-to-file. The actual installed menu and Open chooser
worked. Uninstall removed package files; reinstall preserved marks, preferences
and the prior PDF association. No normal host RPM installation was changed.

CUPS cases covered all/single/contiguous/disjoint selections, changed source path,
cancel, HTTP/IPP refusal and preparation fallback. Vector/text and page order were
checked. Submission errors did not trigger duplicate automatic printing.

An initial application-harness attempt lacked the generated test-source revision
marker and stopped before exercising the application. Its failure log was retained.
After correcting that fixture, all 545 staged source files were checked against
the immutable tag archive plus revision marker on both targets. The failed phase
and every remaining required phase then passed; no RPM rebuild or resubmission
was needed.

## Reproducible source and licenses

PDFium 7881 completed all 1121 native steps on both targets; the locked Rust
application built offline. Slint 1.18.1 uses GPL-3.0-only for the combined product;
dPDF-owned source retains Apache-2.0. The exact source RPMs include vendored Cargo
sources, pinned native sources and dependency notices. Fedora system qpdf is used.
See [LICENSING.md](../LICENSING.md) and [FEDORA_RPM.md](../FEDORA_RPM.md).

## Artifact identity

- `fedora-44-x86_64/dpdf-0.1.48-1.fc44.src.rpm`: 235,437,394 bytes, SHA256 `2480662f1ef96200696d8c98973bbc8b28d9af070f736217e07b66e145006888`.
- `fedora-44-x86_64/dpdf-0.1.48-1.fc44.x86_64.rpm`: 11,324,065 bytes, SHA256 `37ad54e4c1e8134af2c836b52ce47eb95fbb245c0edd95aabacd2bbbb91bd03e`.
- `fedora-45-x86_64/dpdf-0.1.48-1.fc45.src.rpm`: 235,437,384 bytes, SHA256 `b0d6264d081ca35e0654aa908d47d6d6c2820da09de58cf896b17ba33768b455`.
- `fedora-45-x86_64/dpdf-0.1.48-1.fc45.x86_64.rpm`: 11,353,625 bytes, SHA256 `733baf70a1359739498826e04ed20b9e0f225f9e5690ab7ffa612bd8b1775d7c`.

Installed executable SHA256:

- Fedora 44: `a090b3b75759d41d2be411e8711b9fc3f7a87f6b27502bd6cf77f60cd7fc9159`.
- Fedora 45: `a68936f63e8e92a2a5f6c6363d22aef217add817cdb644bdfcd5a7ec22827b24`.

## Install or update

```sh
sudo dnf copr enable zvegelj/dpdf
sudo dnf install dpdf
```

For an existing installation: `sudo dnf upgrade --refresh dpdf`.
Launch `/usr/bin/dpdf` if another development launcher shadows the installed app.
The package records its source under `/usr/share/dpdf/SOURCE-REVISION`.

The local watcher submitted this release automatically after the normal release
controller passed 68 Rust release tests, formatting, both strict Clippy modes and
the required CLI/PDF gates. COPR instructions were updated and read back after
both installed qualifications passed. This proves the local package-publication
path; it does not claim a separate GitHub Release page or attachment was created.

Physical printers, ruler measurements, Wayland, native Windows and owner acceptance
remain unqualified. Production Secure Signature Block stays disabled and the
channel remains v0.1.x development preview. GitHub visibility remains private by
owner instruction; current-source export checks do not clear historical refs.
