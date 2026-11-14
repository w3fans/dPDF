# Dependency updates and package qualification

Inventory checked 2026-11-13 against source baseline
`268b234e4336789922a6b15dd1236963e131d707` and the working T01/T02/T04 changes.
The initial snapshot is T19 inventory/procedure work. Subsequent scoped updates
are recorded below; no new installer acceptance follows from them. [Release status](RELEASE-STATUS.md) retains published evidence.

## Ownership and supported baseline

The contributor preparing an update owns its source inventory, advisory triage,
regression results and notices. Routine development PRs follow DEVELOPMENT_WORKFLOW merge gates; engine,
lifetime or licensing changes retain their required experienced review. The
designated release authority integrates qualified development for delivery and
remains the only version/tag writer. The Windows
qualification operator supplies native receipts; the Fedora package operator uses
the existing single COPR publisher. These are project roles, not newly assigned
people or services. An unavailable reviewer/operator is a pending gate.

| Component | Current configuration | Source of truth / update coupling |
| --- | --- | --- |
| Rust | Edition 2024, declared minimum 1.92; hosted workflows install stable | Cargo.toml, build.ps1 and CI; record actual rustc/Cargo/MSVC versions per artifact. Minimum version is not an exact compiler pin; test declared minimum separately before claiming MSRV support. |
| PDFium binding | pdfium-render =0.9.4 with pdfium_7881, thread_safe and image_025 | Cargo.toml/lock; change API selection together with engine revision and distributions. Preserve engine-thread ownership. |
| PDFium runtime | chromium/7881; Fedora root revision 91b9d569b34be4f38eed7b3c49b227356c3aadad | packaging/fedora/pdfium-sources.json includes dependency revisions; Linux CI URL/hash in ci.yml; Windows URL/hash in scripts/ci/runtime.py. Fedora builds source rather than repackaging downloaded binaries. |
| UI | Slint and slint-build =1.18.1, software renderer/Winit 0.30 ABI | Cargo.toml/lock and retained GPL/desktop attribution texts; review binding/compiler/license compatibility together. |
| Vendored Winit | 0.30.13 plus bounded Wayland receive changes | third_party/winit/DPDF_PATCH.md: upstream e9809ef54b18499bb4f2cac945719ecc2a61061b, archive SHA256 a6755fa58a9f8350bd1e472d4c3fcc25f824ec358933bba33306d0b63df5978d. Retain patch provenance and upstream license. |
| Image/clipboard | image =0.25.10 (PNG/JPEG only); arboard =3.7.0 | Cargo.toml/lock; image input limits, EXIF, transparency and both clipboard backends need affected tests. |
| Native interfaces | windows-sys =0.61.2, libloading =0.8.9 on Linux | Cargo.toml/lock; native Windows tests required in addition to check-windows-print metadata compilation. |
| qpdf | Application minimum 11; mixed-angle assembly requires 12; Windows bundle 12.4.2 | scripts/ci/runtime.py includes archive digest; packaging/licenses/qpdf and third-party notices; Fedora uses distribution packages. Record actual installed version. |
| Installer | WiX 5.0.2 local .NET tool | packaging/windows/build-msi.ps1 verifies tool version; no global tool install. Windows app-local MSVC runtime comes from qualified qpdf distribution. Record actual .NET/PowerShell host; the packaging-host regression must stay fixed. |
| Other Rust packages | Cargo.lock; some manifest constraints are ranges | Regenerate both target notice inventories after changes. A locked build does not imply an advisory-free dependency graph. |
| System print libraries | GTK3, GLib/GObject, Cairo, CUPS | Distribution-managed; record package versions in native print receipts. No new bundled copy. |

Linux CI PDFium archive SHA256:
`1470e21b8b4a3b4ad7f85684e2da11d94f3b69a86d81dee11b9b6709d927ac1d`.
Windows PDFium archive SHA256:
`73cc0de638ac2095e7445bf56a38200a5b7c7ca0e9f4ba144598f2457377ac08`.
Windows qpdf archive SHA256:
`db87077e683630c1217e0e8f9a20a9749d952ab676e881c3689187763a5de25d`.
These are configured hashes, not fresh archive downloads in this receipt.

