# Native physical page-unit printing

The [generated v0.1.34 baseline](native-print-unit-baseline-20261030.json) contains
letter pages with `/UserUnit 2` and a raw 306-by-396 MediaBox. Actual Size incorrectly
used those raw dimensions instead of the physical 612-by-792 points. The native
GTK file-print diagnostic reproduced that failure before the correction.

Both native adapters now multiply engine page dimensions by the snapshot's page
unit before computing an Actual Size print layout. Metadata is read from immutable
bytes through the configured qpdf helper before opening the dialog. Metadata and
loaded page counts must agree. Fit to Paper remains scale independent and skips
that lookup. Existing raster budgets, output protection and cancellation remain.

GTK file-output and Microsoft Print to PDF suites generate equivalent letter
pages at units 0.5 and 2 with the same physical one-inch by half-inch marker.
Printed marker bounds must match the default-unit reference within three pixels
in both modes; source hashes must stay intact. Native Windows outcomes belong to
the exact release's CI report. These checks do not qualify physical paper output
or [private], and do not enable custom-unit mark exports or secure baking.
