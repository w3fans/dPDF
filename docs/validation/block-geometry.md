# Generated block geometry coverage

This extends the [combined-block experiment](combined-block-prototype.md), not the
production app. `qualified_for_secure_export` and `dPDF_secure_export_enabled`
remain false. There is still no arbitrary-contract bake action.

```sh
python3 scripts/probe-block-geometry.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --qpdf /path/to/qpdf \
  --output /tmp/block-geometry.json
```

The 32 original generated cases combine all quarter-turn rotations with cropped
or uncropped pages, positive or negative MediaBox origins, and direct or inherited
page boxes/rotation. Each places two synthetic translucent assets over printed
labels, a line and date, then removes those five live objects and inserts one
opaque block at approximately 350 DPI. Input signatures are never real assets.

Checks cover both PDFium and Poppler at print resolution: outside-region pixels
stay unchanged within the two-pixel boundary allowance; inside-block RGB error
stays below the declared threshold; label/date extraction disappears while outside
text remains. The other page keeps exact raw streams and rendered pixels. Removing
the one image leaves the block blank. Original file hashes stay unchanged.

The prototype maps all rectangle corners through PDFium's native page/device APIs.
It records the effective PDF bounds after its 10,000-step coordinate mapping and
uses those bounds for object containment checks. The image transform is derived
from inverse-mapped displayed corners, so rotations and crop offsets are reflected
in both capture and insertion. A production UI must expose its actual effective
region; it must never cover live content outside the region it validated.

`DPDF_TEST_BLOCK_GEOMETRY=1` runs this developer investigation in the standard
Fedora patch gate and writes `block-geometry-latest.json` with executable/engine/
prototype identity and case results. A partial run is INCOMPLETE, never PASS.
The test requires existing Poppler developer tools and opens only its own fixtures.

This does not qualify fractional boxes, UserUnit, arbitrary clipping/transparency,
nested/shared forms, tag trees/alternate extraction metadata, catalog/resource
reachability, independent review integration, Windows/common editors, physical
printing or the production Rust worker/UI. Passing these cases is evidence for
further implementation, not completed Secure Signature Block acceptance.
