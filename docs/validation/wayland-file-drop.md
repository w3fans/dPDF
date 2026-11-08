# Native Wayland file-drop regression

Baseline: v0.1.50, source e9b837c06dbd6741ce5bc9e9113f81034c881372.
An isolated installed Mutter 51 compositor delivers real seat input to ordinary
GTK 3 and dPDF Wayland clients; X11 is disabled. Documents are generated fixtures.

The positive control transfers two local PDF URIs between GTK applications,
including Unicode, spaces, percent and hash characters. The same native drag
onto the baseline reader starts at the GTK source, but the source receives no
accepted action and the reader opens no document. The source occupies only
320 by 237 pixels; the pointer drops onto the exposed reader at (1000, 700).
Ctrl+W reaches the reader through the actual Wayland keyboard protocol.

Winit 0.30.13 has no Wayland data-device receiver. Upstream tracks this in
https://github.com/rust-windowing/winit/issues/4810 ; the newer upstream API is
incompatible with the pinned Slint integration. This branch retains the 0.30
API and adds a bounded incoming local URI receiver. Its source archive identity
and retained licenses are recorded in third_party/winit/DPDF_PATCH.md.

Required fix verification: native Unicode multiple-file drop, duplicate tabs,
hover cancellation, unchanged source bytes, malformed/remote URI refusal,
resource bounds, existing X11 and reader regressions. These are not marked
passed until the patched executable is built and exercised. The isolated
compositor does not establish owner desktop or physical-printer acceptance.

Baseline native dialog checks also pass through the private Wayland compositor:
Ctrl+O opens the portal chooser; Escape returns to the empty reader; a second
chooser opens the generated 50-page PDF; Ctrl+S saves a separate copy through
the native Save As chooser. Source and output SHA256 are both
`c6327e578b0acda7e5279551c4889b0059c01581ff7534710af4cfc1dffa5a86`.
These checks use the exact v0.1.50 native binary with SHA256
`0e7086e08dd4f62fdd084157bd0c4a3e795fc0bbc33c3de8cb36f54131aa3d16`.
They do not use application-dispatched input hooks. A preliminary chooser
fixture assumed a nonempty title; that assumption was corrected to identify
the actual portal window, and the corrected fixture completed with exit zero.

## Patched candidate

Source `b9216f9793e6645080933f32939ba41bcad03b2c`, native binary SHA256
`5c69722ad2cfdae43e3f6463ef77a852e29377eeb2513987c4b8a8d89c6c760f`:
74 release-profile Rust tests pass, including the shared local URI parser's
Unicode/deduplication, malformed/remote refusal and resource-bound cases.
Strict release-profile Clippy (`-D warnings`) and native build pass on the
configured Linux build service. All 778 tracked source inputs match both the
job copy and compiler workspace; server and downloaded binary hashes match.

The corrected binary passes the same real Wayland drag fixture that fails on
the baseline: hover cancellation does not open a tab; a completed URI drop opens
two PDFs including Unicode/percent/hash filenames; duplicate inputs produce no
extra tab; native Ctrl+Tab switches between them; source bytes remain unchanged.
The manifest remains 0.1.50 during this unreleased PR test. This binary must not
be attached to the immutable v0.1.50 tag or labelled v0.2.0.

The installed Fedora 45 v0.1.50 RPM independently passes the native Wayland
Open/Cancel/Save As sequence through its normal launcher. It also passes native
Ctrl+P, tab switching while the GTK print child remains open, Escape cancellation
and subsequent reader responsiveness, with both 50/300-page source PDFs unchanged.
The test waits for print-window mapping before restoring reader focus; the first
unsettled focus request alone was insufficient. These remain print-dialog tests,
not physical-printer or Windows qualification.

All six existing candidate regression suites also pass: reader corpus, X11
file-drop, viewport at 150%, physical reader layout at 150%, tabs at 150%, and
AES-128/AES-256 password handling. These use isolated X11 displays and the
same candidate binary hash above. “Physical reader layout” is a geometry test,
not a physical print or ruler measurement.
