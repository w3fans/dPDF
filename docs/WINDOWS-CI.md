# Unattended Windows validation

For private repositories, native Windows acceptance uses the existing controlled
the dedicated Windows target harness. GitHub compilation and self-hosted worker dispatch are disabled;
missing the dedicated Windows target prerequisites or acceptance remain PENDING. There is no paid hosted
Windows fallback and no new privileged runner enrollment.

Public repositories retain their independent hosted and existing service lanes.
The separate desktop workflow remains manual. Its harness is pinned to approved
main rather than a caller-selected branch. Source resolution requires the exact
commit to be in approved main history.

See [build infrastructure](BUILD-INFRASTRUCTURE.md) for old/new locations,
artifact reuse, local publication setup and remaining lightweight GitHub jobs.

## Existing public/service setup reference

In an **elevated Windows PowerShell** on Windows test machine, from an up-to-date dPDF checkout:

```powershell
.\scripts\ci\install-windows-runner.ps1
```

Prerequisites: existing machine-wide Git for Windows, Visual Studio x64 C++ build
workload and Windows SDK, outbound HTTPS, enough disk space for a separate Rust
cache, and at least 3 GiB free RAM before starting a job. The script checks the
existing C++ installation; it does not install/alter Visual Studio or dGK.

The script creates `C:\actions-runner-dpdf`, installs checksum-pinned runner,
embedded Python and rustup bootstrap downloads, then installs a **separate native
MSVC Rust toolchain** with rustfmt/Clippy. Its private `CARGO_HOME`, `RUSTUP_HOME`,
PATH and build cache live under that directory. Rust's installed stable version
is recorded in each report; the bootstrap downloads are pinned, not the moving
stable toolchain. The installer restores the invoking shell's Rust environment even on failure.
No `rustup default`, global PATH/registry changes, mounts, or
changes to `D:\zDev\dPDF` / dGK are made.

