# Combined signature-block experiment

This generated-only prototype goes beyond the earlier object-removal probe: it
renders printed labels/line/date plus two synthetic translucent assets into one
350-DPI block, removes the original live objects, inserts the opaque combined
image and assembles the other page from the original with qpdf stream preservation.
It is **not a production secure-export backend**. dPDF has no secure-bake button
or arbitrary-contract input path through this experiment.

```sh
python3 scripts/probe-combined-block.py --binary target/release/dpdf \
  --pdfium /path/to/libpdfium.so --qpdf /path/to/qpdf \
  --output /tmp/combined-block-report.json
```

The developer check also requires installed `pdftoppm` / `pdftotext`. It generates
all PDF/image inputs in an owned temporary directory; no real signature is used.
Optional `--artifacts NEW_DIRECTORY` retains generated PDF/raster evidence for
inspection and refuses an existing directory. Exit 0 proves these fixtures only.
Set `DPDF_TEST_BLOCK_PROTOTYPE=1` for the standard Fedora patch gate to refresh
`combined-block-prototype-latest.json` with the exact executable/engine hashes.
The dated initial [evidence](combined-block-prototype-20261029.json) identifies the
v0.1.21 executable used for the first completed experiment.

The dated initial observations below describe the original unrotated fixture.
The later [geometry investigation](block-geometry.md) adds 32 generated rotation/
crop/origin/inheritance combinations; production qualification remains false.

Observed for the initial generated unrotated 400×300-point fixture:

- Five live objects (label, line, date and two images) became one opaque
  924×389-pixel image in the 190×80-point rectangle, approximately 350 DPI.
- Both PDFium and Poppler extract outside text and no former label/date text.
  Decoded qpdf output contains one image and no original label/date payload.
- Deleting that one image removes the printed block and both synthetic assets
  together; it leaves a white region, with outside text still live.
- The other page keeps byte-identical raw content streams and rendered pixels.
- PDFium at its standard 1000-pixel probe and Poppler at 350 DPI report zero
  changed pixels outside the block (a two-pixel boundary allowance).
- Mean absolute RGB-channel error inside the block was 1.62/255 with PDFium and
  2.32/255 with Poppler at print resolution. The earlier 96-DPI Poppler comparison
  measured 8.19/255: identical appearance at every preview zoom is not established.
- Crossing text/paths, marked content including shared ActualText, any annotation,
  forms and, in the initial version, rotated pages were refused before mutation.
  The current prototype instead covers generated rotations and also refuses empty
  blocks; broader supported-input preflight remains unfinished.

Two implementation details were necessary: qpdf must explicitly preserve stream
data for untouched-page byte comparisons; PDFium's matrix renderer first applies
its default display transform, so the extra crop matrix uses device-space scaling
and translation. A mistaken second Y flip produced a blank block and failed the
appearance test. Passing extraction checks alone would have missed that error.

Still unqualified: arbitrary input/catalog/resource preflight, crop/negative origins,
UserUnit and arbitrary transforms/clipping/transparency, nested/shared forms,
tag trees and alternate extraction metadata, separate remarks integration,
Windows execution, common editor behavior and the production Rust/UI implementation.
Annotations are refused here rather than baked into signatures. No cryptographic
seal or legal-validity claim follows from this experiment. Stable v0.6.0 acceptance
still requires the complete supported workflow and its safety review.

A later generated shared-text-clip fixture exposed an additional integrity limit:
removing contained text/path objects and regenerating the page changed 604 pixels
outside the selected block in the PDFium probe. The hidden clip text disappeared;
the observed failure was outside-region appearance, not retained selectable text.
The prototype now refuses text clipping modes and actual path/text clip state
anywhere on the page before mutation. Both clipping fixtures assert refusal,
unchanged input hashes and absent output. A non-null clip handle is insufficient:
PDFium can expose an empty handle with a path count of -1; zero paths can still
represent text clipping. Supporting clipped input remains unfinished work.
