# Controlled build infrastructure

Private-repository compilation uses the existing build services. GitHub remains
source control, PR coordination, lightweight policy/receipt jobs and release
publication. Public repositories retain useful independent hosted validation.

| Work | Previous duplicate hosted path | Private-repository path |
|---|---|---|
| Linux compile and PDF regressions | Ubuntu on PR/main | the release host controlled adapter to the Linux build service |
| Windows native PR/main/tag checks | Hosted Windows plus the service lane | Controlled native Windows harness |
| Windows desktop acceptance | Separate hosted desktop VM | Controlled native Windows desktop acceptance; otherwise PENDING |
| Windows installer compilation | Hosted Windows on tag/manual | Controlled Windows packaging after native qualification; otherwise PENDING |
| Fedora package submit/wait | Up to 180 minutes on hosted Ubuntu | Existing controlled local COPR publisher |
| Source policy regressions | Duplicate Ubuntu PR/main job | Existing local release preflight; actual PR policy before merge |
| Release page | Lightweight Ubuntu publisher | Existing authenticated local publisher and user timer |
| Windows ref resolution and queued receipts | Ubuntu even when every worker was skipped | Skipped with private Windows workers; retained for public validation |

## Public repository publication

The public repository uses `release.yml` for release notes and the Windows
installer workflow for native preview packaging and qualified asset uploads.
The competing local release-page timer is disabled after successful public
Actions readback. Its existing helper remains available for reviewed recovery;
select one page publisher before using it.

Fedora 44/45 source builds continue through the existing local COPR operator,
with persistent build IDs and exact-source attachment receipts. The hosted COPR
submitter remains off, preventing duplicate builds and credential transfers.
Public hosted Linux and Windows validation is independent of controlled desktop
and physical-printer acceptance. Skipped jobs are never passing test evidence.

The v0.1.54 delivery record binds these public roles to source
`e06b05b5f1e91e9e9e99b97967842bad4996bc67`; see
[RELEASE-STATUS.md](RELEASE-STATUS.md). It is not a new check of this branch.

The local-recovery instructions below describe the private/fallback lane;
they do not require disabling the selected public release workflow.

## Preserved build and release controls

The release host remains the source/release authority. Its installed Cargo wrapper delegates
compilation to the Linux build service through the existing filtered remote build client. Preserve
one shared slot, one CPU and the 2GiB maximum; do not build Rust locally on the development host
or move credentials into workflows. Keep existing toolchains, dependency pins,
release locks and the controller's version/build/validation gates.

The existing `scripts/publish-copr.py` remains the package publisher. Invoke it
from the controlled publication host with the exact released tag; existing
COPR package qualification remains separate from the local reader build.
Private GitHub jobs no longer hold an Ubuntu runner while those package builds
run. Local COPR configuration and GitHub attachment authority must be available
before claiming package publication; preserve prior receipts and package hashes.
Do not introduce a replacement cloud service.

The dedicated Windows target is the authorized private Windows acceptance machine. Use the existing
standalone `scripts/ci/validate_windows.py` harness with a reviewed immutable
source checkout and record its exact commit/ref and artifact/test receipt.
Private GitHub workflows do not dispatch compiler or privileged self-hosted
workers. This avoids executing caller-selected PR code or harnesses on the dedicated Windows target.
Do not enroll an unrestricted privileged runner as a shortcut.

No Windows service or machine security setting is changed. Existing public
runner/enrollment configurations remain intact. An unavailable Windows target harness or
its prerequisites is PENDING, not permission to use a paid Windows VM. Local
GitHub receipt publication needs its existing scoped publishing authority;
keep credentials outside source and do not claim delivery before verification.

## Trusted source and artifact reuse

Private PRs never dispatch application code to the self-hosted Windows machine.
The resolver accepts an exact commit only when it is reachable from approved
main history, including when resolution starts from a shallow checkout. It
captures the requested/event SHA before refreshing main, so a later main commit
does not silently replace the requested source. Unmerged SHAs produce no worker
outputs. Keep the separate read-only worker and scoped metadata publisher.

Reuse an existing artifact only with its original exact commit/ref, target,
toolchain, binary/package hash and successful acceptance receipt. The existing
Windows harness records these identities and checks source cleanliness. Package
and upload the qualified artifact instead of rebuilding it independently for
PR, main, tag and installer. Do not relabel an older binary as a newer release.
Skipped hosted compilation is not a local PASS. Preserve missing/failed receipt
states and report unavailable Windows targets or publication setup as PENDING.

