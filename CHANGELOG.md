# Changelog

## Unreleased

- Keep unhandled text-field keys within their editing context so a platform-specific Redo shortcut cannot change independent PDF review or mark history.

- Retain observed Windows job committed-memory peaks beside native validation logs and compile generated UI separately from application code under the existing build limits.

- Keep reader Undo/Redo within Review while its panel is open, including exhausted histories, so remark shortcuts cannot change independent marks.

- Refresh Windows notices from the native target graph, retaining exact notice bytes and excluding Linux build-host packages.

- Prepare owned Windows helper jobs with bounded pipes, deadlines and cancellation for qpdf and silent Quick Print; keep native dialog handling separate.

- Bound Linux CUPS discovery and submission helpers; report uncertain submission without automatically resubmitting.

- Bound Linux noninteractive helper pipes, deadlines and output for decryption, metadata, page assembly/bookmarks/units, review exports and compact-print preparation, with generated failure/retry tests. Windows supervision and app-level cancellation remain pending.

- Expose active-review revision selection in the reader toolbar and narrow More menu, sharing existing review state; check revision/filter synchronization independently of remark and mark visibility.

- Keep Linux staging and summary in separate workflow steps and guard duplicate step keys with a source-only regression test.

- Strengthen disposable-account MSI upgrade/default/user-data checks, retain source-bound Linux test runtimes, and make the generated CUPS script fixture robust to executable-file write locks without adding production print retries.

- Make release-adapter unit tests portable in public CI, separate installed-controller integration, use the verified tracked policy, and reject three-digit preview results before writing files.

- Record verified public preview delivery and the selected public Actions/local COPR publication roles.

- Run bounded Windows installer packaging under the current PowerShell host so required hashing commands resolve consistently.

- Preserve immutable Windows builds and packaging diagnostics before installer qualification; recover an existing tag from a passed exact-source native cache without recompiling, retagging or overwriting release assets.

## 0.1.54 - 2026-11-08 - Development preview

- Continue the development preview in the clean public source repository, preserving product version progression and license notices; keep hosted validation separate from manually authorized native desktop workers.

- Resume existing preview delivery through the authenticated local publisher, retain exact-tag receipts, expose delivery status and recovery commands, and hand off pushed tags to the existing publication timers without hosted jobs or new versions.

## 0.1.53 - 2026-11-07 - Development preview

- Show a readable, actionable message when a damaged or invalid PDF cannot be opened; retain engine diagnostics in the application log.

## 0.1.52 - 2026-11-07 - Development preview

- Keep reader actions reachable in narrow windows with a compact More menu, preserve primary reading controls, and elide version details when toolbar space is limited.

## 0.1.51 - 2026-11-07 - Development preview

- Open dropped local PDFs on native Wayland, including Unicode filenames and multiple files; reject malformed or remote URI inputs and bound asynchronous transfers.

## 0.1.50 - 2026-11-07 - Development preview

- Record actual Fedora preview installation/desktop launch, normal GNOME clipboard interoperability, generated 50/300/1000-page reader measurements and native GTK printing; keep physical-printer and current Windows acceptance explicitly pending.

- Retry the newest search after PDF worker queue congestion instead of dropping it and leaving "Searching…" indefinitely; discard deferred searches after cancellation or tab changes and report a disconnected worker.

- Return a failing publication exit status when an apply run lacks GitHub API authorization, preserving pending plans and verified receipts instead of reporting a healthy no-op.

- Synchronize the frozen delivery checkpoint from exact-source page and package receipts, retaining unfinished native Windows/final verification gates; propagate historical recovery failures through the real CLI exit status.

- Recover verified existing Fedora release assets without rebuilding or downloading them; preserve conflicting GitHub assets, verify server hashes/sizes, and continue independent later releases after a historical recovery failure.

- Record source-built v0.1.49 Fedora 44/45 RPM hashes, exact-version upgrades, installed rendering/search, controlled CUPS, desktop and user-data lifecycle checks; retain v0.1.48 as the full application/printing baseline.

- Skip Windows source-resolution and queued-receipt jobs together with their disabled private-repository workers, avoiding unnecessary hosted dispatch and misleading validation failures.

## 0.1.49 - 2026-11-06 - Development preview

- Reject private references in exported filenames and directory names, including binary assets, while redacting diagnostics and retaining a hash for locating the original path.

