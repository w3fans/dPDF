# Native CUPS PDF preview validation

Current published Fedora 44/45 packages and their installed qualification are in
[the 2026-11-06 COPR record](fedora-copr-2d52b2f.md). This document preserves the
initial branch qualification below with its original executable/source identities.

Validated 2026-11-05 in a disposable Fedora 44 root. This is unreleased branch
qualification, not a numbered release or physical-printer acceptance.

## Initial integrated candidate

The release executable SHA-256 is
`d3e4699f671d020bf7bc4112501623cd6043993c82343461f1e254cf97d979c8`.
The reconciled source commit is
`eada3214e8df97c17b4b3d8414dc783033f0515e`.
The tested `src/native_print.rs` SHA-256 is
`5f2a835661550becf58dd22b382b8189bc933dfd5fe7b23fb137e8dd68de56c2`;
`src/native_print/direct_pdf.rs` is
`ff8ef81eeab518611d0a3aff6f4e966794322e91d31e61a22e4d03879aec5462`.
The runtime is the source-built PDFium 7881 engine from the published Fedora
preview, with system qpdf and GTK 3.

## Controlled CUPS delivery

A private CUPS server and IPP Everywhere PDF receiver ran in a network namespace
with loopback only. Tests used a dedicated non-root account, private display,
generated PDFs and private spool/service directories. No physical queue was
reachable. The 50-page generated source is 33,864 bytes.

| Selection | Client spool bytes | Receiver PDF bytes | Receiver pages | Result |
| --- | ---: | ---: | ---: | --- |
| All 50 | 33,864 | 36,668 | 50 | Original snapshot byte-identical; searchable text retained |
| Page 7 | 33,864 | 1,204 | 1 | Original snapshot byte-identical; selected text retained |
| Pages 7–9 | 33,864 | 2,638 | 3 | Original snapshot byte-identical; selected text retained |
| `50,7-9,1,7` | 507,204 | 507,682 | 5 | Renderer fallback; pages 1,7,8,9,50 drawn once in document order |

Dialog-to-submission times for the three direct cases were 2.154, 1.654 and
1.668 seconds, including a deliberate one-second UI settling delay. They are
synthetic observations, not physical-printer performance measurements.

Cancellation produced no spool or receiver file. Replacing the pathname with a
three-page PDF while the dialog was open still printed page 7 from the original
opened snapshot. Controlled HTTP 400 and HTTP-200/IPP rejection cases each returned an error,
made one submission request and produced no job; no renderer retry occurred.
GTK transport success alone is insufficient: dPDF requires its tracked CUPS job
to enter the acknowledged pending/printing state before reporting submission.
Anonymous snapshot permissions and unlinked lifetime are also covered by unit tests.

## Fallback and limits

The tested GTK CUPS option encoder forwards only the first disjoint range. At this
initial baseline such selections used the renderer; the follow-up below supersedes
that limitation. Layout options that require client-side
work, non-PDF printers and unconfigured/direct-discovery printers also fall back.
GTK's no-dialog printer search excludes virtual printers and can substitute a
default; the fallback therefore presents a native confirmation using the selected
settings. An explicitly isolated file-only backend retains the original one-dialog
operation. Native authentication remains managed by GTK.

The existing real-window file-print suite passed all/current/disjoint page order,
fit/actual-size geometry, UserUnit 0.5/2, exported 40×20 mm marks, cancellation,
protected-input aliases, reader Ctrl+P snapshot transfer and responsive navigation.
Virtual file output remains rasterized: the generated 50-page PDF produced
4,261,087 bytes (125.83×) in 14.29 seconds; page 7 produced 82,751 bytes in 1.15
seconds. These are a different transport from the CUPS measurements above.

The reconciled candidate passed all 34 local application checks from the Linux
validation plan plus its CLI render/search check, including every window group
at scales 1/1.5/2. All eight controlled CUPS cases passed again. The integration
preserves both packaging-publication and print-benchmark CI tests and resolves
the four overlapping workflow/documentation edits.

Both debug and release unit profiles passed 68 tests. Clippy passed with warnings
denied, including the Windows-print check feature; no Windows-native code changed.
Required hosted PR CI, physical printers, native Windows, real ruler measurements
and owner acceptance remain separate gates. Production Secure Signature Block
remains disabled.

