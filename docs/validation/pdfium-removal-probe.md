# PDFium whole-object removal investigation — 2026-10-29

This is **not secure signature export**. No signature is composited, no new baked
block is inserted, and no production feature is enabled. The probe accepts only
its own generated fixtures, not arbitrary contract inputs. It investigates the
already selected PDFium 7881 engine without introducing another engine or license.

Reproduce using the standard built dPDF executable and matching runtime/helpers:

```sh
python3 scripts/probe-pdfium-removal.py \
  --pdfium /path/to/libpdfium.so --qpdf /path/to/qpdf \
  --binary target/release/dpdf --output /tmp/removal-report.json
```

Exit **3** means the investigation completed and observed a qualification blocker.
The retained machine-readable evidence is
[pdfium-removal-probe-20261029.json](pdfium-removal-probe-20261029.json).

The probe removes only objects whose complete bounds fit within the rectangle,
calls `FPDFPage_GenerateContent`, saves nonincrementally, then rewrites through
qpdf to inspect decoded, reachable content. An object crossing the rectangle is
refused before mutation. Original source hashes remain unchanged. It compares
rendering outside the rectangle, excluding only two raster pixels at the edge.
Coordinates in this experiment are unrotated PDF points on a 400×300-point page.

| Generated case | Observed result |
| --- | --- |
| Contained text | Removed glyphs and raw SECRET payload; outside pixels unchanged |
| Text crossing the region | Refused before mutation; no output created |
| Contained line | Whole line removed; outside pixels unchanged |
| Line crossing the region | Refused before mutation; no output created |
| ActualText belonging only to removed text | Glyphs and alternate text removed; outside pixels unchanged |
| ActualText shared by inside/outside text | **SECRET remains searchable and reachable** after removing the inside object; outside pixels unchanged |
| Last form placement removed | No SECRET payload survives the PDFium/qpdf rewrite |
| One of two shared form placements removed | Other placement remains searchable; outside pixels unchanged |
| Last image placement removed | No image resource survives the rewrite; outside pixels unchanged |

These results support further investigation of whole-object replacement. They do
not prove general redaction. In particular, object removal alone does not repair
shared marked-content replacement text. A candidate must reject unsupported
structures or correctly rewrite them and prove the resulting content, including
alternate extraction paths. Empty pixels or one empty search query are inadequate.

Still unqualified: partial glyph/path/image surgery, nested-form mutation,
shared-resource isolation beyond these cases, arbitrary page coordinates and
transforms, clipping/transparency/masks, tag trees, annotations/widgets, independent
review handling, compositing and final baked-block insertion, Windows execution,
and common editor behavior. A conservative whole-object route must fail closed
when a selected rectangle crosses unsupported content; enlarging the region would
require the user's explicit choice and its own validation.

This complements the separate [MuPDF capability investigation](redaction-probe.md).
MuPDF remains unbundled and unselected. Neither probe is a production backend.

Public API references at the tested branch:

- [PDFium object removal, bounds, marks and regeneration](https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/7881/public/fpdf_edit.h)
- [PDFium nonincremental save API](https://pdfium.googlesource.com/pdfium/+/refs/heads/chromium/7881/public/fpdf_save.h)
