# Production secure-region contract — proposed for review

T12 design candidate, 2026-11-13. **Not approved or implemented.** Production baking
remains disabled. Baseline source: `268b234e4336789922a6b15dd1236963e131d707`.
Requirements: R-SSB-1–8, R-PERF-4, R-INT-1–5 and R-REM-6 in
[PRODUCT_SPEC](../PRODUCT_SPEC.md). This document defines proof obligations; it
makes no claim that the current PDFium/qpdf combination satisfies them.

## Outcome and threat model

A separate exported copy replaces the effective region's live original content
and selected visual marks with one opaque combined image. Removing that image
must not reveal the former labels, signature/stamp objects or hidden original
region content. Outside content retains vectors, searchable text and geometry;
untouched pages retain their original content streams and rendering. Whole-page
flattening and a covering rectangle do not meet this contract.

This offers resistance to casual independent removal of a visual signature from
its surrounding labels. An editor can still replace the entire image or construct
a different document. It provides neither identity/authenticity nor cryptographic
integrity. This feature is not general confidential-data redaction: sensitive
information elsewhere, author metadata, repeated text and external copies are
outside its purpose. Do not market it as irreversible secrecy or legal validity.

The adversary may inspect raw/decoded objects, extract images or text, remove the
combined image and inspect retained objects/revisions. Passwords, private marks
and source bytes must not enter logs or public fixtures.

## Inputs, units and geometry

Inputs are immutable opened PDF bytes, explicitly selected mark-image snapshots,
page identity, region draft, export policy and generation. Never reopen mutable
source paths to obtain export content. Paths are protected destinations/identity
references, not proof of byte identity.

Persist the requested rectangle in unrotated PDF user coordinates tied to the
page's resolved MediaBox/CropBox, inherited Rotate and UserUnit. Physical length
in millimetres is user-coordinate length × UserUnit × 25.4 / 72. Screen coordinates
are only interaction input. Map all four corners through the tested page display
transform; do not apply a second Y inversion. View rotation and inherited page
rotation are distinct and must be recorded. Reject nonfinite/singular mappings,
invalid boxes/units and geometry beyond representable precision.

A nonnegative safety margin is physical millimetres, converted independently by
axis using the resolved page transform. Expand the requested region before object
classification. The effective region also includes outward raster-grid rounding;
show and confirm these effective bounds. Do not silently clamp a margin at the
crop edge or enlarge it after consent. A region outside the visible page is a
recoverable refusal. Fractional boxes, negative origins, quarter turns and custom
UserUnit need T13 fixtures before their support is approved.

