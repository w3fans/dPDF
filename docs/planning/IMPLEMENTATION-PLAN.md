# dPDF implementation plan

Baseline: [13 November audit](CURRENT-STATE-2026-11-13.md). Sequencing and capacity: [schedule](IMPLEMENTATION-SCHEDULE.md). Canonical intent: [product specification](../PRODUCT_SPEC.md). This document plans the repairs and features; tasks start **READY** only when their dependencies pass, otherwise **PENDING**. None is completed by writing this plan.

## Execution rules for a small-context model

1. Work in the active public repository. Read the audit, your task card, the named modules, relevant test script and applicable repository instructions. Check `git status --short` and actual HEAD first; preserve unrelated edits.
2. Take one task or one explicitly named substep. Create a development branch when permitted; never change tags, release versions, published artifacts or private controller installation to make a test pass.
3. Reproduce the behavior using generated fixtures. Record expected versus actual results. For an unconfirmed risk, first establish whether the failure is real; do not invent a fix.
4. Implement the smallest complete change with error, cancellation and stale-result handling. Keep CPU/memory bounds, immutable inputs, atomic export and native-engine ownership.
5. Run the named focused tests, then the required merge gates on approved build infrastructure. A skip/missing binary is PENDING, not PASS. Do not bypass the established remote wrapper with a local compile on this workstation.
6. Update Unreleased changelog and affected requirement/evidence rows. Write the handoff receipt defined in the schedule. Stop when this work package meets its exit condition; a broad refactor is a separate reviewed task.
7. If implementation needs an unproven PDF removal algorithm, engine lifetime change, new dependency/licensing choice or cryptographic design, prepare the reproduction/design and request experienced review. Continue unrelated ready tasks; never weaken a refusal to meet a date.

Estimates below are focused implementer-days including tests and review corrections, excluding waiting for machines, owner review or printers. Split multi-day tasks into the numbered substeps; keep a working, tested intermediate result.

## T01 — Make release-adapter tests usable from public source

Priority P1; dependency none; estimate 1–2 days. Files: `scripts/test-release-version.py`, `scripts/release-version.py`, `release.sh`, `ops/release-policy.json`, `.github/workflows/ci.yml`, `docs/TESTING.md`.

1. Reproduce `PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-release-version.py` in a clean public clone. Preserve the six-error baseline. Inspect controller resolution at the end of `release.sh` and compare the private policy reference in `prepare()` with the tracked policy; do not assume their schemas are interchangeable.
2. Separate public adapter unit tests from installed-controller integration tests. Inject a fake controller/process result at the adapter boundary for unit cases; do not duplicate production version-allocation logic in a fake “real controller”. Use generated fixture files in a temporary directory.
3. Cover mismatch rejection, resume, notes/licenses, failed controller, 0.1.99 ceiling and forbidden stable transition. Make installed-controller testing an explicit integration mode that checks prerequisites and reports unavailable tooling clearly. Never claim it ran when skipped.
4. Add the public suite to CI; document the authority-side integration command. Align policy resolution only after testing compatibility with the installed authority's existing interface.

Exit: public tests work with no private `.agents` files; metadata verification still passes; installed integration has a separate receipt or PENDING status; no release/version/tag mutation. Regression: this suite, `test_pr_policy.py`, metadata verify and relevant release-page/checkpoint tests.

## T02 — Reconcile requirements and evidence

Priority P1; dependency none; estimate 1–2 days. Files: `docs/REQUIREMENTS_MATRIX.md`, `MILESTONES.md`, `LOCAL_HANDOFF_AUDIT.md`, `BUILD-INFRASTRUCTURE.md`, `TESTING.md`, `README.md`.

1. Compare every “current”, “not started”, “pending” and “Remaining sequence” assertion with source, changelog and `RELEASE-STATUS.md`.
2. Record code status separately from platform evidence. Add dated source SHA/platform/scope links, retaining historical qualification without promoting it to current HEAD.
3. Correct compact CUPS printing, Wayland drop implementation, installer delivery, current preview and public/private workflow descriptions. Retain real manual Windows/printer gaps. Distinguish 400-DPI Rust probe constant from older 350-DPI prototype reports.
4. Verify missing requirement rows against the coverage ledger at the end of this plan. Preserve intentional optional deferrals and the reserved stable versions.

Exit: no contradictory current-state claims across these entry documents; no historical PASS rewritten as a new PASS. Check internal links and metadata verify; no app tests needed for prose alone.

