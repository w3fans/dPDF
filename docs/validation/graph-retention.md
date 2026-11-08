# Page and resource retention investigation

The [generated baseline](graph-retention-baseline-20261030.json) records three
retention paths beyond the catalog. An original page stream is referenced through
a page-tree property, an untouched page's PieceInfo, or an untouched page's resource
dictionary. After assembling the generated baked page with the untouched original
page, page search finds zero old-label matches. Decoded qpdf output still retains
the old label and date through each reference.

This repeats a normal generated bake with metadata-only changes to the original;
the CLI never accepts arbitrary PDF input. It identifies a preflight requirement
for the future production pipeline, rather than a shipped secure-export failure.

The generated probe now checks properties on all page/tree nodes and resolves
direct or indirect resource dictionaries before modifying objects. It separately
tracks original content-stream ownership and refuses references outside the owning
page's Contents entry, including aliases below recognized resource keys. Shared
page streams are refused. Unknown properties, resource cycles and excessive
reference traversal fail before raw or removal output is written.

The three original retention cases and a valid shared-stream fixture must refuse
output. A positive indirect-resource fixture must retain outside searchability,
exact untouched-page streams, print/preview appearance and the single opaque baked
image. Rust tests separately exercise aliases below known keys and resource cycles.
Windows report validation requires all fourteen refusal identities and the positive
graph case; a missing case cannot pass as complete coverage.

The gate is intentionally conservative and does not validate every possible PDF
resource or metadata structure. Broader nested/shared resources, tags, forms,
arbitrary transparency/clipping, independent remarks and real-input worker/UI
integration remain unqualified. Production Secure Signature Block remains disabled.
