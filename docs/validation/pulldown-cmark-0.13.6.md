# pulldown-cmark 0.13.6 qualification

Status: prepared focused dependency update; full compiled/native gates pending.
Base: de7a53d1c9c23f8389984a6d17df79f669c510d7. Application version remains 0.1.54.

[RUSTSEC-2026-0433](https://rustsec.org/advisories/RUSTSEC-2026-0433.html) identifies
quadratic work for nested image wikilinks when ENABLE_WIKILINKS or Options::all is
used. The option is disabled by default. The inspected locked Slint parser call
enables only strikethrough, so that observation does not establish an exploitable
application path; the vulnerable package version still warrants maintenance.
The [0.13.6 upstream release](https://github.com/pulldown-cmark/pulldown-cmark/releases/tag/v0.13.6)
backports the fix. This update qualifies that patch independently of helper or UI
changes and does not suppress the historical advisory match.

Both official registry archives were verified against the sparse index checksums.
Dependency requirements and feature declarations match exactly. MIT declaration
and LICENSE bytes are unchanged; Rust 1.71.1 remains below the application's 1.92
minimum. Changed archive files are limited to release metadata, parser code and
upstream wikilink tests. No other locked package or application dependency feature
is changed. Target inventories are updated for the new package identity; exact
native collection equality remains a merge gate, not a legal clearance claim.

An official OSV query for 0.13.6 returned no current matches. This is a dated
single-package observation, not a complete dependency, PDFium or system-library
security audit. Provenance, archive differences and lock hashes are in
[pulldown-cmark-0.13.6.json](pulldown-cmark-0.13.6.json).

Required before merge: repository contracts, format/Clippy, debug/release tests,
full PDF geometry/interop and window regressions, native Windows hosted build and
service corpora, exact Linux/Windows notice comparison and affected native UI
replay. Existing real Wayland/Windows input, physical printing and owner acceptance
remain separately unqualified. No local Rust compilation, release/tag mutation,
new license choice or production secure baking.