## T03 — Reproduce and bound stalled helper operations

Priority P2 reliability; dependency T01 baseline; estimate 3–5 days. Files: `src/assembly.rs`, `encrypted.rs`, `pdf_units.rs`, `review.rs`, `native_print/direct_pdf.rs`, `pdf.rs`; proposed shared helper module only if useful.

1. Inventory qpdf/subprocess calls; identify those on PDF worker versus separate export/print threads. Use a generated fake helper that never exits, another that fills output, and another that exits with a warning.
2. Establish finite deadlines, output byte limits and cancellation policy for noninteractive helpers; do not time out a user-owned print dialog as though it were qpdf. Drain pipes without deadlock; kill and reap children on timeout/cancel.
3. Move expensive preparation off the reader worker only with safe byte-snapshot handoff; PDFium handles remain confined. Tag completions with document/generation if a new asynchronous path can outlive its tab.
4. Test retry, tab closure, app shutdown, source/destination preservation, no password in logs, and ambiguous print submission without resubmission.

Exit: deterministic hanging-helper regression terminates, reader remains usable, resources release, existing successful exports stay unchanged. Run passwords/pages/review/aliases/native-CUPS corpora as affected. If fake-helper tests disprove a suspected path, document why instead of changing it.

## T04 — Finish compact toolbar and keyboard consistency

Priority P2; dependencies T02; estimate 2–3 days. Files: `ui/workspace.slint`, `src/app.rs`, `src/app/ui_smoke.rs`, `scripts/check-tabs.py`, `check-review-ui.py`.

1. Retain the existing Remarks toggle in the wide toolbar and narrow-window More menu (confirmed during T02; F07 overstated this gap). Add active-revision selection there with the established More behavior. Reuse the existing review state/callbacks; do not create a second revision model.
2. Define focus rules for search/page/zoom/review text fields versus document shortcuts, including Enter, Escape, Ctrl+S/Shift+S, Ctrl+Tab and each Undo history.
3. Exercise empty document, loading review, long revision labels, narrow windows, themes and scales 1/1.5/2. Keep primary reader controls reachable.

Exit: R-UI-2 intent met and status context agrees with Review panel; keyboard actions do not unexpectedly edit the wrong text/history. Native visual acceptance stays with T06.

## T05 — Baseline performance and add bounded high-zoom tiles

Priority P2; dependencies T03; estimate 6–10 days. Files: `src/pdf.rs`, `document.rs`, `cache.rs`, `app.rs`; `scripts/benchmark-reader.py`, `benchmark-buffers.py`, `check-viewport.py`, `check-physical-reader.py`.

1. Capture repeated baseline first-page latency, navigation/zoom latency, peak RSS and queue depth for generated 50/300/1000-page, large-image and mixed-geometry documents. Retain private representative-office measurements outside Git. Propose per-machine budgets before optimization; generated historical timings are not universal SLAs.
2. Define tile keys including document, page, generation/scale, rotation and tile coordinates. Build geometry unit cases for crop offsets, intrinsic/view rotations, `/UserUnit`, seams and viewport anchors.
3. Render visible tiles plus bounded overscan at high zoom. Keep existing whole-page/thumbnail path at ordinary zoom; enforce aggregate cache and in-flight budgets. Cancel stale generation work.
4. Validate text/search/mark/review alignment, seams, rapid zoom/tab switching and memory plateau. Compare repeated before/after runs; investigate a proposed >10% median regression on the same fixtures/machine before merge.

Exit: high zoom adds detail without unbounded images/queue growth or misplaced overlays. Numeric budget and measurement method documented; full viewport/physical-reader/reader tests pass. Threshold is a proposed engineering guard, not owner acceptance.

## T06 — Qualify native reader input and clipboard

Priority P1 stable-reader gate; dependencies T04 and applicable T05 changes; estimate 3–5 days plus hardware access. Files: `third_party/winit/.../wayland/uri_list.rs`, existing drop handler, `src/app.rs`, `platform.rs`; scripts `check-file-drop.py`, `check-windows-drop.py`, `check-clipboard.py`, `check-tabs.py`.

1. Run existing generated protocol tests; preserve the already implemented bounded URI/drop parser. Cover multi-file/Unicode, malformed/remote URI refusal, hover cancellation and transfer timeout.
2. On real Fedora Wayland and Windows Explorer, exercise drop/open, focus, clipboard text/image, mixed-DPI monitors, resize, overflow tabs and all shortcuts. Include GNOME with the usual clipboard bridge; separately record bridge-free availability.
3. Fix only reproduced failures. If bridge-free support needs a backend change, make it a separate reviewed compatibility patch with a clear supported-configuration statement; never change compositor security settings to force a pass.

