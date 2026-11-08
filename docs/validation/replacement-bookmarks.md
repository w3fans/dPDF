# Bookmarks after page replacement

The generated [v0.1.37 baseline](bookmark-replacement-baseline-20261030.json)
replaces page 10 of a ten-page contract with a scan. All ten outline entries
remain, but only nine have valid destinations. The last bookmark points to a
null object after its original page leaves the page tree. This fixture does not
retain the old page dictionary at that reference.

The [fixed candidate](bookmark-replacement-fixed-20261030.json) repeats that
replacement with all ten targets valid and the original input unchanged.

This matches the page-selection limitation described in
[qpdf's documentation](https://qpdf.readthedocs.io/en/stable/cli.html#page-selection):
bookmarks refer to page objects, and destinations to pages omitted from the output
stop working.

Each planned page now carries its original primary-page identity where applicable.
Replacement retains that identity; movement/reordering carries it with the page.
Before assembly, dPDF captures resolved outline destinations to replaced primary
pages. Afterwards, a qpdf JSON update changes only those outline dictionaries to
point at their replacement pages. It retains destination zoom/coordinates, title,
hierarchy, styling and GoTo action chains. Named bookmarks receive equivalent
direct destinations. Restored targets are re-read and checked before the existing
qpdf validation and atomic destination replacement.

Inspection retains the 64-MiB metadata limit, with at most 10,000 bookmarks and
64 levels of nesting. Unsupported coordinate/action data fails before replacing
the destination file. Extraction still starts with a clean catalog.

The page regression checks all ten targets after PDF and PNG/JPEG replacement,
including oriented photos. A separate fixture exercises nested, direct, named and
indirect GoTo bookmarks for both scan replacement and mark export. It checks zoom,
styling, chained/external actions, unchanged page streams and source hashes.

This repair covers primary outline bookmarks. It does not rebuild general named
destination trees, page links, forms or tagged-PDF structure. Page assembly remains
separate from secure redaction; production Secure Signature Block is still disabled.
