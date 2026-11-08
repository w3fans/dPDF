# Release compiler memory

This historical comparison records the move to optimization level 2 for dPDF only.
The current optimization-1 evaluation follows renewed compiler memory pressure;
its runtime and full release checks are required before publication. Dependency
profiles, the single build slot, one CPU and all memory limits remain unchanged.
The [recorded comparison](release-profile-memory-20261030.json) identifies both
candidate artifacts by SHA-256 and includes systemd's build resource measurements.

| dPDF optimization | Compiler peak | CPU time | Observed wall time |
| --- | --- | --- | --- |
| 3 | 1646.3 MiB | 103.2 s | 2023.3 s |
| 2 | 1536.6 MiB | 102.8 s | 104.4 s |

The soft memory limit is 1536 MiB. The smaller peak avoided the prolonged stall
observed in the other build. Wall time depends on shared-host memory pressure;
these two observations do not establish a general speedup ratio. Both builds
completed successfully. Build source folders have been retired, so the report
records the development candidate and artifact identities rather than claiming
an independently reconstructed source comparison.

Five runs of each generated reader fixture gave these median headless first-page
times with PDFium 7881:

| Fixture | Optimization 3 | Optimization 2 |
| --- | --- | --- |
| 50 office pages | 11.70 ms | 11.56 ms |
| 1000 text pages | 33.00 ms | 35.31 ms |
| 50 vector pages | 18.67 ms | 20.17 ms |

The benchmark also checks lazy versus loaded geometry, bounded render buffers
and cache usage. These timings measure engine delivery, not desktop presentation.
The normal release gate still runs the PDF, export, real-window and native-print
regressions; the optimization change does not replace those checks.
