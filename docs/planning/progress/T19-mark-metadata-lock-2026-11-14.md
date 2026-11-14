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
