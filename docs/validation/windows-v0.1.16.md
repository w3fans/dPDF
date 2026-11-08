# [private] Windows check: dPDF v0.1.16

Date: 2026-10-29.
Selected release: `v0.1.16`, exact commit `738350ee75ee46fe7e8af56f080bdc16a9ab0af6`.

## Qualification state

**Local [private] build/corpora: DEFERRED at resource admission; not a pass or an application failure.**

The unmodified `scripts/ci/validate_windows.py --lane [private]` was launched on [private] with the selected clean checkout and isolated native MSVC homes. Its pre-build admission check requires at least 3 GiB available RAM. Repeated observations found approximately 2.00–2.04 GiB free on this 8 GiB Windows machine. No compiler or validation child was started. After approximately six minutes the childless admission waiter was stopped deliberately. Other applications were left running. No dPDF instance or fixture print dialog remained open.

Separately, `python scripts/ci/test_ci.py` completed on native [private] Windows: **6 tests passed**, including the native timeout/descendant-kill Job Object check and report publication contract. This does not qualify an application build.

The v0.1.16 hosted Windows lane has an independent **PASS**:
[Actions run 40873488539](https://github.com/w3fans/dPDF/actions/runs/40873488539),
[exact hosted report](https://github.com/w3fans/dPDF/blob/validation/ci-windows-hosted-40873488539-1/docs/validation/automated/report.json).
The fetched report's source commit matches the selected full SHA. All 14 listed checks passed: CI contract, PowerShell syntax, native release build, release unit tests, strict Clippy, format, reader, pages, marks, overlays, review, aliases, geometry and staged-reader. Its recorded executable SHA-256 is `28ff798ed734f58a539736bccdf49f63f7ea91cc152ac3abcd35267fec819916`. These results belong to the GitHub-hosted worker, not [private] or [private].

The [private] Actions validate job in that run remains queued with no assigned runner. A queue receipt is not a native validation pass.

## Requested unattended workflow

[private]'s handoff was found at `[private]:/zDev/dPDF-[private]-setup`, containing `README.md` and `install-[private]-runner.ps1`; there was no `/zDev/dPDF` directory on [private]. The original installer was copied to [private]'s isolated dPDF runtime setup folder, read and parsed successfully. Its actual path guards accept the default runner root and reject development-tree paths. It was not modified.

The installer configures a dedicated `[private]-dPDF` GitHub Actions service under NETWORK SERVICE, triggered by release tags. The repository harness uses one Cargo job, sequential suites, one logical CPU equivalent, a 2 GiB child-tree cap and a 3 GiB free-memory admission gate. Interactive desktop and physical printing qualification remain separate.

Installation is **pending**, because this execution session is not administrator, only the Codex-bundled per-user Git was found, and no authenticated GitHub CLI was available to request a repository runner registration token. VS x64 C++ tools are present at `C:\BuildTools\dPDF`. No runner service was created or registered. Complete the documented one-time administrator setup with machine-wide Git and a short-lived registration token entered only at the installer's hidden local prompt; never send credentials through chat.

An **active hourly check in the existing Windows-validator chat** was created and read back. It checks release/CI state, avoids overlapping test jobs, retains the resource limits, closes task-owned instances, and suppresses unchanged-state updates. It will retry local validation when the memory prerequisite is satisfied. This chat monitor is separate from the still-pending Actions service.

## Limits

No new application build, native print output, physical printing, manual clipboard/pointer acceptance or installer integration was qualified locally at v0.1.16. The preceding v0.1.14 [private] report documents the native printing diagnostic's Explorer-hosted-dialog locator blocker. Application sources under `src/` did not change between v0.1.14 and this selected release; build/staging/CI changes still require their own exact-commit qualification.

Only this diagnostic report is added on this branch. No fixes, merges, main updates, private documents or credentials are included.
