# Custom page-unit sizing investigation

The generated [v0.1.32 baseline](userunit-overlay-baseline-20261030.json) places a
40-mm-wide mark on a page with `/UserUnit 2`. Export preserves that page setting,
but the image matrix has a width of approximately 113.386 default user-space
units. Applying the page multiplier gives approximately 80 mm, twice the requested
width. This is a reproduced sizing defect, not qualified custom-unit support.

`UserUnit` defines the physical size of a default user-space unit as a multiple
of 1/72 inch. See Table 30 in the [PDF specification](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf).
The baseline records source/output hashes and the image matrix. Both PDFium 7881
and the installed Poppler return the unscaled 400-by-300 page dimensions for this
fixture, so agreement between their renders does not establish correct physical
size. The existing geometry benchmark compares lazy and loaded engine sizes; it
does not verify the physical multiplier.

The initial v0.1.34 preflight refused affected non-default-unit pages. Mark export
now normalizes millimetre geometry against physical page dimensions before native
DeviceToPage conversion back to raw PDF coordinates. Default, explicit, null and
indirect units are decoded from an immutable snapshot. Malformed, cyclic or
unresolved metadata fails rather than assuming a scale. The bounded qpdf inspector
is shared with review metadata; revision/remark models remain independent.

Export checks physical corner quantization within 0.005 mm, native image-vector
dimensions within 0.01 mm, and saved non-default units within relative tolerance
1e-6. Insufficient precision fails before replacing a destination. Image resampling
continues to use the requested millimetre dimensions and the existing 400-DPI cap.
The regression verifies PDF transform vectors independently of rendering, with
192 unit/crop/page-rotation/mark-rotation/alpha combinations and unchanged outside
pixels. Invalid units and a generated extreme-precision case preserve prior files.

Actual-size native printing now reads the physical multiplier from the captured
snapshot before opening a print dialog; Fit to Paper remains scale independent.
Generated half/double-unit letter pages retain the same printed marker geometry
in the GTK file-output regression; Windows runs equivalent native checks through
the release CI. Physical printers remain separate qualification.

Scan/photo page replacement now retains the target's page-unit multiplier and
physical dimensions. The [v0.1.35 baseline](userunit-replacement-baseline-20261030.json)
demonstrates the previous loss of `/UserUnit 2`, halving a letter-sized replacement.
Generated regression cases cover multipliers 2 and 0.5, unchanged raw streams on
the other two pages, source hashes, and the 300-DPI sampling budget. Image sampling
uses physical dimensions, with the existing 4096-pixel cap; it does not infer print
size from an imported image's DPI metadata. The unit update operates on the newly
generated image page through qpdf and leaves its image streams intact.

The reader now applies the physical multiplier after the first frame, using the
existing background metadata inspection. It preserves the reading point and keeps
raw engine sizes so repeated facts cannot apply the multiplier twice. Independent
review decoding errors do not discard valid physical dimensions. At 100%, equivalent
letter pages use 816-by-1056 logical pixels under the existing 96-DPI convention;
this is not monitor calibration. Mixed-unit Fit layouts use physical page sizes.

New mark placement waits for verified dimensions. A missing helper or invalid
unit metadata leaves reading available and refuses unverified placement. The
generated real-window regression covers units 2/0.5/1, source rotation/crop,
reading anchors, repeated/invalid facts, broken review metadata and background
tabs at three display scales. A two-process checkpoint test restores and exports
a custom-unit mark through the real worker, then verifies its PDF dimensions and
position. Native GTK and Windows virtual-printer checks compare a 40×20-mm mark
on physically equivalent unit-1/2/0.5 pages in Fit and Actual Size modes.

Physical-printer and broader real-office qualification remain open. Ordinary
default-unit pages retain their existing regression coverage. The production secure-block
feature remains disabled independently of this investigation.