When an existing local GitHub CLI login has repository administration rights,
the installer requests a short-lived runner token through that login. Otherwise,
at the hidden prompt, paste the **short-lived runner registration token** from
[dPDF runner setup](https://github.com/w3fans/dPDF/settings/actions/runners/new).
Repository administration permission is needed to obtain it. Do not paste a PAT
or any token into chat. Registration uses an environment input, not a command-line
argument. The script refuses an existing nonempty runner directory and never
replaces another runner automatically.

The runner name is `dPDF-Windows`; its service account is **NETWORK SERVICE**, not the
interactive user or LocalSystem. Its root ACL grants access only to that account,
SYSTEM and Administrators. The runner has its own `_work` directory. The account
must not have access to real signature libraries or private business documents.
Only trusted repository pushes/tags can start the service workflow; no pull-request
workflow runs application code on Windows test machine. A self-hosted runner executes trusted project
code and is not a sandbox for hostile contributors.

Confirm the runner is online in repository Settings → Actions → Runners. Run
**Native Windows validation** with `release_ref=main` and `lane=self-hosted` once to validate
setup. Subsequent release tags require no local action. The runner's `.env` is
private installation state and must not be committed.

## What each lane checks

The harness checks the resolved 40-character commit against the clean source
checkout and records the ref, exact SHA, executable SHA-256, Rust versions, helper
archive hashes, command results/timings and run URL. It checks source cleanliness
again after validation. No code fixes, merges or application-branch commits occur. The hosted lane reuses
its native Cargo target cache through the pinned official Actions cache; Windows test machine uses
its separate local cache. The hosted restore fallback also reuses dependencies
across patch-version-only lockfile changes; Cargo validates/rebuilds fingerprints
for changed sources, dependency versions or compiler versions.

Service-mode checks run sequentially:

1. Native CI process-tree timeout/isolation contract tests.
2. Standard `build.ps1 -Release` (native MSVC), optimized locked Rust tests, strict
   Clippy and rustfmt.
3. Reader/search/selection, page assembly/scanned replacement, mark cleanup,
   overlay export, revision/remark export, protected-file aliases, and the 32-case
   lazy/eager page-geometry corpus.
4. Stage the actual custom-target executable and notices into a fresh directory,
   then launch it from an unrelated working folder without PDFium overrides or
   dependency PATH entries. This checks adjacent-runtime discovery, not an installer.

Generated PDFs, synthetic mark images, config, data and temporary files are isolated
per run. Report directories include run ID, attempt and lane under RUNNER_TEMP;
publication verifies the same identity to reject stale persistent-runner results. Existing smoke flags are removed. PDFium 7881 and qpdf 12.4.2 archives are
SHA-256-verified and extracted into the run directory; existing `C:\deps` is not
used or changed. The application does not acquire an automatic runtime downloader.

Every compiler/test child tree enters a Windows Job Object **while suspended**,
before executing. Limits: one Cargo build job, one logical CPU equivalent,
2 GiB aggregate job memory, below-normal priority, bounded per-suite timeout and
kill-on-close. A timed-out child's descendants are killed too. Failure to establish
limits fails validation. A bounded admission wait avoids starting beside another
Cargo/MSVC build or when less than 3 GiB memory is free. This cannot reserve the
whole machine against other work starting later; it never stops other processes.
The one service runner serializes Windows test machine jobs. Distinct tags have no shared pending
concurrency slot that could drop older pending validation runs.

After regression checks, a separate bounded Cargo metadata/tree pass collects
Windows dependency notice inventories and hash-verified texts. Its inventory-only
status and missing files are reported separately; it does not grant distribution
clearance or turn license auditing into an application test. New notice bytes are
retained losslessly in the diagnostic JSON for maintainers to review/import through
the normal source/release workflow. The complete bundle stays in the run artifact.

## Reports and diagnostics

Each lane independently uploads `report.json`, `README.md` and per-suite logs as
`windows-LANE-RUNID-ATTEMPT` artifacts (30-day retention). Preflight errors produce
FAIL reports; an empty or skipped suite cannot count as PASS. Checkout/interpreter
failures can prevent a report from being created and remain failed Actions steps.

An Ubuntu announcement job first creates a **QUEUED** receipt on the diagnostics
branch, before waiting for the Windows runner. This records pending work without
claiming test acceptance; the final publisher updates that same branch.

Superseded main-branch runs can finish as **CANCELLED**. Their completed checks
and active-check log remain diagnostic evidence, but a partial cancelled run never
qualifies a commit. An actual failing check remains FAIL even if a later workflow
cancellation occurs; a completed PASS is retained if only later cache work was
cancelled. Scale-case names containing numbers retain their failure logs too.
Every release tag keeps its own uncancelled validation group.

A separate **Ubuntu publisher** holds `contents: write`; Windows workers have only
`contents: read` and do not persist checkout credentials. The publisher verifies
report identity and writes only diagnostics under `docs/validation/automated/` on
`validation/ci-windows-LANE-RUNID-ATTEMPT`, based on the exact tested commit. The Ubuntu announcement/publisher jobs also have `actions: read` for job
metadata; the Windows worker does not. These
report-only commits are the owner-authorized CI exception to development commits
from `/app/dPDF/source`; they never merge or update `main`/release tags. Application
builds/releases still use the established source/authority workflow.

The Fedora development checkout can fetch these branches over its configured SSH
identity without GitHub CLI authentication or messages relayed by the owner:

```sh
git -C /app/dPDF/source fetch origin
# Inspect the latest matching origin/validation/ci-windows-* branch:
git -C /app/dPDF/source show origin/validation/ci-windows-self-hosted-RUNID-ATTEMPT:docs/validation/automated/README.md
```

A PASS establishes only the listed automated checks on the reported machine.
Publishing a report does not turn a failed validation job green. Hosted validation
remains independent when Windows test machine is offline or awaiting one-time registration.

## Dedicated desktop and physical printing

A Windows service runs in Session 0 and cannot qualify the user's desktop. To
prepare a **separate dedicated logged-in Windows test account**, use:

```powershell
.\scripts\ci\install-windows-runner.ps1 -Desktop
C:\actions-runner-dpdf-desktop\run.cmd
```

This installs a separate foreground runner named `dPDF-Windows-desktop`, with no service,
automatic login, scheduled task or changes to the user's active desktop. Keep that
session logged in and available only when desktop testing is wanted. Dispatch
**Dedicated Windows desktop validation** for an exact commit or tag. It records Windows session/window-station/input-desktop accessibility without
activating or switching desktops. Desktop validation rejects Session 0 and
inaccessible/nonvisible desktops and includes service checks plus real-window About, review, password,
recovery and **Microsoft Print to PDF** diagnostics. The print diagnostic refuses
hardware printer devices. Expect fixture windows and native dialogs in this test
session; never run this runner in the normal working account.

Manual pointer/clipboard interaction, installer/default-app integration and actual
paper output remain separate qualification tasks before stable milestones. A
service PASS alone does not qualify these. No network-facing service or desktop
permission is enabled to make a test pass.

## Primary references

- [GitHub: adding self-hosted runners](https://docs.github.com/en/actions/how-tos/manage-runners/self-hosted-runners/add-runners)
- [GitHub: runner routing and queue limits](https://docs.github.com/en/actions/reference/runners/self-hosted-runners)
- [GitHub: concurrency](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency)
- [Microsoft: Windows Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)
- [Microsoft: interactive services / Session 0](https://learn.microsoft.com/en-us/windows/win32/services/interactive-services)

## User-level runtime registration regression

Native service and desktop lanes now stage a uniquely named fixture executable,
register it under a random dPDF ProgID, verify its association/shortcut/icon via
Windows APIs, probe the resolved executable with a generated Unicode PDF, and
remove its own registration. Actual PDF defaults/UserChoice remain unchanged.
The corrected registration checkpoint passed all 15 service checks; exact identity
is retained in `docs/validation/windows-registration-checkpoint.json`. Desktop lanes
add their existing 11 GUI/clipboard/printing checks, for 26 total. Windows test machine remains an
independently queued lane until its runner is installed.

## Exact native artifact reuse

The controlled harness can reuse a build only for the same full commit, native
compiler/Cargo versions, flags/global configuration and binary SHA256. Cargo
identifies the actual output; immutable copies survive subsequent target builds.
Reuse proves build identity, not native acceptance: all applicable tests still run.
Installer work uses the same one-CPU/2-GiB child-tree limits and refuses existing
user installation files or shortcuts. Private native execution remains on the
controlled operator path; no private GitHub runner dispatch is enabled.

For private controlled packaging, use a persistent, isolated `DPDF_CI_ROOT`
before native validation. Keep the returned report with its immutable binary.
`build_windows.py --acceptance-report <report.json> --output <work>/dpdf-native-build`
refuses incomplete, wrong-commit, changed-binary or hosted substitutes. It writes
`artifact-path.txt` as private operator state; that path is not a release asset.
Run `package-preview.ps1` through `limited_windows.py`, passing that binary path,
the source manifest's version, the work directory and `-RequireAcceptance`.
A missing report/artifact or unavailable native target is PENDING. These commands
neither allocate a product version nor create a Git tag.