Exit: exact-source desktop receipt for both platforms, with unsupported configurations and any remaining failures visible. Synthetic X11 tests alone cannot close this task.

## T07 — Complete protected-source page workflows and compatibility

Priority P2; dependencies T03; estimate 4–6 days. Files: `src/assembly.rs`, `pages.rs`, `image_pages.rs`, `page_bookmarks.rs`, `encrypted.rs`, `app.rs`; `check-pages.py`, `check-passwords.py`, `check-aliases.py`.

1. Add a per-operation input queue for protected secondary PDFs: masked prompt, wrong-password retry, cancel entire operation, bounded snapshots, no password persistence or arguments. Reuse existing password validation and opened-snapshot semantics.
2. Cover reorder/replace/merge/extract with bookmarks, links/named destinations, annotations, page labels, forms and attachments. Define which metadata survives each operation; either preserve safely or refuse clearly. General destination-tree repair is not established by the current bookmark helper.
3. Cover EXIF phone rotation, transparent PNG, large/invalid images, source alias changes and mixed page units. Verify page count/order and unchanged pages 1–9 streams/render after page-10 replacement.

Exit: secondary protected inputs no longer need the workaround; cancellation/failure leaves input and pre-existing output intact; compatibility/refusal contract and Scenario B fixtures pass. Optional multi-step page undo is not required.

## T08 — Measure and improve Windows compact printing

Priority P2; dependencies T03; estimate 4–7 days including investigation. Files: `src/windows_print.rs`, `platform.rs`, `print_plan.rs`, `scripts/check-windows-print.py`, `test-print-benchmark.py`.

1. Measure current 50-page and page-7 GDI output/spool size, submission and first-page latency on native Windows with exact driver/printer identities.
2. Investigate supported driver/spool capabilities using current official platform documentation. Select an optimization only when it preserves driver settings and output. Never send raw PDF to an arbitrary Windows queue assuming PDF support; retain qualified GDI fallback.
3. Implement capability-gated compact/vector submission if feasible; otherwise optimize the measured raster bottleneck and record the limitation. Preserve current/all/disjoint pages, copies, fit/actual scale, cancellation, Print-to-PDF alias protection and one submission per action.

Exit: measured improvement with output equivalence and no regression to native dialog behavior, or a reviewed feasibility result with remaining requirement explicitly open. A benchmark alone does not satisfy physical printing.

## T09 — Qualify physical printing and scale

Priority P1 stable-reader gate; dependencies T08 and relevant T03/T05 changes; estimate 2–4 active days plus access. Files: native print adapters, `print_plan.rs`, validation documents.

1. Use a real office printer on Fedora and Windows, including the slow PostScript path if available. Record driver, connection, paper, source/binary hash, job bytes, submission time and time to first/last sheet.
2. Print all/current/disjoint ranges, copies, portrait/landscape, fit/actual, rotated/cropped pages, `/UserUnit` 0.5/1/2, cancellation and printer unavailable. Print 40-mm generated marks and measure with a ruler; propose tolerance before judging results.
3. Verify content, ordering, clipping, page count and no duplicate job after failure. Compare compact and fallback paths where available; retain print-to-file automation as separate evidence.

Exit: signed-off physical output receipt on both platforms; Scenario A page-7 print succeeds. Do not infer a PASS from spool submission or a virtual printer.

## T10 — Qualify mark studio, sizing, recovery and external viewers

Priority P2 v0.4 gate; dependencies T06–T07, T09 for ruler output; estimate 3–5 days. Files: `marks.rs`, `mark_studio.rs`, `placement_ui.rs`, `overlay.rs`, `recovery.rs`, `storage.rs`.

1. Run marks, overlays, aliases, clipboard-placement and recovery suites, including the existing UserUnit corpus. Use synthetic colored stamps/ink, not personal signatures in fixtures.
2. Perform Scenario C with image import/paste, crop/deskew/cleanup, preferred size, drag/numeric placement, opacity, undo/redo, save and reopen. Confirm transparent edges and printed dimensions.
3. Interrupt a mixed draft, change/delete its input, restart and recover; check secondary instance, full store, disabling recovery, delete-all and failed saves. No newer edit may be cleared by an older export completion.
4. Open/export/print in two independent common viewers across platforms; record exact application versions. Keep private real-image acceptance evidence outside the repo.

