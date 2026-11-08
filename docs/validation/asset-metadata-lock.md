# Asset metadata contention

The v0.1.34 hosted Windows service lane failed the concurrent usage/size test:
one thread could not acquire the asset metadata lock within one second. The
desktop lane passed, and both v0.1.35 lanes passed; those later results do not
erase the intermittent failure. See the
[recorded failure](asset-metadata-windows-v0.1.34.json).

A separate [generated Linux reproduction](asset-lock-baseline-20261030.json)
holds an asset's file lock for 1.3 seconds while another process saves its preferred
size. The writer exits before the lock is released, reporting contention and
preserving the previous metadata. Its binary is the scan-replacement candidate
before the lock change, identified by hash in the report.

The [v0.1.37 result](asset-lock-fixed-20261030.json) repeats that separate-process
case successfully: the writer waits for release, saves the new size and preserves
all original, cleaned and thumbnail image bytes.

Lock acquisition now allows a bounded five-second wait for contention, keeping
the existing five-millisecond polling interval and immediate failure for other
errors. Size and usage writes run on I/O workers, so this does not block page
interaction. The longer wait accommodates a brief slow writer; a lock held beyond
the bound still fails and reports an error.

The regression holds a lock past the previous timeout, releases it, and checks
that the waiting acquisition succeeds. It then keeps that acquired lock held and
checks that a second acquisition returns an error rather than waiting indefinitely.
The existing concurrent-update regression still checks fresh size/usage merging,
monotonic timestamps, preserved image bytes and refusal to overwrite future schemas.
