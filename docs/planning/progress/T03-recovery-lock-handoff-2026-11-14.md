# T03 recovery store lock handoff

Status: IN_PROGRESS, focused ownership fix prepared; exact-source native/public
CI qualification PENDING. Baseline accepted main 414a040f; isolated checkout
/tmp/dpdf-recovery-lock-handoff. No local Rust compilation.

## Observed failure and reproduction

Page-input source e1af0ea8 failed Linux debug tests in 43565022530, job
132356847217: 92 pass/one failure reopening Store immediately after its owner
was dropped. The Store implementation matches accepted main byte-for-byte.
[First failure](T03-recovery-lock-first-failure.json) retains exact log/artifact
identity. The ZIP contains minimum-compiler reports only, not a failed release
runtime. Later release/runtime checks skipped, not passed.

A bounded two-process Linux fixture reproduces the inherited-handle mechanism:
closing a close-on-exec owner's file handle leaves nonblocking acquisition
refused (errno 11) while a forked child retains it; explicit unlock allows
immediate handoff with that child alive. New-owner exclusion survives closing
the stale child; all fixture bytes remain unchanged. Original CI concurrent
descriptors were not captured, so this attribution remains an inference. It
does not diagnose earlier decryption/metadata/registration failures.

[Rust 1.92 File locking](https://doc.rust-lang.org/1.92.0/std/fs/struct.File.html#method.lock)
and [Linux flock](https://man7.org/linux/man-pages/man2/flock.2.html) describe
duplicate/inherited lock lifetime and explicit release. The newly added Rust
Store regression has not run locally or been claimed passed from the OS model;
the pre-fix Rust evidence is the retained original CI failure.

## Focused fix and invariants

Immediately after successful try_lock, wrap the File in private StoreLock. Its
Drop explicitly unlocks; raw-file close remains the fallback if unlock errors.
Declare the guard last in Store so nonce cleanup and all other fields finish
under ownership; initialization errors after acquisition also drop the guard.
[Rust field destruction order](https://doc.rust-lang.org/reference/destructors.html)
is declaration order. No other recovery worker, data/format/budget, retry,
checkpoint-publication or engine ownership behavior changes. Competing live
owners remain refused; acquisition remains nonblocking.

The deterministic Linux Store test saves a checkpoint, duplicates the actual
lock handle, verifies live-owner refusal, drops Store and requires immediate
handoff while that duplicate remains. It checks checkpoint bytes/candidate
retention, continued new-owner exclusion before/after stale-handle closure, and
normal subsequent handoff. All original assertions remain, without sleeps,
retries or larger limits. The duplicate models the inherited open-file
description without forking this multithreaded Rust test harness.

Pending: approved public Linux/MSVC source, debug/release/Clippy/runtime gates,
actual native minimum compilers, notices and affected recovery acceptance. This
focused main-based fix excludes unmerged lifecycle and engine implementations;
it waives none of their required experienced reviews. No new dependency,
runtime pin, version/tag/release, resource cap, private build route or production
baking change. Rollback is a focused guard/regression revert; retain failing
fixtures and all user checkpoint bytes.

Local source-only checks pass: fifteen suites, 127 tests (123 pass/four existing
platform skips), formatting, metadata, JSON/links and whitespace. These do not
execute the new Rust regression; its approved hosted checks remain PENDING.