## Private/fallback hosted usage and setup

The existing release preflight already runs source-policy/contract tests before
version preparation; it now also runs the four Git trust tests. The duplicate
private hosted policy-regression job is guarded off. Actual PR policy is checked
against the reviewed base/head before integration.

Private hosted Linux/Windows compilation, hosted desktop fallback, installer
compilation and the long COPR submit/wait job are guarded off. Windows ref-resolution
and queued-receipt jobs share the private-repository guard with their workers;
they do not allocate Ubuntu runners for a validation lane that cannot execute.
In the private/fallback lane the release-page publisher runs locally and its
hosted workflow is disabled after local readback. The active public repository
uses the public publisher selected above; do not activate the competing timer.
Public hosted validation remains available. Normal qualified patch releases continue; stable/platform acceptance
retains its additional gates.

Required local setup is the established release-to-build-service adapter, local publisher access
and the dedicated Windows target's existing documented Windows prerequisites for the controlled harness.
Do not copy build-host secrets into workflow code or run untrusted PR code on
privileged machines. See WINDOWS-CI.md for the existing Windows setup procedure.

## Artifact reuse

Native build receipts bind the full source commit, native compiler/Cargo versions,
release profile, effective flags/global Cargo configuration and binary SHA256.
Cargo's compiler-artifact manifest identifies the actual output; guessed old
target files are not accepted. Immutable copies survive later target-directory
builds. Tampered bytes, changed identity or unexpected paths fail closed.

Validation and installer packaging share this cache under `DPDF_CI_ROOT`.
An operator may configure `DPDF_NATIVE_ARTIFACT_ROOT` for an approved artifact-only
handoff between isolated profiles; do not share runner credentials or another
project's writable Cargo cache. A build receipt proves build identity, not native
acceptance: each applicable test lane still runs and records its result.

COPR's Fedora 44/45 source builds remain necessary independent distribution
targets. A native executable from a different toolchain is not inserted into an
SRPM to avoid those builds. Submission/monitoring runs once per tag with persistent
receipts; retries resume it and verify each distribution's binary/source pair.

## Local COPR operator

`scripts/ci/local_copr.py` uses an operator's existing Git SSH identity and COPR
configuration. Private paths are supplied in an untracked environment file; no
token is printed, exported to a workflow, or copied to another node. The worker
fetches existing tags, requires clean checkouts in main history, checks the
private publication denylist and uses the existing source assembler/publisher.
It never allocates versions, commits, creates/moves tags or pushes Git refs.

The baseline is an existing qualified release. Historical tags below it are
never backfilled. `--verify-tag` verifies an existing publication without submitting
anything; `--dry-run` verifies the baseline and lists future queued work. Completed
receipts prevent duplicate submissions; changed receipts are reverified.

`ops/systemd/dpdf-copr-publication.service` and `.timer` are user-unit templates.
The operator installs the standalone worker and supplies its existing repository,
worktree/output roots, qualified baseline, privacy patterns and verified engine/
Cargo caches in `~/.config/dpdf/publication.env`. The worker is capped at half a
CPU, 1 GiB soft memory, 1536 MiB maximum and 128 MiB swap, independently of the
unchanged remote compiler limits. Its lock serializes publications.

The hosted COPR relay is disabled for private repositories. Public use requires
explicit `DPDF_HOSTED_COPR=true`; retire the local submitter before enabling it.
There must be one submitter per tag. Ordinary source/release publication continues
while Windows acceptance or unrelated infrastructure is pending.

## Release-page recovery

A pushed tag and an uploaded COPR package do not create a GitHub Release page.
The installed `scripts/ci/release_page.py` publishes missing pages from
existing reviewed tags without builds, version allocation or tag changes.

The protected local configuration supplies a separate bare Git index as
`DPDF_RELEASE_PAGE_REPOSITORY`; this avoids concurrent fetches in the compiler
publisher's checkout. The `dpdf-release-pages` user-unit templates use the same
protected path-only environment file and the operator's existing GitHub CLI API
authentication. No credential is extracted, transferred or committed. Install
the reviewed helper and its `check_pr_policy.py` dependency together.

The default mode is an offline plan. `--apply` reports `PENDING_API_AUTH` when the
existing API identity is unavailable; it does not claim a page was created.
Authorized execution verifies the remote tag's exact commit before and after
creation, keeps preview flags, checks page-body readback and preserves already
verified receipts across offline runs. `--recheck` requests fresh readback for
completed entries. Changed existing pages are never overwritten. Concurrent or
ambiguous creation is reconciled by reading the unique page for that tag.

