# T19 — Dependency inventory and update procedure

Status: IN_PROGRESS. T02 dependency passed. Inventory/procedure substep complete;
advisory coverage and exact-source package qualification are incomplete.
Baseline: `268b234e4336789922a6b15dd1236963e131d707`.
Branch: `fix/public-release-adapter-tests`; no pins, lockfiles, versions or tags changed.

Inspected Cargo.toml/lock, vendored Winit provenance, Fedora engine source manifest,
CI runtime hashes, installer WiX pin, third-party notices and native package test
scripts. Added DEPENDENCY-MAINTENANCE with ownership roles, baseline/compatibility
couplings, official dated advisory references, update/regression procedure and
Fedora/Windows package acceptance matrix. Corrected Windows notice inventory
count in THIRD_PARTY from stale 254 to observed 259.

Fresh 2026-11-13 official-source browsing: Google November 3 Chrome bulletin lists
CVE-2026-121166 in PDFium. Configured runtime release identifies 151.0.7881.0;
exact pinned-revision applicability/backport status is unresolved, not a confirmed
vulnerability in dPDF. qpdf official release listing matches bundled 12.4.2.
RustSec image advisories 2020-0073 and 2019-0014 have patched ranges below configured
0.25.10. No whole-lockfile advisory clearance claimed. Winit/Slint security pages
were inspected, but the RustSec Winit package page was unavailable; retain this
coverage gap. Sources and scope are linked in DEPENDENCY-MAINTENANCE.

Checks:

- `PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-cargo-notices.py --target linux`:
  PASS, 348 packages / 646 file references, inventory only.
- Same command `--target windows`: PASS, 259 packages / 490 references, inventory only.
- `python3 scripts/release-version.py verify`: PASS v0.1.54.
- `git diff --check`: PASS.
- Local Markdown file-target check for the three changed docs: PASS.
- Rust build/native package tests: NOT RUN; approved remote transport remains
  unavailable per T04 receipt. No current candidate binary/package is qualified.

Next: inspect upstream bug/fix for CVE-2026-121166 and map to engine revision
91b9d569b34be4f38eed7b3c49b227356c3aadad; if affected, propose a separately reviewed
binding/runtime upgrade with full regression gates. Run a target-scoped advisory
scan on approved infrastructure and record database revision/exceptions. Native
operators must record fresh install/upgrade/uninstall, Unicode paths, exact default
association preservation and data retention on actual candidate packages.

Existing MSI fresh-install smoke does not prove major upgrade or user-data retention;
those remain pending explicitly. No code-signing claim or provisioning action.
Rollback: revert focused maintenance docs and stale-count correction; retain pins,
user data, candidate work and all historical/published receipts.

## Advisory mapping and MSI test repair — 2026-11-13

Upstream fix cce5c5ed801f54cadb5b039d98ed43804994764c references issue
566111382 and changes JavaScript global enumeration. Recorded commit/date and
configuration evidence in DEPENDENCY-MAINTENANCE. Fedora build explicitly disables
V8/XFA, narrowing this specific route; binary archive feature/ancestry verification
remains pending. Pinned file retrieval failed; direct issue pages require sign-in.
Added the bulletin's other PDFium findings 121113/121179 to remaining triage.
No engine version changed or whole-engine security clearance claimed.

Reproduced test-logic gap by source inspection: the old assertion was only
`default -ne net.dpdf.dPDF`. Both before=OtherReaderA and after=OtherReaderB
satisfy that predicate, despite violating preservation. This is a test coverage
defect, not evidence that the actual installer changes defaults.

Changed packaging/windows/test-msi.ps1 to snapshot legacy .pdf default and
Explorer UserChoice values (including presence and hash) in memory before install,
then compare exact serialized state after install and uninstall. Empty-key creation
for Open With is allowed; value absence versus an empty value remains distinct.
No protected association is written, and UserChoice data is not printed.
Existing disposable-account guards remain. Major upgrade and user-data retention
are still separate pending gates.

Checks: git diff --check PASS; release-version.py verify PASS v0.1.54.
Neither pwsh nor powershell is installed here. PowerShell parser/native MSI
execution NOT RUN; this candidate test change needs the controlled Windows lane.
No local or remote compiler/build job was launched.

Next exact checks: native Windows parses the modified script and runs existing
installer qualification with a preexisting non-dPDF default plus a no-default
account. Add/retain major-upgrade and user-data retention acceptance separately.
Continue artifact-bound V8 and other PDFium advisory mapping on approved infra.

