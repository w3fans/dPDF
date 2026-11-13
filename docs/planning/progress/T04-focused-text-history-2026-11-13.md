# T04 focused text and independent document history

Status: IN_PROGRESS. Reproduced on Fedora 45, private authenticated X11, scale 1.
Source-bound runtime: c9153a54197ac8725c30b662f723378e9323672b; executable SHA256
797319200d5341956bb4370e2ccf0e7a9a69a59dc2bea3a22737eb402c87555d.
Reproduction and screenshot hashes: T04-focused-text-reproduction.json.

Create a generated review revision; Escape to reader; Ctrl+Z removes the revision.
Ctrl+F focuses Search, type a generated query, then Ctrl+Y. The revision returns,
the tab becomes dirty and status reports Redid remark edit while Search remains
focused and its text unchanged. This is a reproduced independent-history edit.
Slint 1.18.1 internal/core/input.rs maps text Redo to Ctrl+Shift+Z on Linux and
Ctrl+Y on Windows; the other combination can propagate beyond the text widget.
Source: https://raw.githubusercontent.com/slint-ui/slint/v1.18.1/internal/core/input.rs

Prepared focused correction in isolated /tmp/dpdf-text-field-history, based on
main 30fafd7. Candidate 8ff4332ebda60cd6752189c62a15d2b7ff05ee52,
tree 09a35a4094e7bb3ef7dc6ee47c7eb7997b04a7ad, draft PR #12:
https://github.com/w3fans/dPDF/pull/12. Six file blob hashes match local.

Track workspace text focus, including conditional mark/review/page/print fields,
and stop unhandled keys before document history/navigation handlers. Native text
handling remains first; global captured save/open/search/tab/Escape actions retain
their route. Conditional focus flags are masked by the controls' visibility and
operation, preventing removed controls from reserving reader keys.

Expanded the existing review window diagnostic to retain genuine document Redo,
focus Search through keyboard dispatch, type a character, dispatch both platform
Redo combinations and exercise native text Undo/Redo. It checks the complete
review model, full draft generation and available document Redo; Escape must close
Search and restore reader focus. Existing test harness requires its new milestone.

Local cargo fmt --check, git diff --check, Python AST and CI script contracts pass
(14 cases, 10 passed/4 existing native skips). No local Rust build. New hosted
Rust/PDF/window checks and native corrected replay remain pending. Do not mark
T04 DONE or merge from source checks alone. Engine lifecycle is unchanged; PR #10
retains its separate required review gate. No release/dependency/version change.


## Validated and merged

Every job passed in run 43495909681 for candidate 8ff4332e, including native
Windows hosted checks and complete Linux PDF/window suites. Verified artifact
14072961005 (archive SHA256 3ab5a365807db7abdc48ebed421dda7259cb215b49c84594349656cdfd1cf733)
contains merge source 43d3dc5bce9835dc1c28e27596373e114ca3cda8, executable SHA256
a85939aa202d149a315e8b1f81295b86d0b6625e02223a4b39ff01d1b0b52995.
Replayed original generated native input sequence with checked X window focus.
Ctrl+Y now leaves Revision 1, clean tab, available review Redo and old status
unchanged while Search retains its typed query and focus. Toolbar/review/status
regions have zero changed pixels. Native Ctrl+Z removes search input and Linux
Ctrl+Shift+Z restores it. Source PDF unchanged; processes exited.
Receipt: T04-focused-text-native-replay.json. PR #12 was marked ready and
squash-merged to main as a50144cc03d693ec6810ebcce33113bca026330b after a fresh
all-success check and expected-head merge. T04 overall remains IN_PROGRESS for
the outstanding shortcut/native acceptance scope; no release was published.


## Additional focus and global-shortcut evidence

Native Search and revision-label input retained independent document history at
scales 1/1.5/2; native text Undo/Redo worked, and Escape followed by reader Ctrl+Y
restored the document revision. Exact screenshots/identity:
T04-focused-text-scaled-replay.json. Full native Wayland/Windows remains pending.

Created a private authenticated X display and independent D-Bus session with its
own GTK portal, generated source and isolated XDG/config/data directories. With
Search focused and its query typed, Ctrl+S opened the real Save chooser; Escape
cancelled it without saving/changing source. Ctrl+Shift+S opened it again and
saved a byte-identical generated copy under /tmp. Query survived both global
actions. All owned test processes exited; no personal portal configuration or
security setting changed. Receipt: T04-save-shortcuts-native.json.

The first chooser harness assumed a Save title, but rfd's chooser title was
empty. A later filename expectation also failed because the native chooser
appended its PDF extension. The final harness identifies the chooser by process
and supplies an extensionless location; it produced the expected saved-copy.pdf.
Those earlier harness failures are not reported as application defects or PASS.
