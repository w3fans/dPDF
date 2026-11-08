# Reading-position reflow regression

The generated real-window diagnostic first ran against unchanged v0.1.26 reader
resize behavior, with diagnostic-only additions. Hiding the sidebar at 200% zoom
reset both offsets to zero; the preserved point required approximately
(-484, -1103) logical pixels. The [baseline identity](viewport-jump-baseline-20261030.json)
records the candidate executable hash and exact source base without presenting
it as an unmodified release binary.

Automatic relayout now retains a page-relative point near the viewport center,
restores it at the corresponding viewport fraction and clamps at document edges.
It records the previous view before a resized scroll area can clamp its offsets.
Explicit page/fit/rotation commands retain their existing navigation behavior.

The real-window suite checks both directions of sidebar/search visibility and
physical window resizing across twelve layout/zoom/rotation combinations at
1×/1.5×/2×. Geometry tests additionally cover all quarter turns and fit-page zoom.
The tests use generated documents and isolated application data; native Windows
results remain attached to their exact release report branches.

Toolbar and Ctrl+plus/minus zoom previously performed an explicit page jump. The
[zoom baseline](toolbar-zoom-baseline-20261030.json) records a real dispatched
Ctrl+plus event failing with unchanged v0.1.31 reader code and diagnostic additions.
These commands now reuse pointer zoom around the nearest page point to the viewport
center, including gaps between pages. The suite dispatches both shortcuts in all
twelve cases at each scale, then restores the original fixed/fit-width setting
before checking resize behavior. Explicit fit and page-navigation commands keep
their existing behavior.
