# T19 pulldown-cmark maintenance increment

Status: DONE for this scoped patch; overall T19 IN_PROGRESS.

Merged PR #14 as 1adea6f519160c89934a20d8c183885fa4a03fab. Final head f3cb4bf3
passed all jobs in run 43510850834: Linux 132180400450, Windows 132180399960,
policy 132180400752. That head only adds retained qualification to code candidate
a4e4896f, which passed run 43508365932 and source-bound UI replay.

Official archive checksums, source revision, dependency/feature equality, unchanged
MIT bytes, native inventory equality and all 18 pixel-identical About comparisons
are retained in docs/validation/pulldown-cmark-0.13.6.{md,json}. Exact Linux and
Windows notice texts were hash-verified. Final run artifacts: Linux 14079595720,
archive SHA256 0a2fa572e2b4ea517dcbd5719014e534846563762191cfb7f33a6d0ef091ca88;
Windows 14078583324, SHA256 2ef495e066bd87e8ba4b839e485d7672a00fdb9c0222273748402d348cdaf9f9.
Those final archives are identified by API metadata; the independently downloaded
and verified code-candidate archives are recorded in the qualification JSON.

Local integration preserved 79 unrelated pending hashes from the complete 80-file
backup /tmp/dpdf-before-main-1ade.*, adding only the focused parser changelog bullet.
Both target notice checks, format/diff and metadata verify passed. No local Rust
compilation, release/version/tag or production baking. Other maintenance/advisory,
package and manual platform gates remain open.