Exit: B/C acceptance pack and preserved source hashes; known viewer limitations documented, current Windows recovery exercised.

## T11 — Qualify review persistence and independent export

Priority P2 v0.6 gate; dependencies T06, T10; estimate 3–5 days. Files: `src/review.rs`, `app.rs`, `recovery.rs`; `check-review.py`, `check-review-ui.py`, `check-poppler.py`.

1. Execute Scenario E: empty/new revisions, all/current/selected filters, visibility toggle, notes/highlights/underlines, re-text/delete, separate Undo/Redo, mixed and remarks-only recovery.
2. Round-trip native annotations through two independent viewers; edit/move/delete comments and preserve foreign annotations, replies/popups and empty revision sets. Duplicate identifiers must fail visibly, not lose comments.
3. Check all/current/hidden/clean exports and printing, including source unchanged, clean export retaining drafts and stale export preserving newer edits. Repeat native Windows pointer/text input.

Exit: source-bound interop matrix and no silent metadata/comment loss. Bake independence remains open until T18; optional callouts/flattening are T22.

## T12 — Specify production secure-region contract

Priority P1 v0.6 prerequisite; dependencies T02; estimate 3–5 days, experienced PDF reviewer required. Files: `docs/ARCHITECTURE.md`, `src/block_probe.rs`, existing block-probe reports.

1. Define supported PDF subset, geometry units, region/margin semantics, object boundary intersections, visual signature threat model and recoverable failure behavior. A baked graphic offers casual-removal resistance, not cryptographic authenticity or general confidential-data redaction.
2. Inventory live/retained content routes: shared streams/resources, forms, clips/masks, marked content/ActualText, layers, annotations, attachments, catalog references, prior revisions, encryption and digital signatures. Choose support or refusal for each.
3. Define a read-only preflight result with reason codes and an immutable validated plan bound to source hash, page identity, geometry and draft generation. Unsupported/ambiguous structures must fail before destination replacement.

Exit: reviewed design, explicit refusal table and independent verification strategy. If true bounded-region removal cannot be established, keep production disabled and revise schedule; covering content or flattening the entire page violates requirements.

## T13 — Expand adversarial removal/preflight fixtures

Priority P1; dependency T12; estimate 5–8 days. Files: `block_probe.rs`, `scripts/check-block-rust.py`, block fixture generators and new focused backend tests.

1. Preserve existing generated 32 geometry cases and refusal identities. Add fixtures for each T12 graph/content route, crossing paths/text glyphs, inherited properties, malformed/cyclic references, huge graphs, page boxes/rotation and custom physical units.
2. Establish retained-reference traversal, bounded depth/object/byte limits and ownership rules. Detect aliases outside the modified region/page, including streams retained by metadata or catalog links.
3. For accepted fixtures, verify no removable independent signature or live replaced label remains through independent text/object inspection and rendering; compare outside region and untouched page streams/pixels. For refusals, assert source and existing output hashes unchanged and no published partial output.

Exit: reviewed positive/negative corpus and a justified supported subset. Passing generated fixtures is necessary, not proof of arbitrary-document support; preserve the explicit boundary.

## T14 — Implement production bake export backend

Priority P1; dependency T13 approved; estimate 5–8 days. Proposed `src/secure_block.rs` plus `pdf.rs`, `assembly.rs`, `storage.rs`, `pdf_units.rs`; keep diagnostic probe separate.

1. Consume immutable source/mark bytes and the validated plan. Use the established worker's engine or an isolated bounded child; never reuse probe global init/destroy inside the reader process.
2. Render original region and selected marks at 300–400 physical DPI; account for crop/rotation/UserUnit and alpha. Remove validated live objects/content, insert one combined graphic and preserve outside content. Partial-overlap structures require a proven implementation or refusal.
3. Serialize a fresh candidate without retaining removed content in old incremental revisions; validate structure, output size, independent checks and source identity before atomic publication. Handle disk full, warning exit, cancellation and alias destinations.
4. Route completion with document/generation identity so newer drafts survive. Leave region/mark draft unchanged on failure.

Exit: production backend corpus passes and technical review approves removal/outside preservation. Keep UI feature unavailable until T18 passes its product gate; backend tests do not authorize a secure claim for unsupported inputs.

## T15 — Add region editor, margins and anchors

