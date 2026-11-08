# Development hosts and release ownership

Roles identify responsibilities, not personal computer names or network topology.
The local private project configuration maps these roles to actual machines and
paths; it is never included in Git or source RPMs.

| Role | Checkout | Rights and responsibilities |
| --- | --- | --- |
| Source A: release workstation | Configured editable source plus `DPDF_RELEASE_CHECKOUT` | Imports merged PRs with `./release.sh upstream`; owns versions, release leases, tags and `./release.sh patch`. Remains the single release authority. |
| Source B: additional developer workstation | Independent Git clone/task worktrees | Uses focused branches and PRs. Does not push main/tags, run release patches or overwrite Source A's editable files. |
| Remote Rust build service | Disposable per-source build mirror/cache | Compiles through the configured resource-capped wrapper; never edits authoritative source or publishes releases. |
| Windows qualification workstation | Independent native MSVC checkout and isolated fixture data | Receives exact-commit validation requests; reports native UI/printing results separately from hosted CI and notification receipt. |
| COPR build infrastructure | Submitted, sanitized source RPM | Compiles Fedora packages from declared source inputs. Does not become Git/tag/release authority. |

Verified baseline: `main` and the release checkout were both `d8425df31fe9`,
preview `v0.1.45`. The release checkout was clean and `./release.sh upstream
--dry-run` found nothing to import. Source B's packaging/privacy work and the
print-benchmark PR are independent branch work, not unpublished edits over the
release checkout.

Before a release, fetch upstream and import merged work through the existing
conflict-detecting path. Keep one release lease/lock owner. A second source machine
does not grant a second release writer. A handover requires an explicit owner
decision and a checked, lease-guarded transfer.

Machine-local build commands belong in `DPDF_CARGO`, transport destinations in
`DPDF_RELEASE_TARGET`, and paths in the corresponding private configuration.
Do not copy whole working trees between developer machines or publish local
agent state, dMsg enrollment data, documents, marks or recovery files.
