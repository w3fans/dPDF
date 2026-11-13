# T01 — Public release-adapter test portability

Status: DONE for T01. Public tests, required regression gates and routine code
review passed; PR #5 is merged. Authority-side integration remains explicitly
PENDING, which T01's exit condition permits as a separate recorded status. No
release-authority execution or platform acceptance is implied.

Baseline: `268b234e4336789922a6b15dd1236963e131d707`.
Branch: `fix/public-release-adapter-tests`. Existing Astra documentation edits in
README, CHANGELOG and docs/planning were preserved. No version or tag changed.

Reproduction: `PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-release-version.py`
failed with six setup errors for missing private controller files. Baseline log:
`/tmp/dpdf-t01-baseline.log` (local scratch).

Changes: mock subprocess results at the public adapter boundary, assert actual
controller arguments, test retry without repeated allocation, failed preflight and
validation without file changes, pending mismatch and forbidden controller outputs.
Use generated temporary metadata. Add the unit suite to public CI and document
explicit integration. Reject three-digit results before writing metadata. Switch
prepare to the tracked ops policy after real installed-controller compatibility
passed; version allocation still belongs to the controller.

Checks on 2026-11-13 in this Linux checkout:

- All 15 Python contract suites: 114 cases, 110 passed, four existing skips in
  test_ci; zero errors/failures. Logs: `/tmp/dpdf-t01-checks/` (local scratch).
- Explicit integration: `PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-release-version.py --integration-policy ops/release-policy.json`:
  seven passed, two injected-failure/result unit cases deliberately skipped.
  This exercises copied public release.sh and tracked policy with the real local
  installed controller, using temporary fixtures. It does not qualify AVF.
- `python3 scripts/release-version.py verify`: PASS, v0.1.54.
- `cargo fmt --check` and `git diff --check`: PASS.
- No Rust compile, native app/printing tests or publication: tooling-only change.

Controller identity tested (local `/www/0rig/tools/`):

- release-controller.sh SHA-256: `490d0174daf5bd9b750b45397b41ee031e1fd5128809edbcfc1cfafcfc9a23b9`
- release-policy.rb SHA-256: `dd0e1cc5a97973f5aab25fde0cd0d044cfd952354114a06f553301ea56ef7588`

Relevant requirements: release-tool reproducibility; no application requirement
or platform qualification is promoted. Historical audit counts remain baseline
observations. Requirement evidence is noted in REQUIREMENTS_MATRIX.

Next: reviewer/authority operator runs the explicit integration command on AVF,
records its controller identity and reviews public CI. Then close T01 when its
exit evidence is satisfied. Continue T02 documentation reconciliation meanwhile.
Rollback: revert the focused adapter/test/CI/docs changes; retain unrelated Astra
docs, generated receipts and all published artifacts.

## Focused public PR — 2026-11-13

Created draft https://github.com/w3fans/dPDF/pull/5 via the connected w3fans GitHub
account, verified public main still equals the audited baseline. Candidate
64d0d01da7fbcecd464d450134947dd820560da5 contains only five files: public CI,
adapter, unit/integration harness, isolated release-test documentation section,
and an isolated Unreleased bullet. Other local/Astra edits remain untouched.
Readback Git blob hashes for the three code/workflow files match local source.

Fresh pre-PR rerun: nine unit cases PASS; real local controller integration seven
cases PASS/two deliberate unit-only skips. Authority-side integration still needs
its own receipt. No versions, tags, main update or release operation performed.
T01 stays REVIEW pending public CI and authority/reviewer evidence.
Resume by inspecting runs for this exact candidate; do not duplicate a live job.

Public PR CI run 43463959518 is confirmed IN_PROGRESS for candidate
64d0d01da7fbcecd464d450134947dd820560da5. Track this exact run next session.

CI baseline repair: Linux logs confirmed the same Rust 1.100 needless_range_loop failure in block_probe.rs. Included the identical iterator-only correction on PR #5, candidate 756bb4cb8a6bbaf36c0c8abbdfab5bb52dacd604. Earlier run remains FAIL, not retried unchanged. Windows PR #5 prior candidate passed its hosted lane; that does not qualify the new commit.

Latest candidate 756bb4cb8a6bbaf36c0c8abbdfab5bb52dacd604 completed run
43465813520 with all policy, full Linux and Windows hosted jobs SUCCESS.
Marked PR #5 ready for review and updated its body with final evidence. Authority
integration still has separate PENDING status; no merge or release performed.

## Integration and task exit — 2026-11-13

Revalidated all public CI jobs on reviewed head
756bb4cb8a6bbaf36c0c8abbdfab5bb52dacd604, run 43465813520: SUCCESS. Reviewed
controller-boundary arguments, fail-before-write target rejection, fake-result
unit isolation and actual local integration scope. Under DEVELOPMENT_WORKFLOW's
routine-merge rule, squash-merged PR #5 with expected-head protection to main
1cf522aeccc1794e5cbad4af42a7a02fd1b05c75. Metadata remains 0.1.54; no tags,
released notes, stable transitions or publication changed.

T01's explicit exit permits installed integration to have a separate receipt OR
PENDING status. Earlier receipt wording requiring AVF execution before closing
the public-source task was stricter than the task card; that inference is corrected
here. AVF/controller operational integration is still pending before relying on
this candidate for an authority release, rather than reported as passed.

Local reconciliation: fetched merged main, asserted no staged user changes,
recorded /tmp/dpdf-before-base-update.patch and file hashes, created local branch
work/implementation-plan and advanced its index/base with a mixed reset. Verified
all 42 previously modified/untracked file contents unchanged. Unrelated Astra and
T02/T03/T04/T12/T19 work is preserved; the original local task branch remains.
