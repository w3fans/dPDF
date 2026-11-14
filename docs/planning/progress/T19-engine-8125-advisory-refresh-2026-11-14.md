# T19 advisory refresh for qualified engine candidate

Status: package/version lookup complete; maintenance, package/operator and review
work remain open. Applies only to engine head
5456aa4fc6d54f3900bb9d288f3ae1148e1d171d, not the root worktree's preserved
unmerged lifecycle edits. Main remains e7bec799; no code, dependency, tag or version
changed in this substep.

The previous full lookup binds the older lockfile. Reused its official OSV
querybatch/query/vulns flow against all 552 locked registry package/version pairs
plus the vendored Winit upstream version: 553 lookups total. Verified result count and exact package
multiset against this candidate lock (SHA256
61a1714a64054fb6c42f370b53da749aa8c4e453bd1467ed71b043fb72bd8b33).
Response order is guaranteed by the [official API contract](https://google.github.io/osv.dev/post-v1-querybatch/);
result length and pagination were checked. No single OSV backend database revision
is exposed. Retrieval time, record modification times, raw response hashes, query
driver hash and both qualified target-inventory hashes are in
[the complete report](T19-osv-lockfile-8125-2026-11-14.json).

Six matched records are all RustSec INFO Unmaintained, with no patched versions:
bincode 2.0.1, pin-utils 0.1.1, ttf-parser 0.25.1, zvariant 5.15.1, zbus_names
4.3.5 and zvariant_derive 5.15.1. The report links each official RustSec page and
retains full record hashes. Do not suppress these findings or automatically
replace serialization/font/portal components without upstream compatibility,
license and regression review. The existing maintenance table identifies their
parent dependencies and migration concerns.

The qualified native normal/build/proc-macro inventories contain five matched
packages on Linux (all above except bincode) and one on Windows (pin-utils).
Absence from this retained inventory is not proof of all-feature/test or runtime
unreachability. Locked pdfium-render 0.9.5, pulldown-cmark 0.13.6 and Winit 0.30.13
have no version matches in this dated lookup. Vendored Winit's downstream patch,
native PDFium/qpdf/system components, unknown defects and distribution/legal review
remain outside it. The separate native advisory/source dossier remains required.

Raw response/record copies and exact query driver are preserved under ignored
artifacts/t19-pdfium-8125-source/advisory-refresh in the implementation workspace. This action installed no scanner,
ran no compiler, and changed no app/dependency/package/release configuration.

## Current implementation gates

Draft #18 head 5456aa4f passed every job in run 43540973212; final Linux executable
and PDFium bytes equal the pair used for 63 rendering/text/geometry comparisons,
three real-window scales and four JPEG repair display cases. Source-bundle repeat
collection is byte-identical. Final hash/job receipts are retained in
artifacts/t19-pdfium-8125-source/final-head-qualification.json and
source-bundle-reproducibility.json. These records do not close Fedora GN/full C++
build, installed package lifecycle, native operator or experienced engine review.

Fresh connected PR review lookup: drafts #10/#16/#17/#18 remain open/draft at
respectively a87095b9, 948264e3, e3b452e5 and 5456aa4f; every submitted-review list
is empty. T03 dependencies therefore remain pending the required lifecycle review.
T12's SECURE-REGION-CONTRACT has no recorded GO; T13–T18/T21 stay gated. T05/T07/T08
must not be advanced as READY before T03 passes; native/physical/owner gates remain
separate. T01 public portability and T02/T04 implementation exits stay scoped.

Next: experienced lifecycle and engine reviewers provide source-bound decisions;
Fedora operator runs the existing adapter in the supported disposable buildroot;
native operators execute the prepared package/input/printer acceptance. Continue
any newly ready work or review corrections; keep the overall plan uncompleted.
Rollback: retain this historical lookup with its exact identity; later scans
supersede it explicitly rather than rewriting it as current evidence.


## Vendored Winit provenance correction

Verified the recorded 0.30.13 archive checksum and embedded upstream Git revision.
A complete 217-file archive comparison confirms LICENSE and Cargo.toml.orig are
unchanged, and no X11/Windows source changes exist. The normalized Cargo.toml has
two feature additions for the existing Wayland receive implementation; a semantic
TOML comparison proves those are its only manifest differences. Corrected
third_party/winit/DPDF_PATCH.md to name them, three modified integration modules,
two added Wayland implementation files and omitted upstream VCS metadata. See
[the byte-comparison record](T19-winit-patch-provenance-2026-11-14.json).
This documentation correction changes no dependency, code or native qualification.

Separate [minimum-compiler candidate #19](https://github.com/w3fans/dPDF/pull/19)
checks actual declared Rust 1.92 source compatibility. Its in-progress run
43544105201 must be inspected separately; this documentation-only candidate does
not include that checker or establish a minimum-compiler PASS.