## Advisory triage on 2026-11-13

- Google's [November 3 stable-channel bulletin](https://chromereleases.googleblog.com/2026/11/stable-channel-update-for-desktop.html)
  lists CVE-2026-121166, a PDFium type-confusion fix, in Chrome 157.0.8103.38/.39.
  The [pinned binary release](https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium%2F7881)
  identifies PDFium 151.0.7881.0. **Applicability to our exact source revision is
  unresolved**: Chrome version ordering alone cannot establish whether this build
  contains the vulnerable code or a backport. Follow the linked upstream bug/fix,
  map it to the Fedora root revision and binary provenance, and select a reviewed
  compatible upgrade/backport if affected. Do not report the pin as secure.
- [qpdf's official release notes](https://qpdf.readthedocs.io/en/latest/release-notes.html)
  identify 12.4.2 and robustness changes. The
  [upstream releases page](https://github.com/QPDF/QPDF/releases) also reports 12.4.2.
  This matches the bundled version observed here; it is not proof all system
  qpdf installations are current or free of vulnerabilities.
- RustSec's [image index](https://rustsec.org/packages/image.html) lists
  [RUSTSEC-2020-0073](https://rustsec.org/advisories/RUSTSEC-2020-0073.html)
  (patched >=0.23.12) and
  [RUSTSEC-2019-0014](https://rustsec.org/advisories/RUSTSEC-2019-0014.html)
  (patched >=0.21.3). Configured image 0.25.10 is outside those affected ranges.
  That conclusion covers only these two advisories, not the whole lockfile.
- Inspected [Winit security](https://github.com/rust-windowing/winit/security) and
  [Slint security](https://github.com/slint-ui/slint/security) entry pages.
  Public page availability is not an advisory scan. The RustSec Winit package
  page could not be retrieved. Full lockfile/target advisory coverage, including
  transitive crates and vendored changes, remains **PENDING**. Use an approved
  advisory scanner/database on the controlled build host and retain its version,
  database revision, target scope and unresolved exceptions.
- [pdfium-render upstream](https://github.com/ajrcarey/pdfium-render) is the binding
  compatibility reference; compile-time API selection must match the chosen engine.
  No newer binding/runtime pair has been tested by this inventory.

No dependency version changed in the initial inventory. Advisory triage must be refreshed before a release;
this document is dated evidence, not an evergreen vulnerability attestation.

### PDFium applicability follow-up (2026-11-13)

The upstream [fix commit cce5c5ed801f54cadb5b039d98ed43804994764c](https://pdfium.googlesource.com/pdfium.git/+/cce5c5ed801f54cadb5b039d98ed43804994764c)
changes JavaScript global-property enumeration and references issue 566111382.
The pinned [source revision](https://pdfium.googlesource.com/pdfium/+/91b9d569b34be4f38eed7b3c49b227356c3aadad)
is dated June 5, 2026; the fix is dated September 28. Dates do not prove ancestry
or affected-code presence. Reading the pinned cjs_global.cpp through the browsing
tool failed, so no exact-source vulnerable-function comparison is recorded.

Local `scripts/build-pdfium-source.sh` sets `pdf_enable_v8 = false` and
`pdf_enable_xfa = false`. Upstream's [build documentation at the fix](https://pdfium.googlesource.com/pdfium.git/+/cce5c5ed801f54cadb5b039d98ed43804994764c#selecting-build-configuration)
describes disabling V8 as removing JavaScript support. Inference: the described
JavaScript/V8 route should be absent from a Fedora artifact actually built with
those settings. This is configuration evidence, not binary inspection or a
whole-engine security finding. Retain actual GN/build receipts before closing
artifact applicability.

The binary distributor's [7881 build workflow](https://github.com/bblanchon/pdfium-binaries/blob/chromium/7881/.github/workflows/build.yml)
exposes an enable_v8 input defaulting false. Caller settings and the exact selected
archives still need verification; a default is insufficient to attest Linux/Windows
release binaries. The upstream bug's direct page requires sign-in in this session;
search-index details were available but do not substitute for artifact evidence.
The November bulletin also lists PDFium CVE-2026-121113 and CVE-2026-121179;
include both in the remaining engine triage instead of clearing the pin after one
V8-specific finding. Their direct issue pages were sign-in-only here.

### Complete lockfile OSV lookup (2026-11-13)

The later [query receipt](planning/progress/T19-osv-lockfile-2026-11-13.json)
supersedes the earlier missing-scanner observation for public package/version
lookup coverage. The official [OSV batch API](https://google.github.io/osv.dev/post-v1-querybatch/)
was queried for all 552 registry entries plus upstream Winit 0.30.13, with ordered
results checked, pagination handled and full records retrieved for matches. The
receipt binds Cargo.lock SHA256, retrieval time, package/version rows, advisory
modification timestamps and raw-record hashes. The live API does not provide a
single advisory-database Git revision; this is a dated service snapshot.

| Package/version | Advisory | Assessment / next action |
| --- | --- | --- |
| pulldown-cmark 0.13.5 | [RUSTSEC-2026-0433](https://rustsec.org/advisories/RUSTSEC-2026-0433.html) | Vulnerable version match; nested wikilink parsing requires ENABLE_WIKILINKS or Options::all. The only locked direct dependent is i-slint-common 1.18.1. Its inspected parser uses ENABLE_STRIKETHROUGH only, so the required option is absent on that call. Qualify a separate update to patched >=0.13.6; do not silently suppress the package match. |
| bincode 2.0.1 | [RUSTSEC-2025-0141](https://rustsec.org/advisories/RUSTSEC-2025-0141.html) | Unmaintained informational record; transitive via typed-index-collections. Review upstream migration, not an automatic encoding replacement. |
| pin-utils 0.1.1 | [RUSTSEC-2026-0441](https://rustsec.org/advisories/RUSTSEC-2026-0441.html) | Unmaintained informational record; transitive via spin_on. |
| ttf-parser 0.25.1 | [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html) | Unmaintained informational record; transitive via owned_ttf_parser. Font parser migration needs geometry/render regression coverage. |
| zvariant 5.15.1 | [RUSTSEC-2026-0370](https://rustsec.org/advisories/RUSTSEC-2026-0370.html) | Unmaintained standalone crate; zbus migration requires portal/native integration review. |
| zbus_names 4.3.5 | [RUSTSEC-2026-0371](https://rustsec.org/advisories/RUSTSEC-2026-0371.html) | Same upstream D-Bus migration work; maintain compatibility with the pinned UI/dialog stack. |
| zvariant_derive 5.15.1 | [RUSTSEC-2026-0372](https://rustsec.org/advisories/RUSTSEC-2026-0372.html) | Informational maintenance finding; migration coupled to zvariant/zbus. |

The Slint observation was verified against the cached crate archive whose SHA256
matches Cargo.lock, and the extracted styled_text.rs bytes match the inspected
cache file. That file's hash is in the receipt. This does not establish all-target
feature reachability, native PDFium security, safety of the downstream Winit patch,
or absence of unpublished vulnerabilities. No dependency/pin changed, no exception
approved and no application binary was qualified by this lookup. Six maintenance
findings are not six confirmed exploitable vulnerabilities.

## Qualified parser patch after the inventory

[PR #14](https://github.com/w3fans/dPDF/pull/14) merged as
1adea6f519160c89934a20d8c183885fa4a03fab. It changes only pulldown-cmark
0.13.5 to patched 0.13.6, with identical dependency requirements/features and MIT
license bytes. The historical OSV row above remains evidence for the old lock;
it is not silently suppressed or applied to the new package version.
[Qualification](validation/pulldown-cmark-0.13.6.md) records verified upstream
archive/source identities, all native Linux/Windows geometry/interop/window
checks, exact inventories and 18 before/after UI comparisons. Final documentation
head f3cb4bf3 passed every job in run 43510850834. Other maintenance findings,
PDFium applicability, real package upgrade/default/data acceptance and owner gates
remain open. This is one scoped patch, not whole-project security clearance.

## Minimum compiler validation candidate

The project declares Rust 1.92. Previously public jobs selected stable only;
those successful builds do not establish the declared minimum. The independent
[T19 compiler receipt](planning/progress/T19-minimum-rust-2026-11-14.md) describes
new exact-version native Linux/MSVC source checks in those same approved public
jobs, with separate targets/reports and unchanged Windows admission/resource
limits. This candidate needs hosted evidence before minimum compatibility is
claimed. A source check does not qualify a release executable or native operator
acceptance on that compiler. No dependency, runtime or app-version pin changes.

## Update procedure

1. Prepare one focused branch with current source identity. Record the old/new
   component revisions, official advisory/fix references, affected-code analysis
   and API/ABI/license impact. Engine or native lifetime changes require experienced
   review. Keep a failing generated regression fixture when one is available.
2. Verify official archive hashes/provenance before extraction. Change all affected
   binding features, engine source manifests, runtime URL/hashes, packaging and
   build docs together. For Winit, compare the new upstream base with the bounded
   receive patch; retain it or remove it only after equivalent upstream behavior
   is demonstrated. Preserve URI parsing, transfer timeout and cancellation tests.
3. Use the approved remote compiler route for this workstation, plus native MSVC
   Windows. Run locked tests, formatting, strict Clippy default/check-windows-print
   and both notice checks. Refresh inventory text/hashes through the existing
   collector when dependencies change; inventory success is not a license audit.
4. Run the full source-bound Linux/Windows harnesses in TESTING, including geometry,
   crop/rotation/UserUnit, text/search, assembly untouched streams, marks/review,
   aliases, passwords, recovery and independent Poppler comparisons. Run UI at
   scales 1/1.5/2 and native Wayland/Windows drop/clipboard/printing as affected.
   Compare startup/navigation/zoom/memory baselines on the same host/fixtures.
5. Qualify the actual packages using the matrix below. Record executable, engine,
   helper and package hashes plus source commit, OS/driver/tool versions and logs.
   Reuse exact-source artifacts through the existing receipt/cache path. Do not
   relabel old builds, add another publisher or overwrite conflicting assets.
6. Reviewer records acceptance and unresolved restrictions. Only the release
   authority integrates and publishes through its existing workflow. Retain prior
   source-bound packages/receipts for rollback; never move a published tag.

## Package acceptance matrix

Every row is required evidence for an update; none is newly passed by this prose.

| Environment | Existing checks | Additional acceptance to record |
| --- | --- | --- |
| Fedora 44/45 installed RPM | Existing COPR source build, application/CUPS and lifecycle harnesses; check-desktop.py | Fresh install, upgrade from previous qualified package, removal; default PDF association and user data preserved; actual PDFium/qpdf versions and notices; physical printer gate separately. |
| Windows portable x64 | check-windows-stage.py, package-preview.ps1 and validate_windows.py | Run from unrelated and Unicode/space directories with runtime overrides removed; exact bundled executable/helper hashes; missing/invalid helper behavior and notice completeness. |
| Windows per-user MSI | packaging/windows/test-msi.ps1 (disposable account only), register.ps1 | Fresh install, major upgrade from prior package, uninstall; Open With and Start Menu; exact pre/post default-app choice; user library/preferences/recovery retained. Fresh-install smoke now compares legacy default and Explorer UserChoice values before/after install and uninstall (candidate change, native execution pending). Optional PreviousMsi/PreviousVersion now exercises the MajorUpgrade path; generated preferences/mark/recovery sentinel hashes are checked through install and uninstall. Native execution and real saved-model reopening remain pending. |
| Windows native desktop | Controlled validate_windows.py desktop lane plus manual scenarios | File Explorer open/drop, mixed DPI, clipboard, print dialog/cancel, real paper size and saved-data reopening. Hosted service success does not qualify this row. |

Windows code signing remains a separate unqualified item; inspect the actual
candidate signature before describing it as signed. No certificate purchase,
credential provisioning or change to machine security settings is authorized by
this procedure. Recovery data remains private and must not enter public logs.
