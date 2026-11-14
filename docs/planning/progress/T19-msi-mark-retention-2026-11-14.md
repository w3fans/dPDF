# T19 valid generated mark-library MSI retention

Status: IN_PROGRESS; native PowerShell parsing and MSI execution pending.
Initial baseline main 8510094a, reconciled qualified main b172dc1b after portable
PR #22 merged. Isolated test/msi-mark-retention; no production application change.

## Verified fresh-MSI baseline and remaining gap

Existing artifact-only run 43551199504 on exact source 847d1c58 passed build and
report; publish was skipped by its existing branch-push guard. Inspected all
three artifact archives, checksums and native build identity. The portable
executable and one statically extracted MSI cabinet entry match retained binary
fa7433527c06107d1a57d63ddce35499d7a46cb5193c232095721e9d779e466a.
All pinned qpdf/helper DLL bytes, PDFium DLL and 485 Rust notice references match.
Normal Git text files use Windows CRLF; their canonical content matches source,
while hash-addressed Rust notices remain byte-exact. Source/package/archive hashes
and precise scope are in [T19-msi-fresh-847d1c58.json](T19-msi-fresh-847d1c58.json).

The run proves generated fresh per-user install, version/PDFium/qpdf probes,
registration, uninstall, default snapshot equality and sentinel byte retention
on that one hosted profile. It does not qualify newer source, both explicit
default starting states, major upgrade or valid saved-model reload. Its installed
executable hash and plaintext compiler probes were not directly retained. The
public release inventory contains only v0.1.54; a strictly older qualified MSI
is still needed from the package owner. No version will be allocated to fake it.

Existing sentinels are explicitly not valid mark/recovery schemas. A qualified
Linux CI binary lists a sentinel-only library as empty. The canonical generated
JPEG imports/reloads one valid stamp with all four model-file hashes unchanged;
see [fixture reproduction](T19-msi-mark-fixture-reproduction.json). That is Linux
CLI fixture evidence, not native Windows/MSI acceptance.

## Prepared check

Pair optional SeedBinary/SeedImage inputs for manual callers; the existing
package-preview path always supplies its attested staged binary and canonical
generated JPEG. Keep all absent-install/data guards before writing. Clear data
and config overrides, verify seed version/hash, then import one synthetic stamp
at the actual default library location. Keep preferences/recovery sentinels.

Record exact original/cleaned/thumbnail/metadata hashes, compare original bytes
to the seed image, and decode both generated PNGs through the same app. Require a single
unchanged model with matching identity/schema/name/kind through the installed
binary and after uninstall through the retained staged binary. Every reload
requires exact candidate executable bytes. Preserve seed image bytes and all
existing source/default/registration/uninstall assertions. Success-only cleanup
touches only fixture-owned roots; failures retain generated data for diagnosis.
Emit a compact receipt of generated file and executable hashes after success.

Retain native compiler/Cargo version-probe text (or the recovered validated report)
and packaging Job Object memory metrics in the existing artifacts. No new worker,
compiler route, publisher or resource cap. Plaintext probes supplement existing
hashed identity; they do not turn a build receipt into operator acceptance.

## Checks and next action

All fifteen source contract suites pass: 124 tests, 120 passed/four existing
platform skips. The modified installer workflow passes verified actionlint 1.7.12
with shellcheck integration disabled; this does not parse its PowerShell files.
Whitespace and unchanged release metadata verified locally. No PowerShell host
or Windows MSI execution has run for this candidate; these gates stay PENDING.
Run source contract suites and actionlint, publish focused draft, then require
native PowerShell parsing and ordinary/minimum-compiler gates. Before merging,
exercise this exact candidate through the existing artifact-only installer branch
and inspect its new mark receipt, toolchain probes and packaging metrics. Never
attach these test outputs to a release or relabel an earlier package.

This proves only current-format generated marks and CLI data reload once native
execution passes. Recovery drafts, GUI/native operator acceptance, old-format
migration, two explicit default-state sessions and older-MSI major upgrade remain
separate. No app/dependency/runtime/version/tag/release/license choice or
production baking change. Rollback: revert this fixture/artifact-retention change;
preserve user data, qualified packages and all failing evidence.