## Major-upgrade and user-data fixture preparation — 2026-11-13

Extended existing MSI smoke with paired optional PreviousMsi/PreviousVersion.
It verifies a strictly older installed executable then upgrades through the
existing WiX MajorUpgrade path; no independent installer or publisher added.
Candidate/previous MSI hashes identify the actual test inputs.

Added generated preference bytes under APPDATA/dPDF and mark/recovery retention
sentinels under LOCALAPPDATA/dPDF, matching source path resolution. Existing data
roots cause refusal before seeding. Exact hashes are compared after prior install
(if selected), candidate install and candidate uninstall. Success-only fixture
cleanup uses roots absent at entry; failure retains state/logs. Sentinel files
are not valid mark/recovery schemas and do not qualify reopening user drafts.
Documented fresh versus upgrade invocation and diagnostic cleanup constraints.

Checks: git diff --check and metadata verify PASS. Source-only review confirms
existing WiX UpgradeCode and MajorUpgrade configuration; no actual upgrade,
PowerShell parser or MSI execution has run here (no Windows/PowerShell host).
T19 stays IN_PROGRESS; these are prepared regression checks, not acceptance.
Next: controlled Windows lane executes fresh/upgrade cases in separate disposable
accounts and records source/package identity, then native corpora reopen valid
mark/recovery models. Continue remaining advisory artifact mapping separately.

Advisory-scan prerequisite check: neither cargo-audit nor cargo-deny was found on
PATH, and no advisory database/cache or current target directory was found at
the inspected standard locations. No scanner was installed or compiled. Full
lockfile advisory coverage remains pending approved scanner/database access.

## Full locked-package lookup — 2026-11-13

Approved network execution allowed official OSV API lookup without installing or
compiling a scanner. Queried 552 registry lock entries plus vendored Winit's
upstream version (553 total), checked response completeness and handled pagination.
Saved durable package/version results and matched-record metadata in
T19-osv-lockfile-2026-11-13.json, bound to lock SHA and retrieval time. Generator
script and raw responses are scratch /tmp/dpdf-osv-query.py and
/tmp/dpdf-osv-20261113/. No single OSV database revision is exposed by this API.

Seven package matches: six informational unmaintained records; one vulnerability
record for pulldown-cmark 0.13.5. Verified i-slint-common 1.18.1 archive checksum
against Cargo.lock, then extracted styled_text.rs matches inspected cache bytes.
Its parser enables only strikethrough, not the vulnerability's required wikilinks
option. This scoped source observation does not suppress the version match.
Patched >=0.13.6 update remains separate work with notices/regression qualification.
Maintenance migrations need upstream compatibility review rather than broad
unqualified dependency upgrades. Native engines/system libraries and target feature
reachability remain outside this lookup. No dependency or version changed.

Checks: complete 553 query result rows, seven fetched advisory records, matching
Slint crate/file provenance. Metadata verify and diff whitespace checks PASS.
T19 remains IN_PROGRESS for package execution, remaining native advisories and
reviewed update decisions; do not label this a release-wide security PASS.

## Focused package-check PR — 2026-11-13

Opened draft https://github.com/w3fans/dPDF/pull/7 on
test/msi-upgrade-data-retention, candidate
bc0ebdd6e6079c53ea033679d2bedebf409979d2. Four files only: MSI smoke, isolated
TESTING section, Unreleased entry and identical baseline strict-Clippy repair.
MSI readback blob matches local 1e2b4fb8f23e8f5319c44317737b85d5ae0d98f4.
Public Windows CI parses this script; it does not execute MSI lifecycle acceptance.
No user-data fixture, installation or default-association mutation ran locally.
Other documentation/advisory work remains preserved in the local candidate.

## Source-bound Linux test artifacts — 2026-11-13

PR #7 candidate 24434f7dffc2ff7404fa407bf0d97bb69d621ce3 adds a post-validation
Linux staging step using existing stage-linux.sh. It retains tested executable
and pinned PDFium bytes with notices, checks equality to the tested originals,
writes checked-out commit/tree and SHA256SUMS, and copies the validation report
into the existing run artifact. System qpdf is not bundled; this is a test runtime
artifact, not new release publication, installed RPM or manual qualification.