At proposed default 400 physical DPI (within the product's 300–400 range), compute
raster width/height by outward rounding of physical extent. Derive the image
matrix from inverse-mapped corners at adequate precision. The image must cover
exactly the validated effective region; no remaining live sliver may lie beneath
expanded raster coverage. Region size, raster budget and sampling policy are
part of the plan, not decisions made silently during export.

## Content support and refusal table

The initial candidate subset is deliberately narrow and does not meet all v0.6
acceptance by itself. Every accepted route needs structural and independent
render/extraction tests. Unknown is a refusal, never an assumption of harmlessness.

| Route | Candidate policy / evidence needed |
| --- | --- |
| Top-level text/path/image wholly contained | Candidate support only when parser-level content ownership, graphics state, painted extent and removal are established. An engine bounding box alone is insufficient (stroke width, glyph overhang, rendering modes and transforms matter). |
| Objects wholly outside effective region | Preserve original semantics and resources; prove no alias to removed material. Outside objects that paint into the region are intersections, not outside. |
| Text/path/image crossing or touching uncertain boundary | Refuse `BOUNDARY_INTERSECTION`; do not remove whole crossing objects or clip them without a separately reviewed rewriting algorithm. Conservative tolerance must be explicit in geometry proof. |
| Shared streams/resources and indirect arrays | Traverse all owners, including untouched pages; refuse aliases retaining removed payload. Direct and indirect references both count. Initial subset may refuse sharing entirely. |
| Forms/nested XObjects, patterns, shadings, Type3 glyph programs | Initial refusal `UNSUPPORTED_GRAPHICS`; any future support needs recursive state/ownership proof, not just recursive deletion. |
| Clips, soft masks, blend modes, transparency groups | Initial refusal `UNSUPPORTED_GRAPHICS`; an opaque output image alone does not prove input compositing can be preserved. |
| Marked content, ActualText/Alt, structure trees | Initial refusal `ALTERNATE_CONTENT`; text search absence does not exclude alternate extraction payloads. |
| Optional content/layers | Initial refusal `ALTERNATE_CONTENT`, including currently hidden layers. |
| Annotations, widgets/forms, appearance streams, JavaScript/actions | Initial refusal `ANNOTATION_OR_ACTIVE_CONTENT`; inspect document-wide, including hidden/nonprinting annotations and AcroForm/XFA. This blocks mixed-review acceptance until the independent preservation path below is approved. |
| Attachments, portfolios, thumbnails, PieceInfo, custom catalog/page/tree/resource keys | Initial refusal `RETAINED_CONTENT_ROUTE`; use explicit reviewed key/value policies and reachability traversal rather than deleting unknown data. |
| Incremental history, unreferenced objects, object streams | Parse complete input; export fresh reachable structure without old incremental revisions or orphan payload. Refuse ambiguous parsing/repair and inability to prove serialization cleanup. Fresh save alone is not proof. |
| Malformed/cyclic graph, duplicate/ambiguous object identity, parser disagreement | Refuse `AMBIGUOUS_STRUCTURE`; benign shared references use visited sets, while invalid recursion/ownership requires refusal. No automatic repair presented as validated output. |
| Password-protected source | Initially refuse `ENCRYPTED_SOURCE`; future support requires in-memory unlocked snapshot binding and explicit output protection choice. Never persist password in plan/recovery/logs. Ordinary encrypted Save As stays byte-identical. |
| Existing cryptographic signature | Require explicit invalidation acknowledgement, with no verification/authenticity claim. Initially refuse `SIGNED_SOURCE` until signed-field handling and fresh serialization are reviewed; acknowledgement alone cannot bypass refusal. |
| Empty selected region | Refuse `EMPTY_REGION` in initial subset; do not silently turn this into ordinary overlay export. |
| Multiple regions/pages | Initially one region on one page; refuse `UNSUPPORTED_SCOPE` until overlap/order and resource sharing are reviewed. Never process a prefix of a request as a complete result. |

The supported subset must be useful on representative generated contract-like
fixtures. If ordinary labels/lines cannot meet the removal/outside-preservation
proof, return a no-go decision and revise the backend plan. Broad refusals are a
safe diagnostic outcome, not completion of the product requirement.

## Read-only preflight and immutable plan

Preflight reads the entire relevant document graph before any export candidate is
published. A result is one of `Refused { reasons }`, `Cancelled`, `Failed { kind }`
or `Validated { plan }`; a plan cannot be constructed by UI callers or by coercing
a warning into success. Preflight itself creates no destination file.

Proposed stable reason families are the uppercase codes in the table, plus
`INVALID_GEOMETRY`, `RESOURCE_LIMIT`, `STALE_INPUT`, `SOURCE_ALIAS`,
`HELPER_UNAVAILABLE` and `VALIDATION_FAILED`. Include safe human guidance and page
context, without passwords or extracted contract text. Parser/helper warnings
are explicit failures unless individually reviewed as harmless for this contract.

The nonserializable internal plan binds:

- Source byte digest and length; in-memory document identity and generation.
- Page ordinal plus parsed object/generation identity and resolved page geometry.
- Requested/effective region, physical margin, transforms and raster dimensions.
- Exact selected mark digests, decoded geometry/opacity and placement generation.
- Content/object ownership and removal set, retained-resource closure and all
  checked document routes; parser/engine/helper versions and policy revision.
- Review export policy and invalidation acknowledgement if supported, plus a plan
  digest covering every field used by export.

Choose the digest implementation only after dependency review; SHA-256 is the
proposed identity algorithm, not an instruction to add a crypto library now.
Never persist live engine handles. Rebuild plans on reopen or policy/version
change. Recheck all bound inputs before consuming a plan and immediately before
publication. Changed draft/source/marks produce STALE_INPUT, not partial reuse.

Proposed resource ceilings require measured review: PDF bytes 128 MiB, parsed
values one million, traversal depth 64, raster 8 megapixels/4096 per axis, and
candidate size 128 MiB. Count aggregate decoded bytes and reference edges, not
just unique objects. Use checked arithmetic before allocation. Deadline and
cancellation apply to parsing, helper pipes, rendering and independent validation;
T03 must supply tested bounded-helper behavior. The prototype's 32-MiB save cap
and other constants are observations, not production capacity evidence.

## Transaction, cancellation and engine ownership

Use the reader's sole engine-owning worker, or a reviewed bounded isolated child.
Never initialize/destroy the probe's global engine inside the running reader or
move PDFium handles across threads. An isolated child receives immutable bytes
and plan identity, returns bounded candidate bytes/results, and is killed/reaped
on timeout/cancel. This lifecycle choice needs experienced review before code.

Pipeline: validated snapshots → render original region plus selected marks →
remove validated original objects/content → insert one combined image → fresh
serialization → independent validation → atomic publication of a separate copy.
No helper may write directly to the final destination. Work happens in private
scratch with bounded cleanup. Candidate publication uses the existing storage
alias protections and same-directory temporary replacement, with a final identity
check against protected PDF/image/recipe inputs. Review race handling for changed
symlinks/destination identity before claiming protection against hostile concurrent
filesystem mutation.

Errors, disk full, warnings, cancel or validation failure preserve source bytes,
existing destination and draft histories. Completion carries tab/document and
generation; stale completion cannot clear newer edits or recovery snapshots.
Cancellation acknowledged before the publication commit point leaves destination
unchanged. Cancellation arriving after atomic replacement reports completed
publication; do not describe it as rolled back or automatically retry it.

## Independent review preservation is a mandatory later gate

Initial annotation refusal cannot satisfy R-REM-6/T18. Before product exposure,
review a separate path that classifies and snapshots foreign and dPDF annotations,
revision metadata, appearance resources and references before removal, proves they
do not retain replaced live material, and preserves the explicit export policy.
Annotations inside/crossing the region must remain independently hideable/editable
or be refused; never silently bake remarks into the signature image. A stale or
unsupported review model fails closed. Clean export affects remarks only and
must not unbake the block. Review-free prototype success cannot waive this gate.

## T13/T14 verification and reviewer decision

Retain all 32 geometry cases, fourteen existing refusal identities and the
indirect-resource positive case. Add generated positive/negative fixtures for
every table route, including hidden aliases, object streams/history, crossing
painted bounds, custom units, fractional boxes, graph cycles/limits and review
preservation. Include sentinel independent images/text and copies of replaced
payload in retained metadata to demonstrate refusals detect them.

For accepted cases require: independent parser object/text/image inspection,
PDFium and Poppler rendering, unchanged outside pixels at documented boundaries,
exact untouched-page content streams and renders, searchable outside text, one
opaque combined image with no separately recoverable constituent, and no retained
original payload in decoded/reachable or residual serialized content. Remove the
combined image in a generated verification copy and confirm the original region
is absent. Sentinel byte searches supplement structural proof; they cannot prove
removal on arbitrary documents. A visual tolerance budget is not a security budget.

For refusal/failure/cancel require unchanged source and preexisting destination
hashes, no published partial output, draft retention and bounded process/resource
cleanup. Include tab closure, shutdown, stale completion, alias destinations and
helper hangs/output floods. Physical ruler/print quality and common-editor checks
remain separate native/owner gates.

Reviewer must record: supported subset and practical value; parser/rewriter choice
and licenses; ownership/reachability proof; geometry/rounding policy; independent
oracle limitations; limits/lifecycle/cancel design; review-preservation path; and
GO, REVISE or NO-GO with rationale. No approval is recorded yet. T13 implementation
and T14 backend remain gated on approval; T18 is additionally required before UI
exposure. No version/tag or release operation follows from this document.
