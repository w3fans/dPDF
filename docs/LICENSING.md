# Product and dependency licenses

The owner approved this licensing arrangement on 2026-10-29. dPDF-owned work is
licensed under Apache-2.0; see the root LICENSE and NOTICE.

Use [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0) for
dPDF-owned source, UI, documentation and generated test fixtures. The official license
text is LICENSE and Cargo.toml identifies Apache-2.0. Retain third-party
licenses/notices separately. This grants rights to dPDF-owned work; it does not
relicense its dependencies.

For public open-source releases, Slint 1.18.1 uses its **GPL-3.0-only** alternative.
The combined application is distributed under GPLv3; dPDF-owned files retain
Apache-2.0. Slint explicitly permits this combination in its
[upstream licensing explanation](https://github.com/slint-ui/slint/blob/v1.18.1/LICENSE.md).
The GPL text is retained in `packaging/licenses/slint/GPL-3.0-only.txt`.
Fedora COPR source RPMs include the exact product source, build scripts and all
locked Cargo dependency sources. Keep the source RPM available alongside binaries.

The Fedora preview builds PDFium chromium/7881 from upstream commit
`91b9d569b34be4f38eed7b3c49b227356c3aadad`, using the immutable component
revisions in `packaging/fedora/pdfium-sources.json`. PDFium's top-level license is
BSD-3-Clause; its dependencies retain their individual permissive/font licenses
and notices. The source bundle retains these texts, and the RPM installs the
collected notices under its PDFium license directory. The recipe uses Fedora's maintained GN build tool and compiles the native shared
engine using Fedora's Clang toolchain, without upstream binary engine/toolchain
downloads. Fedora's `deepin-pdfium` is not substituted: its
version and ABI have not been qualified against this pinned binding.

Fedora's separately maintained `qpdf >= 11` is a runtime/build dependency, using
qpdf's Apache-2.0 distribution option. The RPM does not bundle another qpdf
executable. Functional compatibility is checked against the installed Fedora
version rather than inferred from the dependency constraint alone.

Bundled PDFium is maintained by the dPDF release owner: review upstream PDFium
and its bundled-library security updates before each preview, update immutable
revisions and notices together, rebuild offline, and rerun installed-package
tests. A pinned revision makes a build identifiable; it is not a claim that the
engine has no vulnerabilities or receives automatic Fedora engine updates.

Earlier previews selected Slint's
[royalty-free desktop license](https://github.com/slint-ui/slint/blob/v1.18.1/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md)
instead. It permits distribution as part
of a desktop application subject to its attribution and other conditions. Retain the official attribution
widget in About dPDF and badge in the public README and release download pages.
Preserve the exact dependency license in staged runtimes. Do not describe Slint
itself as Apache-licensed or assume these desktop terms cover other uses.

MuPDF offers [AGPL or commercial licensing](https://mupdf.readthedocs.io/en/latest/license.html).
The capability probe does not choose it as dPDF's engine, link it into dPDF, or bundle
it in distributions. Keep it out of the current application
distribution until an engine/license decision and the secure-region acceptance
gate are both satisfied. A subprocess boundary alone is not a license conclusion.

Before qualified binary releases, generate a manifest of the actual bundled Cargo
and native dependencies, retain their required texts/notices, and resolve any
remaining distribution obligations. This selection is not a completed dependency
license audit. Public source previews do not establish installer qualification.