- Record the refreshed historical privacy inventory, verified private backup, unapplied documentation drafts and reversible PR-metadata cleanup without changing history or visibility.

- Recover missing GitHub preview pages from existing exact tags with idempotent API readback, privacy-checked notes and explicit pending-authentication receipts; distinguish tag push from completed publication.

- Record qualified v0.1.48 Fedora 44/45 COPR packages, artifact hashes and automatic local publication; refresh the requirements matrix and preserve historical Windows evidence.

- Place COPR source-assembly scratch files in the private disk-backed user cache so RAM-backed temporary files cannot consume the worker's memory budget.

- Allow source-vendor page-cache headroom within the local COPR worker's unchanged hard memory/CPU caps; clarify that GitHub asset attachment needs separate API authority.

## 0.1.48 - 2026-11-06 - Development preview

- Reuse exact native build artifacts by commit/toolchain/flags/hash, bound installer work, and submit/monitor COPR from an idempotent local operator without a hosted wait.

## 0.1.47 - 2026-11-06 - Development preview

- Route private compilation through existing [private] and [private] adapters; retain public hosted validation and lightweight publication. Refuse unmerged Windows validation SHAs, keep private privileged worker dispatch disabled, and preserve receipt/provenance and normal release gates.

## 0.1.46 - 2026-11-06 - Development preview

- Preserve vector/text content when printing disjoint page ranges through CUPS
  by assembling a private PDF subset before submission. Keep the raster fallback
  if source preparation fails; never retry after an ambiguous submission.

- Submit the opened PDF directly through GTK to configured PDF-capable CUPS queues
  when layout can run server-side, preserving text/vector content and compact jobs.
  Preserve settings for a native raster fallback confirmation; keep immutable private
  snapshots and report submission errors without automatic duplicate jobs.

- Record generated 50-page and single-page virtual-print output sizes, hashes and
  end-to-end timings in Linux and Windows desktop CI artifacts. The measurements
  establish a compact-printing baseline; physical-printer speed remains unqualified.

- Build Fedora 44 and 45 packages together in the release publisher and verify
  each target independently. Keep recovery artifacts small by retaining the
  publication receipt/checksums instead of duplicating large source RPMs.

- Require a source-RPM download URL and unambiguous verified corresponding source
  RPMs before attaching Fedora release assets.

- Stream RPM and source-cache checksums instead of loading entire release
  archives into memory while preparing and publishing Fedora packages.

- Read COPR's nested source-package dictionary correctly when downloading the
  corresponding source RPM for release attachment; reject missing/untrusted URLs.

- Use LLD for Linux application linking within the existing bounded build service;
  include the linker in CI build dependencies. Native Windows linking is unchanged.

- Verify the source RPM's embedded source revision as well as the binary before
  attaching Fedora release assets; reject missing or ambiguous revision markers.

- Prepare Fedora 44 source/RPM and COPR release packaging with source provenance,
  locked vendored dependencies and Slint's GPLv3 distribution option, reflected
  in About and current notices while owned source remains Apache-2.0. Redact
  private workstation details from product documentation and historical reports.