Priority P1; dependencies T12, T14; estimate 4–6 days. Files: `ui/workspace.slint`, `app.rs`, `document.rs`, `placement_ui.rs`; proposed region model/controller module.

1. Implement per-document/page rectangle state in normalized PDF coordinates plus physical mm dimensions; add drag-to-create, move, corner resize, margins and bottom-right/bottom-left/centre-right anchors.
2. Preview the effective bake bounds and included marks. Clamp/validate bounds; refuse invalid units, zero area or unsafe overlap rather than silently clipping meaningful content.
3. Integrate Escape/cancel and coherent Undo/Redo, tab changes, rotation, zoom and draft generation. Region controls must not intercept normal selection/review interactions when inactive.

Exit: deterministic geometry/input tests at three scales and all rotations; region sent to backend matches preview and physical output. Recovery changes follow T16.

## T16 — Persist templates and recover bake drafts

Priority P1; dependency T15; estimate 3–5 days. Files: proposed template schema/store, `preferences.rs`, `recovery.rs`, region model/UI.

1. Define a versioned local template: name, anchor, width/height/margin in mm, mark inclusion, export mode; reserve future date/name fields without implementing speculative automation.
2. Add create/select/rename/delete with atomic persistence, size bounds, duplicate handling and preservation of corrupt/future schemas. PZB/IDZ-style names are examples, not user data to fabricate.
3. Extend recovery schema compatibly for region/template selection and draft generation. Round-trip old drafts and refuse unsupported newer versions without deletion. Never persist passwords or digital-seal keys.

Exit: template portability across page sizes with correct anchor/mm mapping; recovery restores preview accurately; delete/cancel/failure cannot damage the library or older checkpoints.

## T17 — Integrate export modes and honest warnings

Priority P1; dependencies T14–T16; estimate 2–3 days. Files: `ui/workspace.slint`, `app.rs`, `overlay.rs`, secure export module, `README.md`.

1. Expose Simple Overlay and Secure Baked Region only according to capability/preflight. Keep Digital Seal unavailable until its real implementation is qualified; a planned interface must not appear as a working mode.
2. Before baking, explain flattened region/searchability loss and existing digital-signature invalidation. Show the affected region and exact refusal reason with safe alternatives. Protect sources and warn for unencrypted output from protected inputs.
3. Keep simple-overlay behavior, existing marks, review filters and cancellation unchanged. Use secure mode as recommended default only for supported, qualified inputs after T18.

Exit: explicit consent/cancel paths tested; no UI or documentation describes covering pixels as content removal, or visual marks as verified digital signatures.

## T18 — Integrate review independence and qualify Scenario D

Priority P1 v0.6 gate; dependencies T11, T13–T17; estimate 4–7 days plus reviewer access. Files: secure export, `review.rs`, integration corpora and validation receipts.

1. Resolve the current prototype's all-annotations refusal with the reviewed annotation policy. Preserve native review annotations independently; do not bake their visibility into signature pixels. Refuse unsupported annotation combinations explicitly.
2. Bake overlapping generated stamp/signature/printed label/line with margin; inspect through PDFium, independent renderer/text extractor and common editor. Verify no separable live signature/label objects, searchable outside text, untouched pages and unchanged review visibility/filter behavior.
3. Repeat rotations/crops/UserUnit, encrypted inputs, template recovery, cancel/save failure and Windows/Fedora native flows. Physical print checks confirm edge quality and size.
4. Obtain independent PDF security review of the supported subset and owner D/E acceptance. Attach exact source, fixture/output hashes and refusal coverage.

Exit: only the reviewed supported subset can be enabled. Broad unsupported contracts remain refused; inability to meet D blocks v0.6, not unrelated qualified previews.

## T19 — Dependency, packaging and upgrade maintenance

Priority P2; dependency T02; estimate 3–5 days. Files: `Cargo.toml`, lockfile, `third_party/winit`, staging/packaging scripts, licensing docs, CI pin configuration.

1. Inventory runtime/binding/compiler/installer pins and vendored patch provenance. Check current official advisories and upstream compatibility when executing; record dates and sources. Do not infer security from a pin alone.
2. Document update owner, supported versions and test matrix. If a dependency update is warranted, perform it as a separate change with the full geometry/interop/native regression gate.
3. Validate packaged runtime discovery, notices, fresh install/upgrade/uninstall, Unicode paths, Open With, default-app preservation and user data retention. Keep installer signing separate from basic package correctness; no certificate purchase or secret provisioning implied.

