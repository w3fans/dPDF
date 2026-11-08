# Fedora v0.1.46 installed qualification

Published and tested on 2026-11-06 from exact tag `v0.1.46`, source
`a8244447c9417e21cf3064a768b7de96f1e09735`.

Both source-built targets succeeded in [COPR build 11269011](https://copr.fedorainfracloud.org/coprs/build/11269011/).
PDFium 7881 completed all 1121 native build steps, then the locked Rust application
built offline. The package uses system qpdf and includes the selected full-product
GPL-3.0-only/owned-source Apache-2.0 and dependency notices.

| Target | Installed package | Result |
| --- | --- | --- |
| Fedora 44 x86_64 | `dpdf-0.1.46-1.fc44.x86_64` | PASS |
| Fedora 45 x86_64 | `dpdf-0.1.46-1.fc45.x86_64` | PASS |

Each actual signed package was upgraded from the real COPR feed in a disposable
Fedora root. Each passed 34 application checks and nine loopback-only CUPS cases,
installed menu/Open chooser, and uninstall/reinstall. Upgrade and lifecycle tests
preserved all eight seeded user files, including marks, preferences and the prior
PDF association. Generated fixtures and isolated non-root test accounts were used.

Application coverage includes reader/search/selection, page replacement and
assembly, overlays, physical PDF units, marks, review, aliases, independent PDF
checks, desktop behavior at scales 1/1.5/2, passwords, recovery and controlled file
printing. CUPS cases cover all/single/contiguous/disjoint pages, source-path change,
cancel, HTTP/IPP refusal and preparation fallback. Vector/text and page order were
checked; submission failures never trigger a duplicate automatic retry.

## Artifact identity

- `fedora-44-x86_64/dpdf-0.1.46-1.fc44.src.rpm`: 235,439,980 bytes, SHA256 `f6857a305f0e241e9d708cd65a1ca5c06bd8eee244ae8065ca4290c20d6c5ef0`.
- `fedora-44-x86_64/dpdf-0.1.46-1.fc44.x86_64.rpm`: 11,320,904 bytes, SHA256 `10202966d54b1df1ea9c61994f6f603d4cf5265c95de7107668b009a2cbed15b`.
- `fedora-45-x86_64/dpdf-0.1.46-1.fc45.src.rpm`: 235,439,976 bytes, SHA256 `04797e9a4ac0d34ac03149a55f04a4ec2aca5d4e0339c74e3f70185295dce66a`.
- `fedora-45-x86_64/dpdf-0.1.46-1.fc45.x86_64.rpm`: 11,354,475 bytes, SHA256 `5e04749848e6c7a8fe51af2b3067d4098e49e5e37ae7a9a26241ee5be5d79eaa`.

Installed executable SHA256:

- Fedora 44: `f67ba0d3fbdaa444fa9bf47421daa051dec9e11b402f9dd9bfb1363273550311`.
- Fedora 45: `b9871018beb34b733b326ff89863b2a772d130e45698b705b352a837fa71f081`.

## Install or update

```sh
sudo dnf copr enable zvegelj/dpdf
sudo dnf install dpdf
```

For an existing installation, use `sudo dnf upgrade --refresh dpdf`.
Launch `/usr/bin/dpdf` if other development launchers exist.
The source marker is `/usr/share/dpdf/SOURCE-REVISION`.

This qualifies the controlled Fedora preview, not physical printers/ruler tests,
Wayland, native Windows or owner acceptance. Production Secure Signature Block
remains disabled. The release used the owner's explicit passed-local-checks
exception while hosted jobs could not start. The Git tag and COPR packages exist;
the separate GitHub Release page was not published by those blocked jobs.
GitHub visibility remains private by owner instruction.
