# Generated-only Rust block replacement

This ports the combined-block experiment to `src/block_probe.rs`; it does not
enable production baking. The command creates only its own synthetic PDFs and
colored signature/stamp patterns. No real contract or signature can be supplied.

The 32 cases combine quarter turns, crop boxes, negative MediaBox origins and
inherited boxes/rotation. Case bits encode rotation (`case % 4`), inheritance
(`case & 4`), crop (`case & 8`) and negative origin (`case & 16`). The backend
maps all corners through PDFium at a million-step device grid, retains double
precision until the rendering matrix, and checks containment against the actual
inverse-mapped region. The output raster is opaque and approximately 400 DPI.

The driver requires unchanged outside pixels with a two-pixel boundary allowance,
exact untouched-page streams and rendering, removed label/date extraction in the
block, retained outside text and one final reachable image without a separate
alpha mask. Removing that image must leave the entire printed block blank.
qpdf assembles the affected page with the original untouched page.

Crossing text/paths, marked/alternate text, text/path clipping, annotations, forms
and empty regions are refused before output. Engine handles use explicit cleanup;
the save callback is bounded and derives its pointer from the complete callback
container. Existing output folders/files and arbitrary input paths are rejected.

An initial conservative catalog gate accepts only `/Type` and `/Pages` entries
with a valid page-tree root. Extended catalogs are refused before page-object
changes. Two additional fixtures retain the original block stream through a
custom catalog entry or an embedded backup file; both must be refused without
raw/baked outputs. The [retention investigation](catalog-retention.md) explains
why page search alone cannot establish removal. These catalog fixtures remain
part of the exact refusal coverage.

The page/tree/resource gate now also refuses unsupported properties on every page
and detects page-content streams referenced beyond their owning `/Contents` entry.
Shared content streams and indirect content-array ownership remain unsupported.
Reference scanning includes stream dictionaries, has a one-million-value budget,
and bounds indirect resource-dictionary resolution to 16 references with cycle
detection. These conservative checks can refuse harmless additional properties.

The [page/resource investigation](graph-retention.md) adds three retained-content
fixtures plus a shared-content refusal, bringing the required refusal identities
to fourteen. An additional positive case uses indirect resources and undergoes the
same extraction, appearance, outside-pixel and untouched-page checks as the geometry
matrix. Native reports must include that positive case as well as all 32 geometry
cases and fourteen refusal identities. This does not qualify arbitrary input.

Quality comparisons distinguish the 400-DPI print target from resampled views.
The print-target mean absolute RGB-channel error budget remains below 4/255;
preview and resampled 350-DPI comparisons use a separately reported budget below
8/255. Outside pixels must remain unchanged at every measured resolution.
The initial [sampling comparison](rust-block-sampling-20261030.json) records why
the former preview limit, based on simpler imagery, did not transfer to thin
low-resolution patterns: Rust and Python agreed, while print and preview errors
differed. A 350-DPI case with a negative origin/180° rotation exceeded the print
budget; the 400-DPI/high-precision profile passes the complete generated matrix.
Resampling still changes appearance; identical pixels at every zoom or printer
resolution are not established. These budgets do not establish security.

`rust-block-latest.json` is refreshed by the standard release gate. Native Windows
reports retain exact binary identity and the full generated matrix, and explicitly
reject qualified/enabled security claims or incomplete coverage reported as PASS.
Poppler remains an additional independent Fedora check.

Still unqualified: arbitrary input/catalog/resource reachability, fractional boxes
and UserUnit, arbitrary transparency/clipping, nested/shared content, tag trees,
independent review integration, production worker/UI, common editors and physical
printers. No cryptographic seal or legal-validity claim follows from the probe.

## 270-degree fixtures and the Poppler limit (2026-11-04)

Until v0.1.44 the generated fixture computed `/Rotate` as `u8` `id % 4 * 90`, which
overflowed for every fourth case and silently wrote `/Rotate 14` (rendered as
unrotated). The 32-case corpus therefore never exercised 270-degree pages. Pull-request
CI caught the overflow in a debug build; the fixture now writes `/Rotate 270`.

On the truly rotated cases the bake is geometrically correct: PDFium print-resolution
error is 1.41 (cases 3, 7) and 1.12 (case 11), the same as other rotations, and no
pixel outside the block changes in any renderer. Only the independent Poppler
comparison rose to 4.16-4.19 (unrotated case 0: 3.65), just over its former limit of 4,
because of Poppler's rasterisation phase on rotated pages. Its limit is now 5. All
PDFium limits and the zero-outside-pixel requirement are unchanged. This remains a
generated-fixture prototype; production Secure Signature Block stays disabled.
