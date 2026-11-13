# Local Project Knowledge Audit and Handoff

Historical audit date: 2026-11-04. This document preserves that snapshot, including
its historical environment and evidence. For the active public checkout and
current implementation use the [2026-11-13 audit](planning/CURRENT-STATE-2026-11-13.md),
[requirements matrix](REQUIREMENTS_MATRIX.md), [delivery record](RELEASE-STATUS.md)
and [development workflow](DEVELOPMENT_WORKFLOW.md). Development branches/PRs
now come from the public clone; `/app/dPDF/source` remains the sole release
authority. Public CI and controlled local build/acceptance lanes coexist.

Historical repository state audited: `main` = `47ee628` (tag
`v0.1.43`). Purpose: make the product, implementation, decisions, acceptance
gates and roadmap understandable from GitHub alone, for example for a cloud
development agent with no access to the owner's machines.

## 1. Development environment

| Item | Value |
| --- | --- |
| Active development host | **release workstation**, the owner's Fedora workstation. The original specification named an earlier workstation, development workstation; release workstation is the current host. |
| Editable source | `/zDev/dPDF` on release workstation. No Git authority. |
| Git and release authority | `/app/dPDF/source` on release workstation (`main`, tracking `origin/main`). All commits, tags and release pushes come from here through `./release.sh`. |
| Build host | **remote build service**, a remote build service. `./build` and `cargo` on release workstation are wrapped to compile there with one CPU and 2 GiB; nothing compiles locally on release workstation. |
| Windows test machine | **Windows test machine**, the owner's Windows 11 PC, with separate Claude and Codex tester sessions. It builds natively with MSVC through `build.ps1`. |
| Automated Windows CI | GitHub Actions `windows-latest` service and desktop lanes run on every `v*` tag. An `dPDF-Windows` self-hosted lane exists, but its runner has not been online. |

At audit time: editable source and release checkout were identical for all
tracked content, with no unpublished application changes. The checkout had no
uncommitted, untracked or stashed work and no other local branches or worktrees.
Remote branches beyond `main` are report-only `validation/*` branches.

## 2. Local-only knowledge found and what happened to it

| Local material | Content | Action |
| --- | --- | --- |
| Owner "dPDF Build Prompt v4" (2026-10-28; only in a local development-session record) | The complete original product specification: goals, UX, features, Secure Signature Block, remarks/revisions, milestones, Scenarios A–E, out-of-scope list | **Consolidated** into [PRODUCT_SPEC.md](PRODUCT_SPEC.md), product content only. The raw prompt is not published: it also contains agent deliverable instructions and machine names. |
| Local project facts (ignored agent folder) | Paths, hosts, release authority, current-work log | Neutral parts are summarised in section 1 and in [REQUIREMENTS_MATRIX.md](REQUIREMENTS_MATRIX.md). Machine-private details stay local. |
| Local mission and workflow rules | Owner versioning rules, release rules, validation pairing, Windows-validation agreement | Product-relevant rules moved to PRODUCT_SPEC section 14 and to section 5 below. Agent quota and session rules stay local. |
| Reviewed engineering lessons | Seven technical lessons from real failures | **Transferred** to section 4. |
| Secure-block investigation notes and private code drafts | Next steps for geometry and graph preflight | Neutral design notes are **transferred** to section 4. Unreviewed code drafts stay local. |
| Physical-geometry draft notes | Plan for UserUnit reader layout | **Already shipped** in v0.1.40/41; nothing to transfer. |
| Task queue file | One stale entry ("initial 0.1.0 reader preview") | Superseded by REQUIREMENTS_MATRIX. |
| 0rig factory/runner/thread-role metadata | Disabled automation configuration | Agent configuration; stays local. |
| Agent session journals, leases and audit logs | Process records | Stay local. |

Already authoritative on GitHub before this audit:

- [ARCHITECTURE.md](ARCHITECTURE.md): detailed and current design and integrity
  boundaries.
- [MILESTONES.md](MILESTONES.md): gates per milestone (one stale passage corrected,
  see section 6).
- [TESTING.md](TESTING.md): Scenario A–E procedures and every check script.
- [VALIDATION.md](VALIDATION.md), [WINDOWS-CI.md](WINDOWS-CI.md),
  `docs/validation/`: evidence, including Windows test machine reports v0.1.3–v0.1.16 and hosted
  reports.