Exit: reproducible exact-source package qualification and update procedure. Record Windows code-signing limitation if still unsigned. Do not replace the existing publisher/controller.

## T20 — Milestone acceptance and release handoff

Priority P1; dependencies depend on milestone; estimate 2–4 active days per acceptance pass, repeats budgeted separately.

1. v0.2: T01–T06, T08–T09, applicable T19 checks and Scenario A acceptance; unresolved correctness failures block. Classify feature enhancements separately from existing mandatory gates; any deliberate deferral requires recorded rationale.
2. v0.4: add T07/T10 and B/C acceptance, preserved untouched pages, safe saves and external viewers.
3. v0.6: add T11–T18 and D/E acceptance, plus T21 interface planning. Optional full cryptographic signing is not required by the current milestone wording.
4. Re-run merge/release checks for the actual candidate. Record all FAIL/PENDING/SKIP states, artifact identity and owner acceptance. Hand off to the sole release authority's existing workflow; stable-version support needs a deliberate tooling review because the current adapter accepts only 0.1.x.

Exit: reviewed milestone acceptance record, not an automatically created tag. Qualified 0.1.x previews may continue between gates; no 0.1.100 and no version changes for this planning work.

## T21 — Plan digital seal boundary; optional implementation later

Priority P3 (interface planning required for v0.6); dependency T12; planning 1–2 days. Proposed seal design document/interface; no private-key handling in current baking module.

Define an optional post-bake signing boundary over final bytes with explicit result/cancel/failure, certificate selection, validation and trust reporting. Preserve unsigned output on signing failure according to an explicit user choice; never call it sealed. Document that a visual signature differs from cryptographic integrity and that certificate/recipient validation matters.

Full implementation needs separate cryptography/platform review, certificate store decisions, signing/timestamp format, mutation ordering and independent validator tests. Estimate only after that design; it is not bundled into a small model's region-editor task.

## T22 — Optional enhancements queue

Priority P3; dependency relevant parent feature accepted; not on the committed schedule.

| Option | First bounded task | Acceptance before exposing it |
| --- | --- | --- |
| Direct email attachment | Investigate native attachment APIs per platform; retain saved-file fallback | Verified selected saved file attached in supported clients, explicit action, no automatic sending |
| Camera/photo capture | Platform capability and permissions design; reuse image import | Cancel/error paths, orientation, memory limits and offline source retention |
| Review callouts | Extend native annotation model and independent viewer fixtures | Round-trip without breaking existing note/highlight/underline metadata |
| Flatten selected remarks | Explicit export-only policy independent of signatures | Chosen remarks flattened; others preserved; source and draft unchanged |
| Rich image page crop/size | Reuse studio geometry for page import with explicit physical size | Crop preview matches exported page and does not stretch scans |
| Multi-operation page undo | Design an immutable composable page-edit plan/history first | Undo/redo and recovery preserve source identities and metadata |

These require explicit scope selection and estimates after core acceptance; keep them visible as deferred, not “implemented”.

## Verification commands and interpretation

From the active source root, these are safe source-only checks:

```sh
cargo fmt --check
python3 scripts/release-version.py verify
PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-release-version.py
PYTHONDONTWRITEBYTECODE=1 python3 scripts/ci/test_pr_policy.py
```

The third command is expected to fail at the audited baseline until T01. Run the remaining 14-suite command list from `.github/workflows/ci.yml` for tooling changes; individual commands/results are also in the audit JSON. Avoid bare unittest discovery: hyphenated filenames and CI subdirectories need explicit invocation.

On configured approved build infrastructure, resolve the existing Cargo wrapper first, then use the project's standard test/build/Clippy commands. This workstation's unconfigured `/usr/bin/cargo` is not proof of remote routing. Typical gates are `cargo test --locked --release`, strict Clippy both default and `check-windows-print`, and `./build --release` through that wrapper. Native Windows uses `build.ps1` and its documented validation harness. Do not run `release.sh patch` merely to obtain a test build.

With a source-bound binary and validated helper paths, the existing Linux orchestrator is:

```sh
python3 scripts/ci/validate_linux.py --source . \
  --commit "$(git rev-parse HEAD)" --ref local \
  --binary "$DPDF_TEST_BINARY" --pdfium "$DPDF_PDFIUM_LIBRARY" \
  --qpdf "$DPDF_QPDF_BINARY" --xvfb "$DPDF_TEST_XVFB" \
  --output "$DPDF_TEST_OUTPUT"
```