Local test_ci.py: 12 cases, eight passed/four existing skips; YAML parses; diff
whitespace check PASS. New CI run 43469565164 is confirmed pending. Earlier PR #7
Windows hosted lane passed; the new source requires its own receipt. The Linux
staging artifact does not exist until that new validation/staging/upload succeeds.
Use the exact artifact commit/tree and hashes before any local native fixture
test, and do not pair it with the newer uncommitted combined worktree blindly.

## Generated CUPS fixture failure repaired — 2026-11-13

Run 43469565164: Windows hosted/policy PASS; Linux FAIL before staging, in
platform::tests::cups_process_receives_literal_ranges_printer_and_pdf_bytes.
Job132037733205 log confirms release tests73passed/1failed: mock exec returned
Text file busy (errno26). No staged runtime from that failed run is qualified.
Raw log obtained through the existing GitHub job-log endpoint; no live job retried.

Reproduced writable-script ETXTBSY deterministically locally using generated data,
then verified /bin/sh script invocation preserves literal printer/range arguments
while the writer remains open. Kernel FD inheritance is a plausible explanation
for the original intermittent failure, not a traced root-cause proof.

Added a command-injection boundary around existing CUPS argument/submission logic:
production still creates Command::new(lp) and submits once. The unit fixture uses
/bin/sh with the generated script path; it deliberately holds the script writable,
asserts direct execution fails26, then checks arguments/PDF bytes/status via the
interpreter. No production spawn/submission retries or changed print options.

PR #7 now422ee434c13f6ba9f3ffd502bd6a1e7dd42fcff1 incorporates merged T01 main
through a normal merge parent, preserves package checks/artifact retention and
retains both changelog entries. Readback platform/CI/MSI blobs match local files.
Local OS fixture check, cargo fmt --check and git diff --check PASS. New Rust
unit/full corpus and staging execution are pending; no pipeline failure hidden.

GitHub PR readback confirms current head422ee434c13f6ba9f3ffd502bd6a1e7dd42fcff1/base1cf522a and mergeable. No workflow run was yet returned for that new head at two readbacks; do not call its validation running or restart a prior run. Recheck this exact head for run discovery next session.

## Workflow semantic regression corrected — 2026-11-13

No run existed for422ee434c13f6ba9f3ffd502bd6a1e7dd42fcff1. Inspection found
local reconciliation had removed the Summary step header, leaving duplicate run
keys under the staging step. This was an implementation error; ordinary PyYAML
parsing accepted duplicates and was insufficient validation. Restored the header.

Downloaded official rhysd/actionlint v1.7.12 to /tmp only; verified release archive
SHA256 8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8
against GitHub release metadata. Initial license extraction used the wrong member
name; corrected it to LICENSE.txt. Tool/binary integrity was verified before use.
Actionlint accepts the corrected workflow and rejects /tmp/dpdf-ci-duplicate-run.yml
with duplicate run at line166. No app dependency or system tool installation.

Added a narrowly scoped standard-library guard to test_ci.py for duplicate direct
step keys in this repository's CI indentation, plus a valid/broken boundary test.
It complements actionlint and is explicitly not a general YAML parser. Local CI
contracts now14cases:10passed/4existing skips. Formatting, metadata, diff checks PASS.

Pushed normal follow-up3832a2c5b6f4f4e572b49fc0dde86a0176385e33 to PR #7.
Workflow/contract source blob readback matches local files. Native fixture/MSI
lifecycle and staged runtime acceptance still require the new run; no failed or
unstarted candidate is labelled passed.

## Successful runtime retention and toolbar reconciliation — 2026-11-13

PR #7 head3832a2c5b6f4f4e572b49fc0dde86a0176385e33 passed all jobs in
run43474324460, including corrected CUPS release tests and staging. Linux report
binds actual tested refs/pull/7/merge commitd8c1eb04747c73cfb55bc0958234849def01a355,
tree9eca9d90394e4e6096fbf3568da1d2a4d10c8658. Runtime executable SHA256
4f9e4d06dd35ae8a00fcac18af6886ec66fbc805dd6ff05c008bd27f0dddd71c,
PDFium library SHA256f728930966f503652b92acc89b9374a2eeca00ce42e26dccd3e4b5c5161b2d64.
Downloaded artifact14062109241 to /tmp/dpdf-t19-linux-runtime.zip, verified archive
SHA25647d2f0c8e83009c2ad80852f55d50cb1104b0fc05d231d64510e7ebcece3a49e,
safely extracted /tmp/dpdf-t19-runtime and matched executable to PASS report.
Source readback confirms that artifact predates T04 toolbar changes. A filename
inspection initially requested SHA256SUMS; actual staged filename is SHA256SUMS.txt.
No missing binary/hash or current-toolbar acceptance inferred from that lookup.

