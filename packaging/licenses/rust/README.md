# Rust dependency notice inventory

These files retain dependency declarations and upstream notice texts. dPDF-owned
code uses the repository's Apache-2.0 LICENSE; dependencies retain their own terms.

`inventory-linux.json` and `inventory-windows.json`, when present, identify the
target dependency trees captured using standard Cargo metadata/tree commands.
Both runtime and build/proc-macro dependencies are included conservatively. Cargo's
resolved feature union is recorded as metadata; this is not proof that each feature
is linked into the executable. Files under `texts/` preserve original bytes, named
by SHA-256, and are shared between component records to avoid duplicate copies.

Public open-source distributions select Slint's **GPL-3.0-only** alternative.
dPDF-owned files retain Apache-2.0; the combined application is distributed under
GPLv3. Previous previews selected Slint's royalty-free desktop terms. Those and
other upstream alternatives remain in the notice archive as references.
License alternatives for other crates are
retained as declared; this inventory does not automatically select legal terms.

Seven published crate archives omit standalone notice files. `overrides.json` records
reviewed replacements from the exact source revisions in their `.cargo_vcs_info.json`,
with crate checksums, URLs and text hashes. The collector verifies these against the
cached package revision and lockfile. It never fetches or follows external files.

Generation: `scripts/collect-cargo-notices.py`. The metadata and target tree must
come from the same source/features/target. The collector uses package names and
versions from `cargo tree --edges normal,build --prefix none --format '{p}'` to
avoid including unbuilt optional backends from the broader metadata graph.

**Inventory only.** Rust's standard library/native toolchain, PDFium, qpdf, system
libraries and vendored notices not found by the root notice scan need separate
review. Missing-file counts and `qualified_for_binary_distribution: false` remain
explicit. A complete inventory is not a production distribution-compliance audit.