Historical notes may use main's explicit privacy-only redactions; changes to
release facts are rejected. The private denylist is checked before publication.
This does not sanitize old source archives or authorize a visibility switch.
RPM attachment remains a separate verified stage. The recovery unit invokes the
maintained publisher with `--attach-only --source EXACT_TAG_CHECKOUT`, the existing
per-tag output directory and recorded build ID. This mode requires successful
COPR/readback receipts, both targets, matching RPM/SRPM source revisions and the
existing checksum manifest. It never assembles source, submits a build or downloads
replacement packages. For an explicit recovery of the current preview:

```sh
python3 /path/to/maintained/scripts/publish-copr.py --tag v0.1.49 \
  --source /path/to/clean-v0.1.49-checkout \
  --output /path/to/existing-v0.1.49/copr --build-id 11270028 --attach-only
```

Uploads never clobber existing assets. All matching names are checked before any
write; identical server hashes/sizes are retained and conflicts fail for review.
Each new or ambiguously completed upload requires server SHA256/size readback,
followed by a final exact-tag check. `github-assets.json` records only verified
results. Receipt/checksum stamps avoid repeating large local package inspection
on every timer tick; `--recheck` forces fresh verification. Missing package
receipts remain pending, and a failed historical entry does not prevent later
independent pages from progressing. Never discard receipts or allocate another
version to recover metadata.

Add `--dry-run` to attachment-only recovery to verify actual package/source inputs
and write `github-assets.plan.json` without GitHub access. Its `READY_NO_UPLOAD`
status is a preparation result, not a published or remotely verified asset receipt.

When `RELEASE-DELIVERY-CHECKPOINT.json` exists under the configured output root,
the worker synchronizes its page/Fedora-asset milestones from source-bound verified
receipts. It preserves the selected tag/SHA even if newer development tags appear.
Missing, pending or mismatched receipts cannot become published milestones. Native
Windows publication and the final live verification remain separate gates; the
worker records readiness for final verification rather than completing the Goal.
Checkpoint replacement is atomic and protected by its own lock. Install the
reviewed `delivery_checkpoint.py` beside `release_page.py` when updating the unit.
Failures in historical entries still allow later entries to run, and the CLI
returns a failing exit status so systemd cannot report those attempts as success.

Install the reviewed `publish-copr.py` and `file_digest.py` alongside the recovery
helper. Its existing protected configuration supplies publication/worktree roots;
no new credential is introduced. Attachment inspection uses a private disk-backed
temporary cache under the unchanged quarter-core/256 MiB hard limit. Windows
MSI/portable assets still require their separate approved native build, acceptance
and packaging receipts; Fedora publication does not qualify Windows artifacts.

The source assembler needs page-cache headroom while vendoring dependencies. A
512 MiB soft threshold caused sustained memory-high throttling during the first
real automatic publication, without an OOM or hard-limit breach. The 1 GiB soft
threshold retains the 1536 MiB hard cap and all other resource limits.

Source assembly uses the user unit's private disk-backed cache directory for
`TMPDIR`, rather than a possibly RAM-backed `/tmp`. Vendored sources and native
archives are large; placing them in tmpfs charges their full size to the worker
and prevents ordinary page-cache reclaim at the soft threshold. Preserve the
1 GiB soft / 1536 MiB hard memory limits and half-core CPU cap.

An apply-mode page recovery blocked by missing API authorization exits nonzero after
recording pending plans for all selected tags. A successful timer exit therefore
cannot be inferred from a pending-authentication no-op. Dry runs remain successful
preflight observations. Existing verified receipts are preserved.

## Private/fallback local delivery

After the standard controller pushes its immutable preview tag, that tag is the
persistent input to the existing local COPR and release-page timers. On the
publication operator, the adapter also wakes the same release-page service.
The release authority needs its existing Git SSH access; API authorization stays
in the publication account's GitHub CLI. No hosted job is required for this fallback lane. The active public publication
roles are recorded above and in RELEASE-STATUS; select one publisher per component.
A pushed tag reports `TAGGED` and `PUBLICATION_QUEUED`, never completed delivery.

```sh
./release.sh delivery-status
./release.sh deliver --tag v0.1.53
```

