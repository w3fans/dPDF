# T02 — Reconcile requirements and evidence

Status: DONE for documentation reconciliation; no platform acceptance promoted.
Baseline: `268b234e4336789922a6b15dd1236963e131d707`.
Branch: `fix/public-release-adapter-tests` (continuing the existing uncommitted
T01/Astra handoff work; no unrelated edits discarded).

Compared six entry documents with source, changelog, RELEASE-STATUS and the
canonical product requirement IDs. Corrected the current published version/source,
kept historical check marks scoped, recorded compact CUPS and Wayland
implementation, Windows MSI/portable qualification and outstanding manual gates.
Separated public publication roles from private/fallback instructions and labelled
the old local handoff snapshot. Replaced the already-implemented page/mark
sequence with remaining qualification. Distinguished the historical 350-DPI
Python prototype from the current 400-DPI Rust probe. Added R-PLAT-5, R-READ-12
and R-PERF-4; spelled out the grouped R-PERF-2 and R-SEAL-2 IDs.

Correction to audit F07: source already has toolbar/More Remarks toggles. Updated
the matrix and T04 task wording and appended a correction to the historical audit.
Revision selection remains in Review panel. No UI code changed in T02.

Checks on 2026-11-13:

- Local Markdown file targets in README and the five changed entry docs: all exist.
- Canonical PRODUCT_SPEC IDs compared with matrix IDs: all present.
- `python3 scripts/release-version.py verify`: PASS v0.1.54.
- `git diff --check`: PASS.
- Application tests not required/run for prose. External delivery assertions are
  attributed to RELEASE-STATUS, not freshly queried network state.

Next exact action: T04 active-revision selector in `ui/workspace.slint` wide
toolbar/More, retaining existing choose-revision callback. First establish the
approved remote compiler route, then inspect focus handling and existing UI
smoke coverage. T01 authority integration/CI review is still pending separately.
Rollback: revert focused documentation reconciliation, preserving Astra's plan,
T01 code and evidence, and all release artifacts.


## Later source reconciliation

The receipt above records the original T02 pass at 268b234e. Subsequent development
merged toolbar selectors (#4), history isolation (#9/#12) and tab field focus
(#13), so the canonical handoff/matrix/testing text was checked again against
main de7a53d. It now points to the scoped T04 exit and corrected native replay,
without granting real Wayland/Windows or owner acceptance. The original T02
next-action paragraph is historical, not a current instruction to repeat T04.
All canonical documents and supporting planning/evidence still await their
focused documentation PR; local completion is not a claim of publication.