Set those task-specific variables to actual validated files/output directories; they are placeholders, not defaults. Run in a clean candidate checkout for a durable acceptance receipt. Do not pair a new HEAD with a stale binary. Inspect each script's `--help` before running a narrower corpus. `--no-desktop` or `--only` produces limited evidence; do not label it full qualification. Native CUPS, real Wayland, Windows, physical printers and manual scenarios have additional setup/procedures in `TESTING.md` and are not all covered by this single command.

## Requirement coverage ledger

The table below maps every canonical requirement ID to remaining work or retention/qualification. “Retain” means implemented behavior must survive the assigned task; it is not a new feature request. Optional entries remain optional. Cross-cutting source integrity applies to every write path.

| Requirement | Work package | Disposition |
| --- | --- | --- |
| R-PLAT-1 | T19; T20 | Retain architecture/runtime pins; qualify packages on both platforms |
| R-PLAT-2 | T04; T15 | Retain Rust/Slint native architecture; no framework replacement |
| R-PLAT-3 | T19; T20 | Retain architecture/runtime pins; qualify packages on both platforms |
| R-PLAT-4 | T19; T20 | Retain architecture/runtime pins; qualify packages on both platforms |
| R-PLAT-5 | T10 | Retain Rust image cleanup and test colored ink/transparency |
| R-PLAT-6 | T19; T20 | Retain architecture/runtime pins; qualify packages on both platforms |
| R-PLAT-7 | T19; T20 | Retain architecture/runtime pins; qualify packages on both platforms |
| R-UI-1 | T04; T06 | Retain implemented UI; finish toolbar intent and native input qualification |
| R-UI-2 | T04; T06 | Retain implemented UI; finish toolbar intent and native input qualification |
| R-UI-3 | T04; T06 | Retain implemented UI; finish toolbar intent and native input qualification |
| R-UI-4 | T04; T06 | Retain implemented UI; finish toolbar intent and native input qualification |
| R-UI-5 | T04; T06 | Retain implemented UI; finish toolbar intent and native input qualification |
| R-UI-6 | T04; T06 | Retain implemented UI; finish toolbar intent and native input qualification |
| R-READ-1 | T05; T06; T20 | Retain reader behavior; measure quality/performance and qualify native input |
| R-READ-2 | T05; T06; T20 | Retain reader behavior; measure quality/performance and qualify native input |
| R-READ-3 | T05; T06; T20 | Retain reader behavior; measure quality/performance and qualify native input |
| R-READ-4 | T05; T06; T20 | Retain reader behavior; measure quality/performance and qualify native input |
| R-READ-5 | T05; T06; T20 | Retain reader behavior; measure quality/performance and qualify native input |
| R-READ-6 | T05; T06; T20 | Retain reader behavior; measure quality/performance and qualify native input |
| R-READ-7 | T05; T06; T20 | Retain reader behavior; measure quality/performance and qualify native input |
| R-READ-8 | T05; T06; T20 | Retain reader behavior; measure quality/performance and qualify native input |
| R-READ-9 | T05; T06; T20 | Retain reader behavior; measure quality/performance and qualify native input |
| R-READ-10 | T05; T06; T20 | Retain reader behavior; measure quality/performance and qualify native input |
| R-READ-11 | T05; T06; T20 | Retain reader behavior; measure quality/performance and qualify native input |
| R-READ-12 | T04; T06 | Retain light/dark themes; same intent as R-UI-5 |
| R-READ-13 | T05; T06; T20 | Retain reader behavior; measure quality/performance and qualify native input |
| R-PRN-1 | T03; T08; T09; T20 | Retain native printing; measure compact path and qualify physical output |
| R-PRN-2 | T03; T08; T09; T20 | Retain native printing; measure compact path and qualify physical output |
| R-PRN-3 | T03; T08; T09; T20 | Retain native printing; measure compact path and qualify physical output |
| R-PRN-4 | T03; T08; T09; T20 | Retain native printing; measure compact path and qualify physical output |
| R-PRN-5 | T03; T08; T09; T20 | Retain native printing; measure compact path and qualify physical output |
| R-PRN-6 | T03; T08; T09; T20 | Retain native printing; measure compact path and qualify physical output |
| R-PAGE-1 | T07; T10; T20 | Retain operations; protected secondary inputs and metadata/Scenario B qualification |
| R-PAGE-2 | T07; T10; T20 | Retain operations; protected secondary inputs and metadata/Scenario B qualification |
| R-PAGE-3 | T07; T10; T20 | Retain operations; protected secondary inputs and metadata/Scenario B qualification |
| R-PAGE-4 | T07; T10; T20 | Retain operations; protected secondary inputs and metadata/Scenario B qualification |
| R-PAGE-5 | T07; T10; T20 | Retain operations; protected secondary inputs and metadata/Scenario B qualification |
| R-PAGE-6 | T07; T10; T20 | Retain operations; protected secondary inputs and metadata/Scenario B qualification |
| R-PAGE-7 | T07; T10; T20 | Retain operations; protected secondary inputs and metadata/Scenario B qualification |
| R-PAGE-8 | T07; T10; T20 | Retain operations; protected secondary inputs and metadata/Scenario B qualification |
| R-SIG-1 | T10; T20 | Retain studio/overlay functionality; Scenario C and interoperability qualification |
| R-SIG-2 | T10; T20 | Retain studio/overlay functionality; Scenario C and interoperability qualification |
| R-SIG-3 | T10; T20 | Retain studio/overlay functionality; Scenario C and interoperability qualification |
| R-SIG-4 | T10; T20 | Retain studio/overlay functionality; Scenario C and interoperability qualification |
| R-SIG-5 | T10; T20 | Retain studio/overlay functionality; Scenario C and interoperability qualification |
| R-SIG-6 | T10; T20 | Retain studio/overlay functionality; Scenario C and interoperability qualification |
| R-SIG-7 | T10; T20 | Retain studio/overlay functionality; Scenario C and interoperability qualification |
| R-SSB-1 | T12–T18; T20 | Implement missing production secure-region workflow after removal review |
| R-SSB-2 | T12–T18; T20 | Implement missing production secure-region workflow after removal review |
| R-SSB-3 | T12–T18; T20 | Implement missing production secure-region workflow after removal review |
| R-SSB-4 | T15 | Implement region drag/move/resize, physical margin and anchors |
| R-SSB-5 | T12–T18; T20 | Implement missing production secure-region workflow after removal review |
| R-SSB-6 | T16 | Implement local versioned templates and recovery |
| R-SSB-7 | T12–T18; T20 | Implement missing production secure-region workflow after removal review |
| R-SSB-8 | T17 | Implement bake/searchability and signature-invalidation warnings |
| R-SEAL-1 | T21 | Interface planning; full cryptographic implementation explicitly optional/later |
| R-SEAL-2 | T21 | Interface planning; full cryptographic implementation explicitly optional/later |
| R-REM-1 | T11; T18; T20 | Retain review; external round-trip and independence from new baking |
| R-REM-2 | T04; T11; T18 | Expose toolbar toggle; preserve independent review visibility |
| R-REM-3 | T11; T18; T20 | Retain review; external round-trip and independence from new baking |
| R-REM-4 | T11; T18; T20 | Retain review; external round-trip and independence from new baking |
| R-REM-5 | T11; T18; T20 | Retain review; external round-trip and independence from new baking |
| R-REM-6 | T11; T18; T20 | Retain review; external round-trip and independence from new baking |
| R-SHR-1 | T22 | Conditional reliable email attachment deferred; existing R-SHR-2 fallback retained |
| R-SHR-2 | T06; T10 | Retain copy path/reveal saved file; refuse unsaved-work misrepresentation |
| R-PERF-1 | T03; T05; T08 | Measure and bound helper/render/print work; preserve untouched pages |
| R-PERF-2 | T03; T05; T08 | Measure and bound helper/render/print work; preserve untouched pages |
| R-PERF-3 | T03; T05; T08 | Measure and bound helper/render/print work; preserve untouched pages |
| R-PERF-4 | T07; T13; T14 | Verify untouched streams and rendering during assembly/baking |
| R-INT-1 | T03; T07; T10; T14; T16–T18 | Preserve source/atomicity/recovery and warnings in every edit path |
| R-INT-2 | T03; T07; T10; T14; T16–T18 | Preserve source/atomicity/recovery and warnings in every edit path |
| R-INT-3 | T03; T07; T10; T14; T16–T18 | Preserve source/atomicity/recovery and warnings in every edit path |
| R-INT-4 | T03; T07; T10; T14; T16–T18 | Preserve source/atomicity/recovery and warnings in every edit path |
| R-INT-5 | T06; T10; T18 | Retain byte-identical Save As and non-mutating view operations |
