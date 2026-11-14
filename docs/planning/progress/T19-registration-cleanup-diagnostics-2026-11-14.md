# T19 Windows registration cleanup failure diagnostics

Status: IN_PROGRESS. Isolated accepted-main 68910141 checkout; no production
registration change and no identified cause/fix.

Lifecycle source 79ae2894 failed native Windows user-registration in run
43560066579, job 132341541114. The exact artifact ZIP checksum and failing suite
log are retained in [first failure](T19-registration-cleanup-first-failure.json).
Registration/update, association command/argv, shortcut/icon, staged runtime
launch, PDF default assertions and owned-key cleanup reached the final shared
RegisteredApplications value comparison. That assertion failed; its log records
no before/after snapshots. Cause is UNKNOWN. Neither absent-key normalization,
background mutation nor a product registry mutation is established by this log.
The registration suite bounded-job committed-memory peak was 109,064,192
bytes, not RSS, under
the unchanged 2-GiB cap; the reported failure is an assertion, not a cap exception.

The shared snapshot checker now emits a bounded diagnostic only on failure:
before/after absence and value counts, total changed count, at most 32 added/
removed/modified names hashed with their typed-value hashes, and whether each
name belongs to the generated fixture. Raw registry names/values are not printed.
Details beyond 32 remain explicitly counted; no difference is silently ignored.

The exact existing after == (before or []) predicate remains, including the
existing absent-versus-empty distinction. No default/UserChoice assertion,
collision refusal, cleanup order, timeout or production register.ps1 behavior
changes. A passing follow-up will not explain or erase the retained first failure.

Three source-safe tests verify unchanged strict predicates (including both
absent snapshots), added/removed/modified detection with sensitive values
redacted, and bounded detail without hiding failure. Native exact-source Windows
qualification and ordinary PR gates remain PENDING.

Local validation: fifteen source suites, 127 tests (123 pass/four existing
platform skips), including all three new diagnostic cases. Formatting, metadata,
JSON/local links and whitespace pass. No local Rust compilation.
