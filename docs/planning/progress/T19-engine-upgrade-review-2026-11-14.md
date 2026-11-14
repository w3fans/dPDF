# T19 engine upgrade review candidate

Status: pre-implementation provenance/compatibility review snapshot. This branch
now selects the proposed pair; see T19-engine-8125-2026-11-14.md for pending gates.
Existing 7881 JPEG source-match finding requires separate engine review. This
record does not qualify a new engine, suppress an advisory or authorize release.

Candidate pair: published pdfium-render =0.9.5 with explicit pdfium_8125,
thread_safe and image_025; matching immutable PDFium 158.0.8125.0 x64 releases.
The crate archive checksum was verified against crates.io metadata. Its retained
LICENSE.md bytes and MIT OR Apache-2.0 declaration match current 0.9.4 exactly;
no license choice is changed. Official release documentation also describes a
bitmap-buffer overflow fix, page-cache correction and less eager structure loading.
See [binding documentation](https://docs.rs/pdfium-render/0.9.5/pdfium_render/)
and [runtime release](https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium%2F8125).

Both new archives were downloaded and matched upstream asset SHA-256 digests;
embedded VERSION and args.gn identify 8125 with V8/XFA disabled. Resolving the
upstream 8125 branch yielded immutable source 43d55e429cf72c21e9a1e60475b50feefa16fda9.
That source contains owned JPEG correction and observed-self form lifetime checks;
its cjs_global.cpp bytes exactly match the inspected V8 fix revision. These are
source observations; archive VERSION/configuration alone is not a complete binary
build attestation or an exhaustive vulnerability scan.

[Candidate receipt](T19-pdfium-8125-review-candidate.json) retains all exact archive,
crate, license, source and configuration hashes. Scratch inspection material is
/tmp/dpdf-engine-upgrade-review. No compiler, installer or release operation ran.

The reviewable implementation must update binding/lock/features, both Rust notice
inventories, Linux/Windows archive pins, minimal Fedora root/DEPS manifest and
Source2 name, reviewed source-only collector guard and affected build/runtime docs
together. Current collector deliberately accepts only reviewed 7881; do not bypass
it or substitute downloaded binaries for Fedora's source build. Inspect new DEPS
and distro-toolchain assumptions, retain source notices, and keep the existing
publisher/controller. Do not infer current build compatibility from a minor crate
version or a newer engine number.

Before merge: full approved source-bound Linux/MSVC debug/release/Clippy and PDF
geometry/text/untouched-stream/alias/password/review/printing/window gates; exact
notice comparison; malformed-JPEG regression where available; Fedora source-build
and package/operator evidence; experienced engine review. Preserve manual/native
Windows, Wayland, physical printer and real-user gaps explicitly. This work may
proceed independently of the unmerged T03 lifecycle drafts, but any later combined
source needs its affected gates repeated. Production baking and release authority
remain unchanged.
