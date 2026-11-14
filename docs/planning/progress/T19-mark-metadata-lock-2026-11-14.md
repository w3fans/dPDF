# Windows concurrent asset-metadata lock diagnosis

Status: IN_PROGRESS; hosted failure reproduced, cause not established.
Base main e7bec799; isolated test/mark-metadata-trace. This is an existing
mark-metadata regression, not a minimum-Rust compiler incompatibility or new
lifecycle/engine implementation. T10/native acceptance remains separately gated.

PR #19 head 1b5f6927, run 43544105201, Windows job 132291176971 failed its
release unit suite: 67 passed, one failed, one intentional subprocess fixture
ignored. The concurrent usage/size test's usage writer exhausted the five-second
metadata_lock try-lock budget. Stable build, Clippy, format and all other reported
corpora passed. Native minimum-Rust checks were skipped, so that result is pending.
Linux's actual Rust 1.92 source checks passed separately. Failed Windows ZIP
14093747060 hash 8af10791193761a309056763df85639ba05aef0676358bb891d91aa422f16c18,
report/log hashes and source identity are in the retained failure record.

The existing fixture starts two writers together; each performs sixteen metadata
updates. The evidence does not distinguish a single slow lock holder from a
waiter behind repeated acquisitions. Keep both failure and original policy. Do
not increase its five-second lock budget, suppress a failed update, change the
number of writers/updates or relax content/size/timestamp/future-schema assertions.

Capture (step, start_ms, duration_ms) around each existing operation. Each writer
returns its trace and any error; join both before asserting, retaining the winning
writer's trace when the other fails. Only synthetic metadata/error timing is
printed, never private asset content. The same named test runs once more with
nocapture in the existing bounded native Windows plan, giving a retained timing
log even if that run passes. This targeted repeat investigates an actual failed
case; no new worker, compiler route or resource cap is introduced.

Source formatting, whitespace, metadata and pure CI-plan contracts are checked
locally. No Rust compilation runs on this workstation. Next: inspect current-
source Windows timing traces, decide whether production locking or the fixture's
throughput assumption is wrong, then implement and qualify only the evidenced
correction. A single subsequent pass does not erase the original failed receipt.
No release, tag, dependency, signing or production baking change.


## First timing result and bounded follow-up

Head 4ed03140 / run 43546562452 passed all three jobs. Verified Windows artifact
14094533080 (ZIP eb85415aa57203d1fefb31d7fabd4d260bcb938385af06bfaf51745f629a27fb)
and exact-source timing log. Both writers completed all sixteen updates; largest
observed operation was 75 ms. Full release unit suite also passed. This does not
explain or erase the prior five-second acquisition failure. Durations include
waiting plus the metadata transaction, not isolated hold time. See
[the retained observation](T19-mark-metadata-qualified-4ed03140.json).

Reconcile main 3518aedf documentation, preserving both changelog entries. Replace
the single diagnostic invocation with at most 32 identical cases, all original
writer/update/assertion behavior unchanged. Stop at the first nonzero exit and
retain it; never retry a failed case to obtain green. A 210-second scheduling
budget reports INCOMPLETE/nonzero if exhausted; the enclosing existing Windows
Job Object still owns the unchanged 240-second deadline and all descendants.
No new build service, worker, resource cap or production policy is introduced.
Two injected-runner contracts prove failure stops immediately and incomplete
budget is not PASS. No local Rust compilation runs. Inspect the bounded samples
before selecting a production or fixture correction; no root cause is asserted.


## Qualified bounded sampling and minimum-compiler main reconciliation

Head 56e29774 / run 43548629971 passed all jobs. Verified both artifact ZIP
digests, Linux runtime/source/tree identity, exact native notice inventories and
all 648/485 referenced notice texts. Windows completed all 32 generated cases
(1,024 updates) in 29.688 seconds, retaining sixteen updates per writer and every
original assertion. Largest observed operation was 666 ms (case 31, size writer,
first update). Full parallel unit suite separately passed 68 tests with one
intentional subprocess fixture ignored. The named samples run independently of
that parallel suite; their timings combine waiting and transaction I/O. See
[T19-mark-metadata-qualified-56e29774.json](T19-mark-metadata-qualified-56e29774.json)
for source, archive/report hashes and all synthetic traces.

No failed sample or error-kind trace occurred, so the original five-second
failure remains unexplained. Retain instrumentation for a future reproduced
failure; these passes justify no production lock change or relaxed fixture.

Main now includes minimum-compiler PR #19 as 847d1c58. Reconciled it through a
normal merge, keeping both changelog entries and every CI contract from both
branches. AST comparison proves all 23 helper/test methods are retained unchanged
(22 tests). This combined source must pass fresh hosted gates, including both
actual minimum-compiler checks; earlier qualified sources remain historical
evidence. No app behavior, locking policy, resource cap, release or version change.

All fifteen source contract suites pass on the combined checkout: 124 tests,
120 passed and four existing platform skips. Formatting, metadata verification
and whitespace checks pass. No Rust compiler was run locally.