These commands run in the configured local publication account. Status is
read-only and reports exact-source receipt evidence separately for the page,
Fedora targets, GitHub attachments and Windows. `deliver` validates an existing
tag and notes against the private denylist, then starts or joins the existing
service. It does not build, allocate versions, move tags or submit COPR jobs.
The service reconciles all pending reviewed tags using its existing lock.
Windows remains `PENDING_WINDOWS_ARTIFACTS` until native qualification and an
exact-source publication receipt are available; it never delays page publication.

The page service has a 45-minute total bound and each existing-build attachment
has a 15-minute bound. CPU and memory caps remain unchanged. This permits large
source RPM uploads without the former five-minute termination. Timer retries
retain successful receipts; `--recheck` requests fresh remote readback. A failed
historical tag does not prevent later valid releases. A conflicting uploaded
asset is preserved for review. An abandoned `starter` upload is not a published
asset: inspect its ID/state/time and record operator review before removing it;
never delete a completed asset to make recovery pass.

Normal patch releases remain independent of stable milestone acceptance. Missing
API authorization is `PENDING_API_AUTH`; missing packages are `PENDING_COPR`;
missing native Windows outputs are `PENDING_WINDOWS_ARTIFACTS`. Full delivery
must not be inferred from a successful service exit or source tag alone.

## Bounded Windows compiler memory measurements

Repeated native application builds have hit LLVM out-of-memory under the existing
2 GiB Job Object cap (for example runs 43476228011 and 43485260680). Other builds
passed; an isolated later pass does not establish a deterministic memory fix.
Keep the one-logical-CPU-equivalent and aggregate 2 GiB limits unchanged.

The native job wrapper now queries `JobObjectExtendedLimitInformation` before
closing each owned job and retains `<suite>.log.memory.json` beside its log.
`peak_job_memory_bytes` is aggregate Windows committed memory, **not peak RSS**.
A failed query records `status: unavailable` and an error code, never a fictitious
zero-memory result. These files inherit the enclosing report's source/toolchain
and command identity; they do not independently qualify a binary.

The explicit 16-unit baseline in run 43489953584 (source fe081869) compiled
fresh and peaked at 2,125,107,200 committed bytes against the 2,147,483,648-byte
cap, leaving about 21 MiB. The 256-unit candidate (source bd71a8f5,
run 43492468459) failed in application LLVM code generation with out-of-memory;
its job recorded 2,153,193,472 committed bytes. Retain this failed measurement,
including the original logs; it supplies no usable application binary.

The single-unit candidate (source 21df7bdf, run 43493985255) also failed in LLVM
with out-of-memory, recording 2,147,762,176 committed bytes. Avoiding local
ThinLTO with one unit did not establish a memory improvement.

The level-0 candidate (source 8849548f, run 43495457705) passed all CI jobs,
but its fresh application compile peaked at 2,137,460,736 committed bytes, leaving
only 10,022,912 bytes below the cap. Lower optimization did not meet the target
and is not accepted as a memory fix.

The next candidate restores application optimization level 1 and the default
codegen partition count. Generated Slint types move unchanged into the internal
`dpdf-ui` path crate, allowing UI and application compilation to finish separately.
Its release optimization and dev/test debug levels match their previous settings
inside the application. There is no new external crate version, license selection,
engine ownership or runtime worker. The internal crate retains the project's
existing LICENSE and NOTICE; native notice readback must confirm the inventories.

The first separated build (source 87fadf2b, run 43497797523) passed every CI
job. Both UI and application compiled fresh at optimization level 1. The native
Windows build peaked at 1,779,228,672 committed bytes, below the proposed
1.75-GiB target, leaving 368,254,976 bytes (~351 MiB) under the unchanged cap.
This single measured run is not a universal bound. Same-machine generated reader
medians (five repeats per fixture) showed no >10% median first-page/render/RSS
regression on the 50-page office, 1000-page text and 50-page vector fixtures;
32 geometry cases and full Linux/Windows PDF regressions passed. GUI latency and
representative-office/native acceptance remain separate performance work.

Native Windows inventory readback matched all prior external components but
found CRLF conversions in the new internal crate's LICENSE/NOTICE. The final
candidate preserves those copies verbatim through Git attributes and requires
fresh Linux target collection to equal the retained Linux inventory as well.
Windows native readback must be compared again after that correction. No license
selection changes; the unsuccessful original measurements remain retained.

The final combined candidate also includes the merged focused-text fix. Repeat
its full gates, check fresh UI/application identities and native job peaks, and
compare exact native notice inventories before merge. Cargo concurrency,
dependency optimization and the CPU/memory caps remain unchanged.

References: [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html#codegen-units)
and [QueryInformationJobObject](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-queryinformationjobobject).