- Exercise genuine 270-degree pages in the generated secure-block corpus (the old
  fixture's `/Rotate` overflowed and was never rotated). The bake stays geometrically
  exact; the independent Poppler appearance limit is 5 to absorb its rotated
  rasterisation phase, with PDFium limits and zero outside-pixel change unchanged.

## 0.1.45 - 2026-11-04 - Development preview

- Add Undo/Redo for review remarks: adding, removing and editing remark text and
  creating a revision are undoable from the Review panel, or with Ctrl+Z /
  Ctrl+Shift+Z / Ctrl+Y while it is open; visibility and filters are kept.
- Show page/total in the status bar and the remarks on/off and active-revision
  state whenever review is loaded, not only once remarks exist (R-UI-4); clear a
  previous tab's review state when switching to a document without review.
- Add a **Privacy…** panel: turn local draft recovery off (persisted; existing
  drafts are kept until deleted) and delete every recovery draft after
  confirmation, while recovery is on or off. Remove empty session folders left
  by interrupted instances. Real-window recovery checks cover both paths.
- Add hosted pull-request CI: repository policy (no agent/private files, no
  release-metadata edits, Unreleased changelog entry), Linux format/Clippy/debug
  and release unit tests, release build, every PDF corpus and the private-Xvfb
  window suites at three scales, plus the existing hosted Windows harness for
  pull-request heads. Pull-request code never runs on [private].
- Add `./release.sh upstream` to bring merged pull requests into the editable
  source and release checkout; synchronization and previews now refuse to run
  while GitHub `main` has merged work the release checkout lacks, so a checkpoint
  can no longer silently revert it.
- Restore page editing and signature/stamp copy export with qpdf 11, which the
  documentation already claimed to support: qpdf 11 job JSON accepts a single
  rotation string, not the qpdf 12 list. Mixed rotation angles in one export now
  explain that they need qpdf 12.
- Fix an arithmetic overflow in the generated block-probe fixture: quarter-turn
  case 3 wrote `/Rotate 14` in release builds instead of `/Rotate 270`.

## 0.1.44 - 2026-11-04 - Development preview

- Publish a per-user Windows MSI installer and a portable zip with every release.
  They bundle the pinned PDFium and qpdf with their notices, register Start Menu
  and Open With without changing the PDF default, and are install/run/uninstall
  tested on a clean hosted Windows runner before upload.
- Add the canonical product specification, a requirements and implementation
  matrix, and a local-knowledge handoff audit; correct the stale `/UserUnit`
  milestone note. Documentation only.

## 0.1.43 - 2026-11-03 - Development preview

- Keep partially typed page and zoom entries while their fields are focused;
  navigation, rendering and relayout no longer reset them mid-entry.
- Add real-window Scenario A checks: slow page entry, invalid and clamped page
  entries, bookmark navigation, byte-identical Save As after view changes,
  clear refusal of unwritable Save As destinations, and invalid print ranges
  refused without submitting a job; missing PDFium now fails clearly.
- Adopt the [private]-drawn application icon (page, "d" monogram, signature stroke)
  for the window, Fedora launcher and Windows registration.

## 0.1.42 - 2026-11-03 - Development preview

- Refuse three-digit preview patch numbers in release verification; the
  0.1.99 rollover to the reserved v0.2.0 remains refused before acceptance.
- Reduce application-only compiler optimization after renewed soft-limit stalls;
  keep dependency profiles and host resource budgets unchanged.
- Extend the generated bake preflight to page/tree/resource properties and page
  content ownership, refusing retained-content aliases and shared page streams.
- Add an indirect-resource positive case, four additional refusal fixtures, and
  exact native-report coverage checks; keep production baking disabled.

## 0.1.41 - 2026-11-02 - Development preview

- Export signature/stamp overlays at their requested physical size on custom-unit
  pages, retaining page scale, crop/rotation, alpha and untouched page streams.
- Refuse invalid units or insufficient coordinate precision before replacing an
  output; verify saved page units and physical image dimensions.
- Extend coverage to 192 overlay cases, recovered-mark worker export, and native
  GTK/Windows virtual-print checks for a 40×20-mm mark on units 1/2/0.5.

## 0.1.40 - 2026-11-02 - Development preview

- Honor custom PDF page units in the reader's 100% and fitted layouts, including
  mixed-unit documents; retain the reading point when physical dimensions arrive.
- Share the existing post-first-frame metadata inspection with independent review
  and physical-size decoding; refuse mark placement until dimensions are verified.
- Verify physical snapshot dimensions before restoring recovered marks, including
  marks that fit the printed page but exceed its raw coordinate bounds.
- Add real-window mixed-unit, rotation/crop, malformed metadata and background-tab
  checks at three display scales to Fedora and native Windows desktop validation.

## 0.1.39 - 2026-10-30 - Development preview

- Refuse extended catalogs before the generated signature-block probe changes a
  page, covering retained content streams and embedded backup files.
- Expand generated refusals from eight to ten and require their exact identities
  in Windows reports; keep production Secure Signature Block disabled.

## 0.1.38 - 2026-10-30 - Development preview

- Keep original bookmarks pointed at replacement PDF/image pages and pages changed
  by mark export, including nested, named and GoTo-action bookmarks.
- Preserve bookmark zoom, styling and action chains; validate restored targets
  before atomically saving the document copy.

## 0.1.37 - 2026-10-30 - Development preview

- Allow up to five seconds for signature-library metadata lock contention on its
  I/O workers, avoiding the reproduced premature one-second size/usage failure.
- Verify delayed lock release and bounded refusal of a persistently locked asset;
  retain fresh metadata merging and unchanged source/cleaned signature images.

## 0.1.36 - 2026-10-30 - Development preview

- Preserve physical page dimensions when replacing a custom-unit PDF page with a
  scan or photo; retain untouched page streams and sample images at physical size.
- Reduce dPDF's release compiler memory using package-only optimization level 2,
  retaining dependency profiles and the existing shared build-host resource caps.

## 0.1.35 - 2026-10-30 - Development preview

- Honor PDF page-unit multipliers for actual-size native printing on GTK and
  Windows GDI; retain scale-independent Fit to Paper without the metadata lookup.
- Verify physically equivalent half/double-unit PDFs against native printed marker
  size and position, preserving source bytes and existing print safety checks.

## 0.1.34 - 2026-10-30 - Development preview

- Refuse millimetre-sized mark exports on affected PDF pages with custom page
  units before writing a copy, preventing a reproduced doubled signature size.
- Resolve default, null and indirect page-unit values through bounded qpdf metadata
  inspection; keep unrelated custom-unit pages intact and preserve prior outputs.
- Add generated physical-unit regression cases to Fedora and Windows validation;
  retain the independent remarks model while sharing metadata inspection.

## 0.1.33 - 2026-10-30 - Development preview

- Make the toolbar zoom percentage editable: enter a whole value from 10% to
  600%, with or without the percent sign, while retaining the reading point.
- Restore the current value after invalid input and exercise the actual zoom field
  with pointer/keyboard events at three display scales.

## 0.1.32 - 2026-10-30 - Development preview

- Keep the reading point visible when zooming with toolbar buttons or Ctrl+plus /
  Ctrl+minus, including page gaps, facing pages and rotated views.
- Extend the three-scale real-window regression with dispatched zoom shortcuts
  in all twelve layout/zoom/rotation cases, retaining a demonstrated failing baseline.
- Wait briefly for Windows fixture log handles to close before removing temporary
  drop-test files; retain failure for persistent locks and print success after cleanup.

## 0.1.31 - 2026-10-30 - Development preview

- Add native Windows OLE file-drop regression checks at three display scales,
  covering cancelled hovers, Unicode filenames, multiple files and duplicate tabs.
- Keep native pointer/keyboard tests behind explicit dedicated-desktop opt-in,
  separate from noninteractive service validation.

## 0.1.30 - 2026-10-30 - Development preview

- Require absolute PDFium overrides and a bundled/configured engine on Windows,
  avoiding automatic PDFium discovery through document folders or PATH.
- Exercise engine discovery with isolated executable copies and harmless library
  decoys, retaining configured and adjacent-runtime loading on both platforms.

## 0.1.29 - 2026-10-30 - Development preview

- Port generated-only signature-block replacement into Rust with bounded engine
  handles/output, a 400-DPI profile, 32 geometry cases and eight fail-closed fixtures.
- Add native Windows block-probe coverage with exact-build machine-readable reports;
  keep production Secure Signature Block export disabled pending qualification.
- Allow bounded diagnostic raster widths for print-resolution comparisons and
  record separate print-target, resampled and preview quality limits.

## 0.1.28 - 2026-10-30 - Development preview

- Separate explicit real-window smoke diagnostics from the reader controller,
  retaining the existing regression coverage and command-line entry points.

## 0.1.27 - 2026-10-30 - Development preview

- Preserve the reading point when resizing the window or showing/hiding panels,
  instead of returning to the top of the current page; clamp at document edges.
- Add real-window reflow checks for single/continuous/facing views, view rotation,
  fixed/fit-width zoom and three desktop scales, with a recorded failing baseline.

## 0.1.26 - 2026-10-30 - Development preview

- Reveal the selected document tab after opening, keyboard switching, closing and
  resizing; add Ctrl+Shift+Tab for reverse cycling and retain manual strip scrolling.
- Exercise twelve-document overflow and complete tab/close-control visibility at
  three desktop scales in Fedora release checks and the Windows desktop lane.

## 0.1.25 - 2026-10-29 - Development preview

- Scroll the thumbnail list while a page-order drag hovers at its top or bottom
  edge, with viewport-relative drop positions and cancellation checks.

## 0.1.24 - 2026-10-29 - Development preview

- Record saved signature/stamp usage on actual page placement, using a bounded
  background writer and coordinated metadata updates that preserve default sizes.
- Refuse text/path clipping before mutation in the generated block prototype after
  a shared text-clip fixture exposed an outside-region rendering change.

## 0.1.23 - 2026-10-29 - Development preview

- Extend the generated-only combined-block investigation across 32 rotation, crop,
  negative-origin and inherited-box cases with independent rendering, extraction
  and single-object removal checks; keep production secure baking disabled.
- Keep validation assertions enabled in release and Windows test environments;
  reject optimized Python execution for the block investigations.

## 0.1.22 - 2026-10-29 - Development preview

- Correct Windows registration validation when the PDF extension key was absent:
  an added empty container is distinct from changing PDF default values.
- Demonstrate generated-only PDFium region replacement with a 350-DPI combined
  block, independent Poppler checks and unsupported-case refusals; keep production
  Secure Signature Block export disabled.

## 0.1.21 - 2026-10-29 - Development preview

- Add Windows per-user Start Menu/Open With registration and cleanup for staged
  runtimes, using a multi-size dPDF icon and preserving PDF default settings.
- Distinguish cancelled Windows validation from failures, retain interrupted-check
  context, and preserve diagnostic logs for scale cases with numbers in their names.

## 0.1.20 - 2026-10-29 - Development preview

- Share the immutable opened PDF bytes with the engine instead of retaining a
  second whole-file buffer; preserve exact-copy/recovery snapshot behavior.
- Add a practical saved-file sharing panel with Copy file path and Open folder,
  requiring unsaved signatures/review changes to be saved first.
- Match native Fedora window identity to the installed dPDF desktop entry.

## 0.1.19 - 2026-10-29 - Development preview

- Connect native operating-system PDF drops to the reader through the pinned
  desktop backend, deduplicating files still opening and checking real isolated
  X11 multi-file/cancel/keyboard-tab behavior.
- Record executable and PDF engine hashes with generated reader benchmarks, and
  retain exact hosted Windows clipboard/placement/printing evidence.

## 0.1.18 - 2026-10-29 - Development preview

- Add a compact application icon and Fedora user-level application-menu/Open With
  registration for the staged runtime, preserving the current default PDF viewer.

- Exercise native Windows text/image clipboard and placement/history workflows
  on the dedicated hosted desktop at 1×, 1.5× and 2×, matching Fedora scale checks.

## 0.1.17 - 2026-10-29 - Development preview

- Stage hash-verified Rust dependency notice inventories on Fedora and Windows;
  add a separate hosted desktop test lane guarded by actual session accessibility.

- Extend native print diagnostics to fixture-owned Windows 11 broker dialogs
  through owner-chain verification and UI Automation, without global input injection.

- Collect target-specific Cargo license notices with content hashes and exact
  upstream provenance for omitted crate texts; keep distribution-audit limits explicit.
- Give every Windows run/attempt a fresh report directory and verify its identity,
  preventing stale artifacts on a persistent [private] runner from appearing as new results.

## 0.1.16 - 2026-10-29 - Development preview

- Check page replacement, physically sized overlays and visible/hidden/clean review
  copies independently with Poppler across rotations/crops, retaining binary-version evidence.

- Honor custom Windows Cargo target directories during runtime staging and add
  a relocated-package smoke check for adjacent PDFium discovery and retained notices.

- Probe existing PDFium object removal on generated text, vector, form, image and
  ActualText cases; record the shared-ActualText blocker and retain the secure-export gate.

- Select one native executable when Windows PowerShell finds duplicate Python,
  Cargo or Git installations; retain bootstrap and failed-suite diagnostics.

- Publish queued Windows-validation receipts before waiting for a runner; update
  the same diagnostics branch on completion, with explicit Actions read permissions.
- Retain optional test skips and let [private] setup use its existing local GitHub login
  to obtain a short-lived registration token without copying stored credentials.

## 0.1.15 - 2026-10-29 - Development preview

- Add independent hosted Windows and [private] release-validation lanes with exact-commit
  reports, bounded native MSVC builds and all noninteractive regression corpora.
- Isolate [private] runner/toolchain/data from dGK, publish diagnostics on report-only
  branches, and keep dedicated desktop/native print tests separate from services.

## 0.1.14 - 2026-10-29 - Development preview

- Gather reader page geometry without loading every page's content before the
  first frame; add phase/memory benchmarks and a 32-case geometry comparison.
- Add an About window following RUWAC/GSE information, project and optional-support
  sections, with local QR and Slint attribution; open public links only on action.
- Adopt Apache-2.0 for dPDF-owned work, retain Slint's royalty-free desktop terms,
  and stage notices on Fedora/Windows with attribution on release download pages.
- Add a generated redaction capability corpus recording crossing-vector,
  coarse-image and ActualText failures; keep genuine secure baking gated.

## 0.1.13 - 2026-10-29 - Development preview

- Add native Windows printing through an isolated system-dialog/GDI process,
  current/range/all selection, fit/actual size and bounded print-quality rasters.
- Protect controlled print-to-file paths, abort failed spool jobs, retain the
  explicit Quick Print helper, and add a Microsoft Print to PDF diagnostic suite.
- Type-check the Win32 adapter in standard [private] release checks; native Windows
  execution and physical printer output remain separate qualification gates.

## 0.1.12 - 2026-10-29 - Development preview

- Keep current-revision and selected-revision filters distinct, validate filter
  changes atomically, and reset remark edit state when switching documents.
- Show failed review loads with an explicit retry action while ordinary reading
  remains available; cover active mark controls and cross-tab editing regressions.
- Add an opt-in native Windows display context to the shared diagnostic helper,
  isolate fixture configuration/data on both platforms, and retain [private]'s v0.1.10 report.

## 0.1.11 - 2026-10-29 - Development preview

- Add independent review notes, rectangular highlights and underlines, revision sets,
  global visibility, current/selected-revision filters and a review sidebar.
- Export standard PDF annotations and embedded revision metadata as visible,
  current-visibility or clean copies; preserve page streams and foreign annotations.
- Combine review and signature-copy exports without flattening remarks; protect newer
  edits during saves and recover mixed or remarks-only drafts after interruption.

## 0.1.10 - 2026-10-29 - Development preview

- Open password-protected PDFs with a masked prompt, retry/cancel and queued-file
  handling; preserve original encrypted bytes in ordinary Save As.
- Keep passwords in memory and private pipes, prepare bounded unlocked working
  copies for edits/printing, and warn that edited copies are unencrypted.
- Retain [private]'s exact v0.1.8 native validation report, including the confirmed drive/UNC
  alias fix and successful reader/placement/studio scale matrix.

## 0.1.9 - 2026-10-29 - Development preview

- Recover unsaved placed marks from bounded local checkpoints containing the opened
  PDF snapshot, physical sizing and cleaned images, even if the source later changes.
- Save checkpoints off the UI thread, protect the store against competing instances,
  offer explicit recover/discard controls and retire drafts after successful export.

## 0.1.8 - 2026-10-29 - Development preview

- Add per-tab Undo/Redo for signature/stamp insertion, move, resize, numeric edits
  and removal, with one action per drag, cancellable gestures and bounded image history.
- Keep native text-field Undo separate from mark shortcuts; retain newer drafts
  when an earlier export finishes and retain all history after failed saves.

## 0.1.7 - 2026-10-29 - Development preview

- Fix Esc cancellation of a pending mark when the main keyboard capture owns the event.

- Add a native Fedora print dialog in an isolated process, with current/range/all
  pages, system page setup and copy controls; retain direct CUPS Quick Print.
- Render print pages at bounded print quality while keeping reader navigation
  responsive, protecting source paths and treating cancellation as cancellation.

## 0.1.6 - 2026-10-29 - Development preview

- Protect PDF, scanned-page, mark-image and placement-recipe inputs by file identity,
  including hard links and Windows drive/share aliases; recheck before publication.
- Cover alias refusal and unchanged bytes with generated Fedora/Windows regressions,
  and continue Fedora acceptance independently of [private] test scheduling.

## 0.1.5 - 2026-10-28 - Development preview

- Normalize Windows drive/UNC paths consistently at qpdf helper boundaries while
  retaining canonical Rust paths and atomic source/output safeguards.
- Synchronize locked studio dimensions when saving or placing, including fast edits
  that occur before the preview timer; retain [private]'s exact v0.1.3 validation report.

## 0.1.4 - 2026-10-28 - Development preview

- Place cleaned signature/stamp images from the studio onto PDF pages, move and resize
  drafts, adjust printed mm sizes/opacity, and export a separate marked copy.
- Keep marks attached to their tabs, confirm discarding unsaved drafts, and prevent
  printing/page operations from silently excluding them; preserve untouched pages.

## 0.1.3 - 2026-10-28 - Development preview

- Add diagnostic simple-overlay copy export with physical mm placement, transparency
  and rotated/cropped page geometry; preserve untouched pages through qpdf assembly.
- Keep ordinary image overlays explicitly separate from secure baking; validate
  multiple marks, input protection and rendered positioning with generated fixtures.

## 0.1.2 - 2026-10-28 - Development preview

- Add a graphical signature/stamp cleanup studio with local-library browsing, crop/deskew previews and preferred millimetre sizing.
- Support explicit clipboard-image capture and retain valid library entries when
  another item has damaged or future metadata; exclude private asset data from releases.
- Keep image processing off the UI thread and preserve immutable imported sources
  through preview and save; release working images when the studio closes.

## 0.1.1 - 2026-10-28 - Development preview

- Fix Windows qpdf page exports by closing the temporary output handle before child-process writes.
- Use portable fixture filenames and actual layout coordinates for scale-aware reader diagnostics.
- Publish a versioned patch preview before completing development prompts, using
  the standard controller, guarded release checkout, annotated tags and GitHub release automation.

- Add signature/stamp cleanup and local asset-library groundwork.
- Preserve imported originals, cleaned transparent PNGs, thumbnails and preferred
  millimetre sizes; validate crop/deskew/levels, aspect lock and metadata versions.
- Expose diagnostic asset commands while the graphical mini-studio and placement
  workflow remain in development.

- Arrange output pages by dragging thumbnails with visible insertion targets.
- Keep thumbnails keyed to source pages, validate typed order, and support reset
  or cancellation before saving a reordered copy.

- Add rectangular PDF text selection and native clipboard copying.
- Support Ctrl+A for current-page text, Ctrl+C, Copy and Esc, with crop/rotation
  coordinate handling, stale-result rejection and an isolated clipboard diagnostic.

- Replace, insert and merge scanned PNG/JPEG pages with physical page sizing, aspect
  preservation, phone-photo orientation and bounded image decoding.
- Preserve untransformed JPEG streams and validate image import failures before
  replacing output; extend the contract corpus to mixed PDF/image sources.

- Add PDF page replacement, insert/remove/extract/merge/rotate/reorder copy export through qpdf.
- Validate assembled outputs before atomic save, preserve original snapshots and
  untouched page streams, and reopen edited copies in tabs with signature warnings.
- Virtualize central page controls and discard obsolete scroll-render requests;
  build and benchmark an optimized native executable under the standard [private] limits.

- Discover Fedora printers asynchronously and retain view and print preferences between sessions.
- Keep printer submission off the PDF worker, use the viewed page for Current Page,
  and test CUPS ranges, literal arguments, byte transfer, and error propagation without printing paper.

- Add individual search highlights, next/previous match navigation, and pointer-anchored Ctrl+wheel zoom.
- Preserve tab scroll when closing another document; follow the current page in
  the thumbnail sidebar and verify crop/rotation/search limits with a generated corpus.

- Connect the owner w3fans/dPDF repository through the configured w3fans SSH alias;
  add guarded development checkpoints from the app build/release checkout.

- Start the 0.1.0 reader development preview in Rust/Slint/PDFium; reserve v0.2.0
  for the first stable reader milestone after Fedora and Windows acceptance.
- Add background PDF opening/rendering, tabs, lazy thumbnails, bookmark jumps,
  single/continuous/facing layouts, zoom/fit, view rotation, themes and page search.
- Preserve original bytes in atomic Save As and print from the opened snapshot;
  add validated CUPS ranges/scaling and an optional explicit Windows print helper.
- Retain standard [private] build tooling and standard release-controller checks; add
  compatibility/native Windows scripts, runtime staging, architecture/milestone
  docs, original PDF fixtures, and integrity/cache/geometry unit tests.

## 2026-10-28

- Prepared [private] Rust/editor tooling, initial dPDF Cargo starter and 0rig help.
- Configured [private] remote compilation with conservative host resource limits.
- Added explicit remote check/build/test tasks and preserved local runtime.