Reconciled merged T04 main via normal merge candidate
bb33f9582c703dde5130070252e48f4ee7dfbb7a/tree1cafe1b0ec81cf20b65255b559c8258a1dbec9a5.
Run43477133988 is confirmed live. Retain prior artifact with its own source;
new combined-source runtime/qualification must come from this newer run.
Actual MSI install/upgrade/uninstall, real saved-model reopen and platform owner
acceptance remain PENDING. No release or version change.

Staged SHA256SUMS.txt was subsequently checked successfully with sha256sum -c:
executable and PDFium both OK. Archive extraction removes executable mode, so a
future local runtime launch must restore only the verified fixture executable's
mode first. The source tree remains separate from newer uncommitted work.

Final combined-source run43477133988 completed policy/full Linux PDF/UI and
Windows hosted jobsSUCCESS forbb33f9582c703dde5130070252e48f4ee7dfbb7a.
Marked ready and squash-merged PR #7 as87560c34b2d05c956d193cccb8a8c26eeec27d3a.
Native MSI lifecycle/manual acceptance remains open; merge is routine development,
not a package acceptance or release. Local branch/index advanced to87560c3 after
fetch, with all50 previous edited/untracked file hashes preserved. Saved patch
and identity at /tmp/dpdf-before-main87560.patch and .json; no source overwritten.

## Bounded build-memory baseline and controlled experiment — 2026-11-13

Recurrent Windows LLVM OOM is an actual build-maintenance failure, not a reason
to relax the1CPU/2GiB bound. Created isolated public worktree /tmp/dpdf-codegen-metrics
(branch work/codegen-metrics) at main30fafd7; async PR #10 and user edits untouched.
Added QueryInformationJobObject telemetry before job close, explicit unavailable
query state (never fake0), and cleanup-preserving telemetry failure behavior.
Two source-safe tests cover measured peak/cap mapping and unavailable queries.
Local CI contracts16cases:12pass/4existing native skips; format/diff/noticesPASS.

PR #11 https://github.com/w3fans/dPDF/pull/11 baselinefe081869f89b66035f6a44dbe20bdcb0a8aee140,
tree2e2dece6f195834d2cc86e3221c129deac775c17, explicitly sets16application codegen
units (Cargo release default) and changes no dependency profile or resource cap.
All five source readback hashes matched local. Run43489953584 completed policy,
full Linux and native Windows jobsSUCCESS. Downloaded Windows artifact14070285357,
SHA256b0ec0a06893007a6a9f09ce8bfa692a2c7c66ac176dc0d59ecad17d93c287959.
Native compiler-artifact fresh=false; optimization1. Build aggregate committed
peak2,125,107,200bytes versus cap2,147,483,648; headroom22,376,448bytes (~21MiB).
Native rustc1.99.1/LLVM23.1.1, cargo1.99.1. This is Windows commit charge, not RSS.
Receipt T19-build-memory-baseline.json. No deterministic memory fix claimed.

Verified baseline Linux artifact14070170281 archive
acfc37fcbcc6bd9b8ff5b17b1dae928de485d30096394a2a4dbbaed0568269cd,
actual merge sourced8d0f622c6ec648751a41965ed58f430b4c16500, binary SHA256
232b1d03f496cda5de1c5ad2cae0d55526c04b025067a5c584dbfee16f07315b.
Before changing codegen count ran benchmark-reader.py --repeat5 on this verified
runtime/Fedora45:50-office/1000-text/50-vector first-page medians17.174/31.754/
22.696ms; cache45,459,264bytes, process peaks roughly91–99MiB. All32geometry
cases passed. Raw T19-codegen-baseline-reader.json; generated/headless only,
not GUI latency, real-office SLA or native Windows runtime performance.

Next experiment prepares256application codegen units at the same optimization,
dependency pins/profiles, concurrency and resource caps. Proposed engineering
build-commit target <=1.75GiB, before/after same-machine reader/output checks,
and investigation of >10%median regression. Await exact native measurement;
if it worsens memory or fails gates, do not describe it as a fix. Baseline and
failed receipts remain intact. APIs verified against official Cargo profile and
Microsoft QueryInformationJobObject documentation, linked in build instructions.