- [BUILDING.md](BUILDING.md), [LICENSING.md](LICENSING.md),
  [THIRD_PARTY.md](THIRD_PARTY.md), `CHANGELOG.md`, `docs/releases/`.

## 3. Agent-file exclusion check

- **Tracked files:** no `.agents/`, `.claude/`, `.codex/`, `.kimi/`, `.cursor/`,
  `.githooks/`, `.vscode/`, `AGENTS.md`, `CLAUDE.md` or `KIMI.md`. `.gitignore`
  already excludes the agent and provider folders, `ops/project.env` and runtime
  state. No `.gitignore` change was needed.
- **History:** `git log --all --name-only` over all reachable commits on all
  branches (528 distinct paths, 363 commits) shows no agent-instruction paths and
  no credential-like files.
- **Content scan:** all reachable commits were searched for common token and
  private-key patterns (GitHub tokens, private-key headers, cloud keys, chat
  tokens). Nothing matched.
- **Remaining mentions:** tracked docs name the machine roles release workstation, remote build service and Windows test machine
  (glossary in section 1). They contain no addresses, user names or credentials.
  `dPDF.code-workspace` is an editor workspace with only colours and title
  settings; the shared project standard keeps it as project content.
- **No history rewrite** was needed or done.

## 4. Engineering knowledge transferred from local notes

### Reviewed lessons from real failures

1. Keep a Slint repeated model **stable during pointer drags**. Replacing the
   `ModelRc` recreated the conditional resize handle and lost pointer capture;
   updating rows of the same `VecModel` passed on X11 and Wayland.
2. Keep canonical Windows paths for Rust file-integrity checks, and normalise
   drive/UNC paths consistently at qpdf JSON and CLI boundaries. Windows test machine showed JSON
   assembly succeeding while CLI validation failed on the same verbatim path.
3. Synchronise aspect-locked size fields inside save/place callbacks. A separate
   periodic preview timer does not guarantee consistent dimensions for immediate
   user actions.
4. Protecting existing files needs **filesystem identity**, not just canonical-path
   equality. Windows test machine reproduced a drive/UNC alias overwrite; shared same-file handles and
   hard-link/symlink regressions now cover the export paths.
5. Private GTK tests must force `GDK_BACKEND=x11`. Even with `WAYLAND_DISPLAY`
   unset, GTK can still connect to the normal `wayland-0` socket.
6. GTK reads `gtk-print-backends` from GtkSettings. The similarly named environment
   variable alone does not exclude lpr/hardware backends; set the property before
   print modules load.
7. Cairo's PDF surface can keep image references after painting. Use Cairo-owned
   image storage, not a short-lived Rust pixel `Vec`. GTK's registered GObject
   types also need their dynamic libraries to stay loaded until exit.

### Secure Signature Block: current state and next steps

- **Prototype:** `--probe-block` and `scripts/probe-combined-block.py`. On
  generated fixtures, live labels, lines, the date and two assets are removed; one
  350-DPI image is inserted in the historical Python prototype (the current Rust
  probe constant is 400 DPI); untouched pages keep exact streams and pixels; outside
  pixels are unchanged in PDFium and Poppler; unsafe structures are refused before
  mutation (14 refusal identities as of v0.1.42). A 96-DPI preview showed higher
  pixel error (MAE 8.19) than 350 DPI (2.32) because of rasterizer phase
  differences. This is documented, not hidden.
- **Production stays disabled.** Do not add an enabled "Secure" button because
  generated fixtures pass.
- **Geometry approach:** map a selected PDF-space rectangle with
  `FPDF_PageToDevice` / `FPDF_DeviceToPage` at a fixed canonical device size.
  PDFium already applies the page display transform, so do not Y-flip twice. Build
  the image matrix from inverse-mapped corners (origin = bottom-left; first column
  = bottom-right − bottom-left; second = top-left − bottom-left). This handles
  quarter turns and crop/negative origins. Report effective bounds after rounding,
  and never leave live content under an expanded bitmap.
- **Graph preflight next:** extend the conservative preflight over the page tree
  and page/resource metadata of **every** source page before mutation. Retained
  content can hang off untouched pages through PieceInfo, custom page-tree keys or
  shared resource dictionaries. A narrow key allowlist is not a general proof.
  Clips, masks, marked content and UserUnit inside the block remain unqualified.
  Catalog attachments are already refused (v0.1.39).
- **After the preflight audit:** design the production worker, region UI (drag,
  handles, margin, anchors), templates and bake warnings. No MuPDF dependency has
  been added.

