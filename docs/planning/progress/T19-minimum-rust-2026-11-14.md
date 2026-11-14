# T19 declared minimum Rust source checks

Status: IN_PROGRESS; implementation and process-injected contracts prepared,
actual compiler validation PENDING approved public CI.
Base main e7bec799b0d5ff30175fa28cd2ff743d9b9b4a72; isolated
/tmp/dpdf-minimum-rust, branch test/minimum-rust. Independent of unmerged lifecycle
and engine drafts #10/#16/#17/#18. It does not qualify their changed sources.

Reproduced evidence gap: Cargo.toml declares rust-version 1.92, while the public
Linux job installs/defaults stable and native MSVC environment selects stable.
Retained successful artifacts report newer actual compiler versions. Reading
cached package/index declarations or the official Slint 1.18.1 metadata (minimum
1.92) does not demonstrate this project's actual source compiles on 1.92. No
application compiler bug is inferred from that missing test.

Add an exact numeric minimum-toolchain source check to the existing Linux and
Windows hosted jobs, after stable runtime tests. Use private disposable targets,
locked all-target Cargo checks (test code included), and Linux Windows-print
metadata check. Verify source cleanliness/commit before and after, actual compiler
version and native host. Force Cargo to use the checked shim/version and disable
inherited compiler wrappers. Windows uses the existing bounded native process
runner/admission; no resource cap, worker, publisher or default compiler change.
Linux timeout/success cleanup owns its process group. Retain per-command logs and
FAIL/error state. Output reports live inside the existing validation artifacts.

Pure contract tests inject compiler results and execute no Rust: wrong source
refuses before commands; a newer compiler is not a minimum pass; failure exit 42
and its log survive with temporary target cleanup; aliases/command text cannot
be used as a version. All 20 CI contracts pass locally with four explicit existing
platform skips (16 passed). All fifteen local script suites exit 0: 122 tests, 118 passed and four existing
platform skips. A separate generated Python timeout fixture verified no live
descendant remains after Linux process-group cleanup; the adopted grandchild may
be a zombie awaiting its external reaper. This does not claim direct grandchild
waitpid by the checker. Full source checks must occur through public CI; no local
Rust compilation, default toolchain update, release, tag or version change.

Minimum-version Cargo check is source compatibility, not a runtime executable,
link/release build or PDF/native/installer acceptance on that compiler. Existing
stable runtime gates remain required. Keep failed compiler output and fix only
reproduced compatibility defects, or record an explicitly reviewed supported-
minimum change; never change the advertised minimum merely to hide failure.

Next: publish this focused candidate, inspect exact-source hosted compiler logs,
then record both native minimum targets and all existing regression outcomes.
No experienced lifecycle/engine review gate is waived for the other drafts.
Rollback: revert this focused CI/checker/docs increment; preserve qualified
artifacts and all source/user data.
