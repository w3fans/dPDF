# T19 declared minimum Rust source checks

Status: DONE for native minimum-source-check tooling. Final explicit-target
source 87e12390 passed all hosted gates and both actual Rust 1.92.0 native source
checks in run 43548625649; merged as 847d1c58. Minimum-compiler release/runtime
and native operator/package acceptance remain separately unqualified.
Historical candidate states below retain their original scope.
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


## First hosted result and independent-check admission correction

Head 1b5f6927, run 43544105201: policy and Linux passed. Linux's actual compiler
report verifies Rust 1.92.0 / x86_64-unknown-linux-gnu and both locked all-target
checks. Stable Linux executable matches main's qualified bytes. Windows built,
passed its other reported corpora, but release unit tests failed in the existing
concurrent mark usage/size metadata test (67 passed, one failed, one intentional
subprocess fixture ignored). A writer exhausted metadata_lock's five-second wait.
No production cause is claimed yet; timing evidence must distinguish one slow
write from repeated-writer starvation. The failed report/log/artifact are retained.

Windows's minimum step was skipped after that stable-suite failure, so native
Windows Rust 1.92 remains untested. Make both independent minimum checks run when
the job has not been cancelled, including after another validation failure. The
overall job retains its earlier FAIL. This reports each independent result without
rerunning, skipping or relabelling the failed mark test. No deadline/resource cap
is increased. Mark-lock diagnosis is separate work; full corrected-source gates
and native Windows minimum evidence remain required before merge.


## Qualified native minimum checks and main reconciliation

Head 63607995 / run 43546102757 passed all three jobs. Verified artifact ZIPs,
source identities, both native Rust 1.92.0 compiler reports and locked all-target
source checks. Linux also passed Windows-print adapter metadata checking. Stable
Linux runtime bytes equal the existing main qualification. Actual native notice
inventories match Linux 349 / Windows 255 components; all 648 / 485 notice refs
were rehashed. Windows minimum-check aggregate committed-memory peak was
1,254,400,000 bytes under the observed 2,147,483,648-byte Job Object limit; this
is not RSS or a whole-app memory budget. Full identities/reports are in
[T19-minimum-rust-qualified-63607995.json](T19-minimum-rust-qualified-63607995.json).

Reconciled documentation main 3518aedf in a normal two-parent merge. Only
CHANGELOG insertion conflicted; retained both focused entries. App source,
Cargo configuration/dependencies and runtime pins remain unchanged. Strengthen
the checker to select the native target explicitly in both Cargo argv and its
environment, so an inherited cross-target setting cannot change the claimed
scope. Existing injected-runner test now supplies a conflicting target and checks
that it is replaced. No incorrect target was observed in the qualified run.

The final explicit-target follow-up repeated all applicable hosted gates. The
prior Windows lock failure remains retained and under independent diagnosis;
a later passing run is not a root-cause verdict. No minimum-compiler release
binary, physical/native operator acceptance or experienced lifecycle/engine
approval follows from these source checks.


## Final explicit-target qualification and merge

Final source 87e12390 / run 43548625649 passed policy 132305660372, Linux
132305660793 and Windows 132305660688. Verified actual Rust 1.92.0 compiler
reports, explicit native GNU/MSVC targets and all seven minimum-check commands.
Both stable suites passed (33 Linux / 18 Windows checks). Exact native notice
inventories match 349/255 components, with all 648/485 referenced texts rehashed.
Linux archive 14095503559 SHA-256
60b94b7e24e8418dc9f1ec80cf0a5f580c16f4ba4e3a62551d0ba24069320c47;
Windows archive 14095344006 SHA-256
f305e99a4742c4de32c7db70a56eba1b95bc21fbce59aaaee17440df18aadc16.
Windows minimum-check peak committed job memory was 1,256,439,808 bytes under
the unchanged 2,147,483,648-byte cap; not RSS or a whole-app budget.

Routine CI PR #19 merged as 847d1c581bf80ce0e66f2d25d9242a4b4b7e3333. This closes
the declared-minimum source-test coverage gap and changes no app/runtime pin or
release authority. It does not explain the earlier mark-lock failure, qualify
unmerged lifecycle/engine sources, or provide minimum-compiler release/runtime,
physical/native operator or package acceptance.