## 5. Process rules at the historical audit (current rules: DEVELOPMENT_WORKFLOW)

- **Versions:** 0.1.x patch previews. v0.2.0/v0.4.0/v0.6.0 are reserved for
  accepted milestones. Two-digit patch numbers only. Version numbers are
  calculated by the release tooling, never hand-edited.
- **Every change:** update `CHANGELOG.md` under `## Unreleased`; keep
  [REQUIREMENTS_MATRIX.md](REQUIREMENTS_MATRIX.md) current when a row's state or
  evidence changes.
- **Releases:** only from the release workstation release checkout, through `./release.sh patch`,
  after the full Fedora gate (Rust tests, both Clippy configurations, every check
  corpus, real-window suites at three scales, native GTK printing). The tag
  triggers the hosted Windows lanes and the preview publication.
- **Windows validation:** the Windows test machine tester of the **same provider** as the developer
  validates (Claude-developed → Windows test machine Claude; Codex-developed → Windows test machine Codex). Reports
  go on `validation/windows-vX.Y.Z` branches with exact commit and executable
  hash. Fixes never land on report branches.
- **Integrity:** never present a covering bitmap as redaction or secure baking.
  Never modify the source file. Generated fixtures only in tests: no real
  contracts, signatures or business documents in the repository.

## 6. Discrepancies and corrections (refreshed 2026-11-13)

| Item | Resolution |
| --- | --- |
| MILESTONES described `/UserUnit` mark export as a live defect | Corrected: resolved in v0.1.35–v0.1.41 |
| Spec places the Remarks toggle and revision selector on the main toolbar | Remarks toggle and shared revision selector now exist in toolbar/More (selector merged in PR #4); T04 implementation/generated checks complete, broader native acceptance remains T06 |
| Spec asks for "Send via Email" with attachment | Deliberately not built; copy path / open folder fallback per R-SHR-2 |
| Spec status bar shows remarks/revision state | Implemented and covered by smoke/review UI tests (R-UI-4); owner visual acceptance remains |
| Spec lists Fedora/Windows packaging scripts | v0.1.54 Windows MSI/portable and Fedora installed qualification recorded in RELEASE-STATUS; manual install/default-app acceptance remains |
| Windows test machine runner lane | Never online. Tag jobs expire after 24 h. Last native Windows test machine pass was v0.1.14 |
| Printing speed on PostScript-slow printers | R-PRN-6 compact PDF jobs implemented for eligible Linux CUPS queues; Windows optimization and physical-office-printer measurements remain |

## 7. Historical cloud-agent environment notes

**Can learn from the repository:** the full product specification, current
implementation status with evidence, architecture, integrity boundaries, test
procedures, milestone gates, release history and validation reports.

**Cannot do or reach:**

- **remote build service build service and release workstation desktop.** `./build` calls an release workstation-only wrapper. In
  authorized public hosted environment use plain `cargo build --locked`,
  `cargo test --locked --release` and
  `cargo clippy --locked -- -D warnings` (also with
  `--features check-windows-print`). `.cargo/config.toml` limits jobs to 1, which
  must be preserved on controlled infrastructure. This development workstation
  must use the approved remote wrapper, not local compilation.
- **Runtime helpers.** Most `scripts/check-*.py` need PDFium chromium/7881
  (`pdfium-binaries`, Linux x64 archive SHA-256 recorded in
  [BUILDING.md](BUILDING.md)), qpdf ≥ 11 (12.x tested) and, for real-window
  suites, an Xvfb binary, xdotool and GTK 3. Pass them with `--pdfium`, `--qpdf`
  and `--xvfb`.
- **Releases.** Do not bump versions, edit `release.json` or create tags. Work on a
  branch and open a pull request; hosted pull-request CI (Linux and Windows) validates
  it, and the owner's release workstation session brings merged work in with `./release.sh upstream`,
  runs the full gate and releases. See [DEVELOPMENT_WORKFLOW.md](DEVELOPMENT_WORKFLOW.md).
- **Windows test machine, physical printers and real-user acceptance.** These stay owner-side gates.
- **Local agent configuration** is intentionally private and is not needed for
  product work.

## 8. Readiness verdict

From GitHub alone, a cloud agent can understand the product, the implementation
state, the decisions and the remaining roadmap, and can develop and test on a
branch. It cannot release, qualify a milestone or reproduce the remote build service/Windows test machine/printer
environment. Those steps remain with the release workstation release workflow and the owner.
