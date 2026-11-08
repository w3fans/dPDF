# Catalog retention in region replacement

The [generated investigation](catalog-retention-baseline-20261030.json) demonstrates
two ways block text can remain reachable independently of the replacement page.
It bakes the ordinary generated case, then assembles that page with an original
whose catalog differs by a retained content-stream reference or embedded backup
file. Page search reports zero matches for the removed label, yet qpdf's decoded
output still contains both the original label and date.

This experiment does not supply arbitrary input to the CLI: the probe continues
to generate its own fixtures. It demonstrates a missing preflight requirement for
a future production pipeline, rather than a claim about a shipped secure export.

The Rust probe now checks bounded qpdf metadata before changing page objects.
Its deliberately narrow first gate accepts only a basic catalog containing
`/Type` and `/Pages`, with a page-tree root. It refuses all extended catalogs,
including the two generated retention cases, before raw or removal output is
written. The driver preserves source hashes and reports each refusal separately.
The native Windows report consumer checks the exact required case sequence,
including both catalog fixtures, and rejects duplicate or missing identities.

This gate also refuses harmless extended catalogs. It is a conservative prototype
boundary, not a general PDF classifier. Page-tree and resource graphs, tag trees,
alternate extraction metadata, forms and independent review integration still
need qualification. Production Secure Signature Block remains disabled.
