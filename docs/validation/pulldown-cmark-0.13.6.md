# pulldown-cmark 0.13.6 qualification

Status: code candidate a4e4896f qualified below. The final documentation commit
still requires its own normal PR gates.
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


## Code candidate qualification

Candidate a4e4896f278a8a3bf9a5861aa31bcf5f02993b3d passed every job in
[run 43508365932](https://github.com/w3fans/dPDF/actions/runs/43508365932): policy,
Linux format/Clippy/debug/release tests, all 33 Linux geometry/interop/window
suites, and 18 Windows hosted checks. The Linux tested merge was
5f16d23bde0de75ab08b43f66e1a22912728fe0e; Windows tested the PR head directly.
The verified artifact archives, executable hashes and job IDs are in the JSON.
Every retained Linux/Windows inventory field equals native collection, with all
notice-text SHA256 values verified (349/255 components, 648/485 references).
The fresh Windows build peaked at 1,766,981,632 bytes job committed memory under
the unchanged 2,147,483,648-byte cap; this is an observation, not an RSS guarantee.

The source-bound Linux executable was compared with the prior main executable
on the same private authenticated X11 display and software renderer. All 18
About snapshots are pixel-identical: light/dark app-only snapshots and details,
plus native F1/open, pointer scroll and Escape at scales 1/1.5/2. Main readers
remain open, generated PDFs remain unchanged, and no external link was opened.
The fixture initially used an OR window query; correcting it to require process
ID AND title removed a false close failure. All final input checks passed.
These are scoped UI comparisons; they do not replace real Wayland/Windows input,
physical printing, installer or owner acceptance. No release/version/tag changed.
