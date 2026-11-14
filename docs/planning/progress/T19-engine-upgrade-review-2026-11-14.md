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


## Accepted main reconciliation and minimum compiler gate (14 November)

This review candidate now includes accepted main 68910141 as a normal merge
parent, retaining qualified 5456aa4f. Only changelog insertion conflicted; keep
both entries. Binding/runtime pair, lockfile, both target inventories, Fedora
source manifest, source-only collector and build adapter are byte-identical to
the qualified engine candidate. The new main supplies native Rust 1.92 source
checks, test-only mark diagnostics and portable/MSI checks.

Prior stable compiler/runtime qualification does not establish this combined
source or its advertised minimum compiler. Fresh hosted gates on both native
targets and affected exact-source comparisons remain PENDING. The canonical
8125 source bundle's upstream revisions and collector inputs are unchanged;
this is not a new Fedora GN/C++ build or installed-package qualification.
Experienced engine review and actual Fedora source-build/operator evidence
remain REQUIRED. Keep this PR draft; no security clearance, release, tag,
version or production baking approval follows from CI or this reconciliation.

Sixteen local source suites pass: 129 tests, 125 passed/four existing platform
skips, including all five source-collector cases. Formatting, metadata, local
links/JSON/whitespace checks pass; no local Rust or C++ compilation. New native
minimum-compiler and ordinary hosted/runtime evidence remain PENDING.


## Final accepted-main conflict resolution for review

Readiness check found this candidate conflicted against accepted main 414a040f.
Normally merge that main into qualified 3f52fdda. Only changelog insertion
conflicted; retain both entries. Every Rust source, manifest/lock and UI byte is
unchanged, as are binding/runtime/source-collector/package inputs. Main adds the
qualified strict registration diagnostics and three source-safe contracts.
Fresh exact-source hosted/native/minimum-compiler gates remain PENDING. Prior
63-case/JPEG/three-scale compatibility evidence remains source-bound; do not
relabel it as a new run. Experienced engine review, actual Fedora GN/full C++
source-build/package/operator evidence remain REQUIRED. Keep draft. No new
build route, source bundle, release/version/tag or production baking change.

Local final-integration checks pass: sixteen suites, 132 tests: 128 pass/four existing skips. Formatting
and whitespace pass; no local Rust/C++ compilation. Hosted gates remain pending.


### Qualified recovery lock fix integration

Normally merge accepted main 30767936, retaining the prior candidate. The exact
qualified main recovery guard/regression delta is the only Rust production
change; all other source, manifests/lock and UI bytes remain unchanged. Only
changelog insertion conflicted and both entries are retained. Main's focused
fix passed every gate on a68ebe6b: new duplicate-handle regression and original
handoff in both Linux profiles, plus Windows portable handoff and both native
minimum compilers. The original transient descriptors were not captured, so
attribution remains inferred; unrelated historical failures are not diagnosed.

Fresh combined-source hosted/native/minimum-compiler and affected runtime gates
remain PENDING. No new ownership/engine implementation is promoted by this
integration. Existing experienced lifecycle/engine review, actual Fedora source
build and manual/operator gates remain REQUIRED; keep draft. No version/tag/
release, resource-cap/private build route, runtime pin or production baking change.

Local recovery-integration checks pass: sixteen suites, 132 tests: 128 pass/four existing platform skips.
Formatting/whitespace pass; no local Rust/C++ compilation. Hosted gates pending.
