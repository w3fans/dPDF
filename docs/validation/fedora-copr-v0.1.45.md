# Fedora COPR development preview — 2026-11-05

This is v0.1.45 development-preview packaging, not stable v0.2.0 acceptance.
The GitHub repository remains private. Only a sanitized committed source export,
locked Cargo dependency sources and pinned native-engine sources enter the SRPM.

## First successful build

- Project: [zvegelj/dpdf](https://copr.fedorainfracloud.org/coprs/zvegelj/dpdf/).
- Build: [11261771](https://copr.fedorainfracloud.org/coprs/build/11261771), **succeeded**.
- Target: Fedora 44 x86_64; source `bea0329cabe4efac72f8a52a0ff2c009ebefdd7f`.
- RPM: `dpdf-0.1.45-0.1793846141.gitbea0329cabe4.fc44.x86_64`.
- RPM SHA256: `8ef92537d7b656de0751a3cc19ac32dad0b4c35ade70cc51bfc20b1ba77ff050`.
- RPM size: 11,253,828 bytes; installed package files: 35,608,114 bytes.

PDFium 7881 and the Rust application compiled from source inside COPR with
buildroot networking disabled. PDFium completed 1,121 native build steps.
Fedora GN/Clang/LLVM/compiler-rt generated and compiled the native engine; no
precompiled engine or toolchain was hidden in the source RPM. Cargo used its
locked vendored sources offline. System qpdf is required rather than bundled.

## Installed-package results

A disposable Fedora 44 root enabled the actual COPR repository, verified its
signing key through DNF and installed the published RPM from that feed. The app
ran as a dedicated unprivileged fixture user, with generated documents only.

All **34** installed application checks passed: public launcher/automatic engine
lookup, reader/render/search/selection, page operations, geometry and physical
units, marks and overlays, review, clipboard/UI at 1x/1.5x/2x, passwords, recovery,
and controlled GTK Print to PDF. Generated-only block-probe checks passed;
production Secure Signature Block remains disabled.

The first run had 33 passes and one generated appearance comparison failure
(5.931882 against a limit of 5). The minimal root substituted Noto Sans for the
fixture's unembedded Helvetica. Installing test-only Liberation Sans made that
case and the full 34-check rerun pass with the same application binary and
unchanged limits. The initial failure evidence was retained.

The actual installed system desktop entry passed desktop-file validation and
opened a generated 50-page PDF via Gio. The existing fixture PDF association
remained unchanged. A normal package uninstall followed by reinstall from COPR
preserved all eight seeded user files byte-for-byte, including preferences,
synthetic mark assets and the prior PDF association. Package-owned application
and desktop files disappeared on uninstall; reinstall restored the same package.

## Portal-complete upgrade

[Build 11261854](https://copr.fedorainfracloud.org/coprs/build/11261854)
**succeeded** for source `bc37601165872f880c95459fb00558454fd66411`.
Its RPM is `dpdf-0.1.45-0.1793849252.gitbc3760116587.fc44.x86_64`,
11,253,927 bytes, SHA256
`716d98b97f63ff0dfcddba75c01305b2ea4e0fc116a986cf6951f1fea04cc9b8`.

A real DNF upgrade from the first COPR package passed, preserving all eight
seeded per-user files and the prior PDF association. DNF automatically installed
`xdg-desktop-portal-1.22.1-1.fc44` and `xdg-desktop-portal-gtk-1.15.3-3.fc44`.
The application and PDFium ELF files are byte-identical to the first package's
fully tested binaries. The installed source-revision file correctly identifies
the newer packaging commit. Its source RPM is published alongside the binary.

The first package did not show a chooser on the minimal target, which lacked
both portal packages. The upgraded package activates the GTK portal and displays
its chooser. Ctrl+O, selecting a generated PDF with a path containing spaces,
and loading it into the application passed with the existing PDF association
unchanged. The GTK dialog has an empty title; the test identifies its real
window class and waits for readiness rather than assuming an English title.
This additional check is separate from the passed installed desktop-entry open.

These tests do not qualify physical printing, ruler measurements, Wayland,
Windows hardware or owner acceptance. Fedora 45 is configured but untested.
Notification delivery and Windows validation remain separate gates. PR checks
must pass before merging; this record does not authorize tags or main pushes.

## Bounded release checksums

The source assembler and publisher now stream file checksums instead of reading
entire RPMs, vendor archives or PDFium snapshots into Python memory. On 2026-11-05,
the shared helper reproduced the recorded SHA256 values for build 11261854's
233,983,110-byte source RPM and 11,253,927-byte binary RPM. `tracemalloc` reported
394,337 bytes of peak Python allocations for each checksum call. This measures
Python allocations during hashing, not total process RSS or build memory. All
eight packaging contracts passed; no application rebuild was needed.
