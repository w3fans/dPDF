# dPDF current-state audit — 13 November 2026

Start with this audit, then follow [the implementation work packages](IMPLEMENTATION-PLAN.md) and [the schedule and handoff rules](IMPLEMENTATION-SCHEDULE.md).

## Baseline and scope

The workspace startup note selects `public-source` as the active public checkout. The adjacent development, recovery and publication worktrees are historical or operational inputs, not interchangeable sources. Audited HEAD: `268b234e4336789922a6b15dd1236963e131d707`; Cargo package and release metadata: **0.1.54**. The checkout was clean before this documentation work.

This is a repository audit and implementation handoff, not a completed feature implementation, penetration test or new platform qualification. It combines product-specification coverage, inspection of core execution/save/print/prototype paths, build/workflow inspection and fresh source-only tests. No external issue tracker, release service or upstream dependency advisory was queried. Remote delivery statements below describe repository records, not fresh network verification. Unknown bugs may remain; the plan includes reproduction and triage gates rather than inventing a complete list of runtime defects.

The product specification and its A–E scenarios define scope. Optional features are explicitly separated from milestone requirements. Do not interpret every historical “remaining” line as missing code.

## Fresh checks

Machine-readable results: [audit-checks-2026-11-13.json](audit-checks-2026-11-13.json).

| Check | Observation |
| --- | --- |
| `cargo fmt --check` | PASS |
| `python3 scripts/release-version.py verify` | PASS, v0.1.54 |
| All 15 `scripts/test-*.py` and `scripts/ci/test_*.py` suites | 111 cases: 101 passed, four skipped, six setup errors |
| 14 suites other than release-version | Exit 0; the four skips are in `scripts/ci/test_ci.py` |
| `scripts/test-release-version.py` | Exit 1: all six tests require absent private controller files |
| Rust compile/test/Clippy and PDF/UI corpora | NOT RUN: no current executable or configured remote Cargo wrapper in this checkout |
| Fedora/Windows interactive input, physical printers, owner scenarios | NOT RUN |

The public CI script-contract job lists the 14 passing suites and omits `test-release-version.py`; its separate metadata verification passes. Thus the observed failure does not establish that public CI is currently red. On the release authority, privately installed tooling may satisfy those tests. The reproducible defect is a public-source test portability/documentation gap, with a potential release integration mismatch to investigate.

Raw audit logs are local scratch under `/tmp/dpdf-audit-20261113`; the committed JSON preserves the commands, case counts, exits and failure cause without private environment contents.

## Architecture and constraints

| Area | Current design | Implication for implementers |
| --- | --- | --- |
| UI/controller | `src/app.rs` (3,756 lines), `ui/workspace.slint` (762); diagnostic helpers under `src/app/` | Add small cohesive modules; avoid a whole-controller rewrite alongside a feature |
| PDF engine | `src/pdf.rs`: one engine-owning worker, command capacity 64 and response capacity eight | Never send live PDFium handles across threads or initialize the probe's engine beside live reader documents |
| Rendering | `src/document.rs`, `src/cache.rs`: virtualized layout, 64 MiB frame LRU, 3072×4096 raster limit | Cache limit excludes document snapshots, engine memory, UI copies and responses; high zoom magnifies capped images |
| Search | `src/app/search_queue.rs`: one newest pending command, retry on full queue, token/tab checks | Queue-loss defect already fixed; preserve cancellation and backpressure tests |
| Source integrity | Immutable opened bytes; `src/storage.rs` alias checks and sibling temporary-file replacement | Save As remains byte-identical; never solve an export failure by overwriting a protected input |
| Page edits | `src/pages.rs`, `assembly.rs`, `image_pages.rs`, `page_bookmarks.rs`; qpdf snapshot assembly | Exports are separate copies; exact untouched streams/pixels matter, not identical whole-file serialization |
| Marks | `marks.rs`, `mark_studio.rs`, `placement_ui.rs`, `overlay.rs` | Library, cleanup, mm sizing, placement and overlay export already exist |
| Review | `review.rs`, UI integration; native annotations plus dPDF revision metadata | Preserve foreign annotations, threads, filters and separate mark/review histories |
| Recovery | `recovery.rs`: bounded drafts, single owner lock, idle checkpoints, privacy controls | 128 MiB PDF, 64 MiB mark images, 512 MiB store; no passwords in persisted metadata |
| Printing | Native GTK/CUPS and Windows dialog/GDI child processes | Linux eligible queues receive compact PDFs; other paths may rasterize; no duplicate retry after ambiguous submission |
| Secure baking | `block_probe.rs`: generated-only CLI, fixed region geometry, own raw engine lifecycle | Prototype is not a production backend; output explicitly declares secure export unqualified/disabled |
| Delivery | Public workflows plus designated local Fedora publisher; exact-source artifact receipts | Do not rebuild/relabel an older artifact or create another release writer |

## What actually exists

Reader functionality includes tabs, three layouts, navigation/bookmarks, search, text selection, themes, editable zoom, view rotation, protected-PDF prompts, unchanged Save As, native printing and saved-file sharing. Page operations cover insert/remove/reorder/extract/merge/replace/rotate, scans and physical page-unit handling. Marks include image import/cleanup, local library, physical sizing, editing history and recovery. Review includes revisions, notes/highlights/underlines, filters, native persistence, clean export and mixed recovery.

