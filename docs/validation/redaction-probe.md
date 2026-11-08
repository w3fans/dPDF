# Secure-region backend investigation — 2026-10-29

This is a capability investigation, not secure signature export or v0.6.0
qualification. The application still offers ordinary overlays only.

Probe: scripts/probe-redaction.py with official Fedora MuPDF 1.28.2-1.fc44,
qpdf 12.3.2 and PDFium 7881, extracted to /tmp without host installation. These
artifacts were used only on generated fixtures; this is not an engine-version
recommendation or licensing clearance.

| Generated case | Observed result |
| --- | --- |
| Text object crossing the rectangle | SECRET erased, two OUTSIDE phrases retained, outside pixels unchanged |
| Shared form invoked twice | Selected instance erased, other instance remains searchable, outside pixels unchanged |
| Crossing vector, remove-if-covered | Original live path remains inside the rectangle |
| Crossing vector, remove-if-touched | Whole path removed; 3,870 pixels changed outside the rectangle |
| ActualText marked-content payload | Visible glyphs erased and ordinary search empty, but original SECRET ACTUALTEXT remains reachable in the decoded content stream |
| Coarse 20×20 image crossing the rectangle | Original raw image pixels replaced, but whole intersecting source pixels also alter 11,250 outside-region raster pixels |

The outside comparison excludes only a two-raster-pixel border for edge rounding.
Original source hashes stay unchanged. A fresh garbage-collected output alone
fails to remove the ActualText payload, so an empty text-search result is
insufficient evidence. Both explicit line-art policies fail the strict crossing
vector invariant in different ways. Image pixel granularity also needs handling.

Do not wire this raw pipeline to a button labelled Secure Baked Region. Any
candidate must handle these failures, then prove crop/rotation/transforms,
clipping/transparency, annotations/widgets and independent remarks, shared image
resources, tag/alternate payloads, fresh-file/unreachable-object cleanup, final
raster placement and common-editor behavior. The current corpus is a starting
point for that gate, not its full completion. A subprocess boundary is not a
licensing decision; MuPDF remains unbundled and unselected.

Official references:

- [MuPDF PDFPage redaction API](https://mupdf.readthedocs.io/en/latest/reference/javascript/types/PDFPage.html#PDFPage.prototype.applyRedactions)
- [MuPDF PDF write options](https://mupdf.readthedocs.io/en/latest/reference/common/pdf-write-options.html)
- [MuPDF license](https://mupdf.readthedocs.io/en/latest/license.html)
- [Fedora MuPDF package](https://packages.fedoraproject.org/pkgs/mupdf/mupdf/)

Downloaded package SHA256 values:

```text
8b91776de1e376b4919610b2ed402079384fa6ae6423a2b886518f0be375ed88  mupdf-1.28.2-1.fc44.x86_64.rpm
af1622f1f7f36ca6bd4e148c3f2f5b2f00445c4cae606fabfcfebdd4bf42ec86  mupdf-libs-1.28.2-1.fc44.x86_64.rpm
```
