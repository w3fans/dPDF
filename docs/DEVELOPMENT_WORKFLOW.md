# Development and release workflow

dPDF has two cooperating authorities:

- **GitHub pull requests** are the normal place for development. Each change is a
  focused branch, validated by the applicable local/hosted gates before it merges into `main`.
- **The release checkout** (`/app/dPDF/source` on the owner's Fedora workstation)
  remains the only place that changes versions, writes release notes, creates tags
  and runs the full Fedora release gate through `./release.sh`.

This document describes how the two coexist without either silently undoing the
other. Product requirements are in [PRODUCT_SPEC.md](PRODUCT_SPEC.md); status is in
[REQUIREMENTS_MATRIX.md](REQUIREMENTS_MATRIX.md); gates in [MILESTONES.md](MILESTONES.md).

## Pull requests

1. Branch from the current `main`. Keep one concern per branch.
2. Make the change, with tests or a reproducible acceptance check where practical.
3. Add a bullet under `## Unreleased` in `CHANGELOG.md`. Update the requirements
   matrix when a row's status, evidence or limitation changes.
4. Open a pull request. **Pull-request CI** (`.github/workflows/ci.yml`) runs on
   hosted runners for independent public builds. Private heavy builds use the
   existing controlled local adapters described in [BUILD-INFRASTRUCTURE.md](BUILD-INFRASTRUCTURE.md):

   | Job | What it checks |
   | --- | --- |
   | Repository policy | No agent-specific or private files anywhere in the tree; no version, tag or release-note edits; an `Unreleased` changelog entry; release metadata consistency; Python contract tests for the CI, policy and release tooling. |
   | Linux | `cargo fmt --check`; Clippy with warnings denied (default and `check-windows-print`); unit tests in debug (overflow checks on) and release profiles; release build; dependency-notice inventory; then `scripts/ci/validate_linux.py`: every PDF corpus (reader, pages, marks, overlays, page units, review, aliases, block probe with Poppler, Poppler cross-check, desktop entry) and the private-Xvfb window suites at scales 1/1.5/2 (file drop, About, viewport, physical reader, tabs, clipboard/placement, review UI, passwords, recovery, native GTK print-to-file). |
   | Windows | The existing hosted-lane harness (`scripts/ci/validate_windows.py --lane hosted`) on `windows-latest` for the PR head commit: native MSVC build, tests, Clippy, format and all service corpora. Reports are uploaded as artifacts; nothing is published to `validation/*` branches. |

   Each job uploads `report.json`, `README.md` and per-suite logs with the exact
   commit and executable SHA-256.
5. Fix failures on the branch. Merge when every applicable validation passes.
   For private builds, retain the reviewed commit and local build/test receipts;
   a skipped hosted compiler job does not supply that evidence. Source-policy
   contracts remain required. Do not run untrusted PR code on privileged nodes.

Pull-request code never runs on the self-hosted Windows test machine runner. Windows test machine and the dedicated
desktop lane run only for trusted pushes, release tags or manual dispatch (see
[WINDOWS-CI.md](WINDOWS-CI.md)). A queued Windows test machine job is not a pass.

### Who may merge

Routine development pull requests may be merged once their automated gates pass.
Never merge, through a pull request, anything that:

- changes versions, `release.json`, `docs/releases/` or released changelog sections;
- promotes a stable milestone (`v0.2.0`, `v0.4.0`, `v0.6.0`);
- describes the Secure Signature Block as secure or enables it in production
  without the evidence named in MILESTONES.md;
- changes licensing commitments; or
- marks physical printing, Windows test machine or real-user acceptance as qualified from synthetic tests.

Those decisions stay with the owner.

## Releases from the release checkout

`scripts/sync-release.sh` copies the editable source over the release checkout before
every checkpoint and preview. Without care, a pull request merged on GitHub would be
overwritten by an older editable copy and reverted by the next checkpoint. The
release tooling therefore enforces:

- **`./release.sh upstream`** (run from the editable source) fetches `origin/main`,
  and, if it contains merged pull requests, writes the changed files into the
  editable source and fast-forwards the release checkout. It refuses, before
  writing anything, when the release checkout is dirty, has diverged from GitHub,
  or when the editable source has local edits to a file that also changed on
  GitHub (each such path is listed for manual merging). `--dry-run` only lists
  the update. An interrupted run completes when re-run.
- **`scripts/sync-release.sh`** and **`./release.sh patch`** refuse to run while
  `origin/main` has commits that the release checkout lacks, before any file or
  version is changed.

Typical owner sequence:

```sh
./release.sh upstream          # bring merged pull requests in
# ...optional local work, then the full Fedora gate and a preview:
./release.sh patch
```

### When to cut a preview

Normal development patch releases continue after completed functional work passes
the project's merge and release gates. The designated release authority reconciles
merged work with `./release.sh upstream`, then runs `./release.sh patch` for the
normal version progression, build, tests, validation, publication and authorized
installation/update process. Contributors use branches and approved CI; a second
development machine does not become an independent release writer.

Related completed changes can share a coordinated patch release. Do not withhold
qualified `0.1.x` previews merely because a stable milestone needs further
acceptance, an unrelated feature remains unfinished, or an audit recommends more
stabilization. Those recommendations are not a standing release freeze. In
particular, outstanding physical-printer, native Windows or production Secure
Signature Block qualification does not by itself prevent a clearly labelled
development preview. Production Secure Signature Block remains disabled until
its own security gate passes; other preview features retain their documented
qualification limits.

Genuine gates remain mandatory: failing tests or security checks, absent required
local/hosted validation, active release locks, conflicting source changes and required approvals must
be resolved. Report the specific failed gate and its corrective action rather
than attributing the delay to a general audit recommendation. Never overwrite
another source machine's work to make a release proceed.

Documentation-only audits and candidate-only tasks do not justify inventing
functional changes or deployments to create a version. `./release.sh checkpoint`
can still record intermediate work without a version change. Development patches
remain two-digit `0.1.x`; the ceiling is `0.1.99`, and the existing controller
refuses to cross into a stable milestone automatically.

### Stable milestones

`v0.2.0`, `v0.4.0` and `v0.6.0` are tagged only by the owner after every gate in
[MILESTONES.md](MILESTONES.md) is satisfied, including the physical-printer, Windows test machine
and real-user acceptance items that hosted CI cannot provide. The current preview
adapter (`scripts/release-version.py`) only releases `0.1.x`; a stable release
needs a deliberate, reviewed tooling change.

## Repository hygiene

The policy job enforces what `.gitignore` already expresses:

- no agent folders or instruction files (`.agents`, `.claude`, `.codex`, `.kimi`,
  `.cursor`, `AGENTS.md`, `CLAUDE.md`, `KIMI.md`, …) — project decisions belong in
  neutral documentation like this file;
- no credentials, key files, `ops/project.env`, signature/stamp asset folders,
  recovery snapshots or `.dpdf` drafts;
- generated fixtures only: never real contracts, signatures or business documents.

Normal `.github` workflows and `.cargo` configuration are project content.