The changelog records the search retry fix in 0.1.50, native Wayland file drops in 0.1.51, compact narrow-window controls in 0.1.52 and readable invalid-PDF errors in 0.1.53. Current source contains these paths. They require regression/desktop qualification, not duplicate implementations. `/UserUnit` size bugs are recorded as fixed in earlier releases.

`docs/RELEASE-STATUS.md` records 0.1.54 Linux/Windows hosted validation, Windows installer/portable delivery, Fedora 44/45 installed qualification and native Wayland smoke. Its source SHA is `e06b05b5f1e91e9e9e99b97967842bad4996bc67`, not audited HEAD. Do not silently transfer that evidence to new code. Manual Windows desktop, physical printers, printed rulers and owner acceptance remain open in that record.

## Findings ranked for action

| ID | Classification / priority | Evidence and consequence | Work package |
| --- | --- | --- | --- |
| F01 | Reproduced tooling error / P1 | Release-version tests copy `.agents/bin/release-controller` and `.agents/release-policy.yaml`; absent in public source, six setup failures. `release-version.py:prepare` also refers to private policy; inspect alignment with `release.sh` and tracked `ops/release-policy.json`. | T01 |
| F02 | Confirmed documentation drift / P1 | Requirements matrix calls 0.1.49 current; handoff calls compact print “not started” and qualified installers absent, while current matrix/release record says otherwise. Milestones retain private-workflow language and implemented page-operation steps under “Remaining sequence”. | T02 |
| F03 | Missing product feature / P1 for v0.6 | Production Secure Signature Block, region editor, templates and bake warnings absent; probe refuses annotations and unsupported graphics/reference structures. | T12–T18 |
| F04 | Acceptance gap / P1 for stable reader | No current physical-printer, owner Scenario A or native manual Windows acceptance established by audited records. Hosted automation cannot substitute. | T06, T09, T20 |
| F05 | Implemented limitation / P2 | High zoom enlarges bounded page bitmap (`pdf.rs:render`, UI request width clamp); no tile renderer. | T05 |
| F06 | Partially delivered requirement / P2 | Compact CUPS PDF jobs exist; Windows uses raster GDI and needs measured optimization. Office-printer latency unqualified on both. | T08–T09 |
| F07 | Confirmed scope discrepancy / P2 | Required main-toolbar remarks toggle/revision selector live in Review panel. Optional library sidebar need not be implemented. | T04 |
| F08 | Known workflow limitation / P2 | Additional password-protected page-operation sources lack prompts; current README supplies separate-open/extract workaround. | T07 |
| F09 | Integration qualification gap / P2 | External viewer review round-trip, annotations/forms/links after assembly, scanner images, printed dimensions and mixed recovery need current coverage. | T07, T10–T11 |
| F10 | Static robustness risk, not reproduced / P2 | Several qpdf calls use blocking `.output()`/`.status()`; decryption reads/waits without a deadline. A stalled helper may indefinitely occupy work, including the PDF worker. | T03 |
| F11 | Documented platform limitation / P2 | GNOME clipboard backend needs existing Xwayland bridge in recorded environment; bridge-free Wayland support is not established. | T06 |
| F12 | Maintenance gap / P2 | Pinned PDFium 7881 and vendored Winit patch need documented advisory/update ownership and upgrade qualification. No claim here that either has a known current vulnerability. | T19 |

No newly reproduced application crash, data-loss bug or rendering failure is claimed. Risks from source review must get a deterministic failing test before being reported as confirmed runtime defects.

## Missing versus optional

Required for remaining milestones: current reader/printing acceptance; practical page/signature interoperability and scenarios B/C; production secure content replacement with conservative refusal; region geometry, templates, warnings and review independence; scenarios D/E. Performance work must preserve the product's speed-first priorities.

Optional/later: digital-seal implementation (interface planning is in v0.6 scope), camera import, callouts, flattened remarks, direct email attachment where reliable, richer image-page editing and multi-operation page undo. Existing copy-path/reveal fallback satisfies the practical sharing route. OCR, arbitrary PDF text editing, cloud sync and mobile remain out of scope. Do not quietly turn optional features into stable-release blockers.

## Completion standard

The app is a substantial development preview, not an empty scaffold and not a qualified stable milestone. Finish the narrow public-source tooling repair, reconcile evidence, establish repeatable baselines, then implement and qualify the missing workflows. Security-sensitive baking requires explicit technical review and negative fixtures before product exposure. Each remaining task and requirement is assigned in the plan; dates are estimates conditioned on these gates.

## Implementation-time correction — 2026-11-13

T02 source inspection found that F07 overstates the toolbar gap: the Remarks
checkbox already exists in the wide toolbar and narrow More popup in
`ui/workspace.slint`. Only the active-revision selector is confined to the Review
panel. T04 must retain that toggle and add the selector using existing state,
then qualify focus/geometry; it must not create a duplicate visibility model.
The finding table above remains the original audit observation.