The matching source-only candidate SRPM was assembled as
`dpdf-0.1.45-0.1793877174.giteada3214e8df.fc44.src.rpm`, SHA-256
`b8cbd3b9a34fa605b82861ea59933fcc5f76fae60f458289924623b96f8882dd`.
It has not been published as a new COPR build or attached to a tagged release.
The current product export passed the selected private-reference scan (526 files,
zero findings); full-history/public-visibility review remains separate.

## Disjoint vector-print follow-up

Functional source `5c753c3fcf3cd074f55ba6b29e0ff4a0a52cf4d5` was validated with
executable SHA-256
`ff0438c3dc78cb3891cb35e88adde43d79a78b70c23786611e2f1163274c0edd`.
The tested `src/native_print/direct_pdf.rs` is
`0126092dbfeb78070f02745e487c7fa97c2d951c729932a35cb464542daac7ef`;
`src/assembly.rs` is
`aeabaf3a602d1d0a82cb757aae8d5e68bc57e778d3aee4e51b9c5f6adfa56462`.

Disjoint ranges now use system qpdf to select pages from the immutable bytes in
a private workspace. PDF streams remain intact. The output page count is checked,
the snapshot is unlinked, and original range options are cleared before sending
the subset, so the server does not apply them twice. A preparation failure occurs
before any submission and retains the existing raster fallback. Submission errors
still never trigger automatic retries.

For input `50,7-9,1,7`, both client and receiver contain exactly pages 1,7,8,9,50,
once each in document order:

| Path | Client spool bytes | Receiver PDF bytes | Images | Text retained |
| --- | ---: | ---: | ---: | --- |
| Earlier renderer fallback | 507,204 | 507,682 | 5 | No |
| New private PDF subset | 3,133 | 4,075 | 0 | Yes |

The client spool is 99.4% smaller. The direct case took 2.481 seconds including
the deliberate one-second dialog settling delay; a pathname-mutation repeat took
1.663 seconds. These are fixture timings, not physical-printer measurements.
The direct subset needs one dialog confirmation. Forced qpdf preparation failure
correctly used the two-confirmation renderer and reproduced its 507,204-byte spool.

Nine controlled cases passed: direct disjoint selection, all/single/contiguous
regressions, disjoint pathname mutation, cancellation, HTTP refusal, HTTP-200 IPP
refusal and forced subset-preparation failure. Refused submissions made one request
and produced no job. No private print workspace remained after the suite.

Both Rust unit profiles passed 68 tests; strict Clippy including the Windows-print
check feature and formatting passed. The native file-print suite passed again,
including physical geometry/UserUnit, 40x20-mm marks, protected inputs, cancellation,
Ctrl+P snapshot transfer and reader responsiveness. File output remains rasterized:
the 50-page stress case produced 4,261,087 bytes in 14.37 seconds, and page 7 produced
82,751 bytes in 1.27 seconds, including dialogs and validation.

This follow-up has not been published as a new COPR package or numbered release.
Required PR CI remains a separate merge gate. Windows-native, physical-printer and
owner acceptance are not claimed.

## Reproduction

Use an explicitly disposable Fedora root whose network namespace exposes only
loopback, with a dedicated non-root account. Install the preview runtime and test
dependencies: CUPS, cups-client, cups-printerapp, Avahi, dbus-daemon, GTK, Xvfb,
xdotool, qpdf and poppler-utils. Stage this source's scripts alongside the candidate.
Do not run this fixture service harness on the normal desktop.

Inside that root only:

```sh
export DPDF_DISPOSABLE_PRINT_ROOT=1
export DPDF_PDFIUM_LIBRARY=/usr/libexec/dpdf/libpdfium.so
python3 scripts/check-native-cups.py --user fixture --binary /tmp/candidate/dpdf
python3 scripts/check-native-cups.py --user fixture --binary /tmp/candidate/dpdf \
  --range '50,7-9,1,7' --expect-subset
python3 scripts/check-native-cups.py --user fixture --binary /tmp/candidate/dpdf \
  --range '50,7-9,1,7' --expect-raster --refuse-subset
```

Also exercise `--range ''`, `--range 7-9`, `--cancel`, `--mutate-source` and
`--expect-failure` (also add `--ipp-refusal` to test HTTP-200 IPP errors). Each run retains its generated report and logs and shuts down
only its own fixture processes. The ordinary `check-native-print.py --stress`
suite independently covers virtual-file printing and reader interaction.
