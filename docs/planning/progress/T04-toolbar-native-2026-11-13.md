# T04 toolbar and long-label native X11 checks

Status: partial native evidence; task remainsIN_PROGRESS.
Verified executable from run43486282919/PR #10 candidate9519b5c, actual merge
sourcec9153a54197ac8725c30b662f723378e9323672b, executable SHA256
797319200d5341956bb4370e2ccf0e7a9a69a59dc2bea3a22737eb402c87555d.
Relevant toolbar/shortcut source matches merged #4/#9. This does not qualify the
async worker lifecycle review or other native platforms.

Fedora45, private authenticated X11, winit-software, scale1, 800px window height.
Generated50-page PDF and isolated config/data only. Inspected widths900/1119
(compact) and1120/1140/1180(wide). Primary controls remained reachable; at1120
Print options meets the right edge, so broader geometry/scale checks remain open.
No speculative breakpoint change made from that single observation.

Created the generated revision label 'Revision label with a deliberately long
generated description for the November review round' through native text/button
input. Wide toolbar/Review selectors and narrow More selector elide the label
without losing their arrows or forcing an oversized control. More contents fit.

Directly clicked Revision1 in the More ComboBox at900px: the menu closed, Review
selector and status context changed to Revision1, then native PageDown navigated
to page2, demonstrating reader shortcut focus after selection. Screenshots:
/tmp/dpdf-t04-layout-pointer/pointer-first-revision.png and after-page-down.png.
A prior Home/Return attempt closed the inner dropdown without changing its
selection; Home navigation was not treated as supported or as a confirmed app bug.
No test result was inferred from that attempt. Direct pointer selection passed
in a fresh workspace without recovery-state carryover.

Fixtures/scripts/screenshots retained under /tmp/dpdf-t04-layout*; generated
processes exited. No source PDF bytes changed. Full focus/shortcut matrix,
empty/loading cases, themes and scales1.5/2 still need systematic coverage;
Fedora Wayland/Windows native and owner acceptance remain T06 gates.


## Focused scaled replay

Repeated with explicit `xdotool windowfocus --sync` and a checked X input focus,
using a fresh isolated workspace at each scale 1, 1.5 and 2. A private 2560×1800
X display accommodated the logical 1180×800 and 900×800 windows. No product
scaling or desktop security setting changed.

At each scale, native typing created the long label, with the resulting full
label visible in status context. Wide and compact selectors elided it while
retaining the selection arrow. In light mode, selecting Revision 1 from More
closed the menu and native PageDown rendered page 2. In dark mode, selecting the
long-label revision closed More and PageDown rendered page 3. Screenshots and
native field/status contents were inspected; OCR separately confirmed the
rendered page numbers. Source PDFs were unchanged and every test process exited.
Hashes and exact binary/source identities: T04-toolbar-focused-scales.json.

The earlier /tmp/dpdf-t04-scaled-selectors replay did not explicitly focus its
window: typing never created its long label and keyboard navigation stayed on
page 1. Those observations were inconclusive input delivery, not an application
regression or a passing scaled long-label test. The corrected sequence lives at
/tmp/dpdf-t04-focused-selectors.py; captures are under the matching directory.

T04 remains IN_PROGRESS: the full text-field/save/tab/Undo shortcut matrix,
empty document and loading-review states still need coverage. This synthetic
X11 evidence does not close native Wayland/Windows T06 or owner acceptance.


## Empty and loading-review states

Exercised empty document and a controlled stalled metadata helper at scales 1,
1.5 and 2 in light/dark themes, with the same verified binary and fresh isolated
workspaces. Wide logical 1180×800 and compact 900×800 controls fit. More's Remarks
and revision selector stayed disabled when empty or loading. Native pointer
selection attempts left the disabled selector pixel-identical in all 12 cases.
The PDF remained rendered during the metadata stall. Explicit helper release
then enabled Remarks and the Revision 1 selector at each scale. Captures were
inspected, including normalized selector contact sheets. Source hashes were
unchanged and generated processes exited. Receipt: T04-toolbar-empty-loading.json.

This completes the listed generated geometry/state checks; it does not close T04.
Native text-history testing reproduced a further focus crossover and led to
PR #12; see T04-focused-text-history-2026-11-13.md. Save/tab shortcuts and native
Wayland/Windows acceptance retain their separate required evidence.
