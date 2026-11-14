# Build, package, release

## Existing standard workflow

Source is `/zDev/dPDF`. The configured standard workflow offloads Cargo
build/check/test/clippy to remote build service. The remote mirror is disposable, not Git authority.
Retain its one-job/one-core/2-GiB resource limits; queued/busy jobs must wait.

```sh
cargo fmt --check
cargo check --locked
cargo test --locked
cargo clippy --locked -- -D warnings
./build                 # standard remote development build
./build --release       # standard remote release-profile build; no publication
```

`build.sh` is only a compatibility wrapper around `./build`. Never use `/usr/bin/cargo`
to bypass the remote compiler on the source workstation. When adding dependencies,
run an unlocked standard remote check once, then retrieve its generated Cargo.lock
using the approved build transport before returning to locked builds. The current
worker returns final binaries, not automatically the lockfile.

On a separate native Fedora build machine, install Rust, a C/C++ compiler driver, LLD, pkg-config,
fontconfig development files, and the desktop backend prerequisites; use native
Cargo only where the project's standard explicitly permits compilation. Linux
application linking selects LLD on the build machine to avoid prolonged GNU linker
memory pressure within the existing resource limits. At runtime
Fedora needs the system's Wayland/X11/font libraries, PDFium, and `cups-client`
(`lp`, `lpstat`) for Quick Print and printer discovery. Native printing also uses
installed `gtk3` and Cairo runtime libraries, loaded dynamically by the print child;
GTK development headers are not a build dependency. Reading still launches without
GTK. Native file dialogs use the desktop portal service.

## PDFium

Bindings are pinned to `pdfium-render` 0.9.5 and its `pdfium_8125` API feature.
Use [PDFium binaries release chromium/8125](https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium%2F8125)
for the matching OS/architecture, or build that revision yourself. The archive
contains the library plus LICENSE and licenses/; retain those with a staged runtime.

The Linux x64 archive used for the initial local probe was:

```text
pdfium-linux-x64.tgz
SHA256 1470e21b8b4a3b4ad7f85684e2da11d94f3b69a86d81dee11b9b6709d927ac1d
```

The checksum records the tested archive; verify downloaded provenance separately.
Do not use an unpinned "latest" engine or silently download code when opening PDFs.
The explicit `DPDF_PDFIUM_LIBRARY` setting must be an absolute path and takes
precedence, followed by an engine beside the executable. Linux also supports the
system library loader. Windows requires the configured or bundled DLL; it does
not request bare-name PDFium discovery through the working directory or PATH.
This avoids the working-directory/PATH entries in
[Windows' standard DLL search](https://learn.microsoft.com/en-us/windows/win32/dlls/dynamic-link-library-search-order).

```sh
export DPDF_PDFIUM_LIBRARY=/path/to/pdfium/lib/libpdfium.so
./target/debug/dpdf --probe /path/to/sample.pdf
./target/debug/dpdf /path/to/sample.pdf
```

## Runtime staging

Packaging scripts copy already-built artifacts. They do not replace standard
build or release tools, install system packages, register a default app, or publish.

```sh
./build --release
export DPDF_PDFIUM_DISTRIBUTION=/path/to/extracted/pdfium
./scripts/stage-linux.sh /app/dPDF
/app/dPDF/dpdf
```

Use a writable `/tmp/dPDF-preview` output for validation without changing `/app`.
Register a staged runtime for the current Fedora user with the standard
`desktop-file-utils` tools already installed:

```sh
python3 /app/dPDF/install-desktop.py
# Or register a runtime staged elsewhere:
python3 /tmp/dPDF-preview/install-desktop.py
```

The helper validates and installs an application-menu/Open With entry, scalable
icon and user MIME cache under `$XDG_DATA_HOME` (normally `~/.local/share`). It
leaves the default PDF viewer unchanged. To make an explicit user choice later,
use the desktop's Open With settings or
`xdg-mime default net.dpdf.dPDF.desktop application/pdf`.
Runtime executable paths must be ASCII without control characters, `%` or `=`;
registration refuses unsupported paths before writing user data. PDF filenames
can contain Unicode and spaces. Multiple file arguments are passed separately.
Keep the staged runtime in its registered location; rerun after moving it.
RPM/installer production packaging and full dependency-license manifests remain
release qualification work.

## qpdf page-operation runtime

Reading needs only PDFium. Page operations also need qpdf 11+; the tested Fedora
helper is qpdf 12.3.2, matching the installed qpdf-libs 12.3.2 / libqpdf.so.30.
Use a system qpdf, a helper beside dPDF, or `DPDF_QPDF_BINARY=/path/to/qpdf`.
No automatic runtime download or source-workstation package installation occurs.

The initial test helper was extracted into /tmp from the official Fedora build:

```text
qpdf-12.3.2-1.fc44.x86_64.rpm
SHA256 a4293aee60a2004af1d0e9e4c65461ab1443cc66732b9dbbdd6f79337081798c
```

Its RPM digests were checked. This helper is dynamically linked and requires the
matching system qpdf-libs; it is not a universal Linux bundle. To stage it alongside
an existing build, set `DPDF_QPDF_BINARY` and `DPDF_QPDF_LICENSE_DIR` before running
the Linux staging script. On the tested Fedora host, the notice directory is
`/usr/share/licenses/qpdf-libs`. Windows needs a separately supplied matching qpdf
runtime and its DLLs; native deployment remains unverified.

## Windows / Windows test machine

Use Rust 1.92+ with the native MSVC toolchain and Visual Studio C++ build tools on
Windows test machine. Do not route this through the Linux-only remote build service worker or cross-compile and claim
native acceptance.

```powershell
.\build.ps1
.\build.ps1 -Release
$env:DPDF_PDFIUM_LIBRARY = 'C:\deps\pdfium\bin\pdfium.dll'
.\target\release\dpdf.exe C:\docs\contract.pdf
.\packaging\windows\stage.ps1 -PdfiumDistribution C:\deps\pdfium
# Optional explicit executable; otherwise CARGO_TARGET_DIR is respected:
.\packaging\windows\stage.ps1 -PdfiumDistribution C:\deps\pdfium -BinaryPath D:\build-cache\release\dpdf.exe
```

The staged runtime includes `register.ps1` and a multi-size icon. In an ordinary
PowerShell session for the intended user:

```powershell
& C:\app\dPDF\register.ps1
# Remove only this runtime's user registration later:
& C:\app\dPDF\register.ps1 -Unregister
```

Registration adds a Start Menu shortcut, App Paths, PDF Open With and registered
application capabilities under HKCU. It never writes the PDF extension's default
value or protected UserChoice settings, changes PATH, or needs administrator
rights. Choose a PDF default explicitly in Windows settings if desired. Rerun
after moving/updating the staged runtime. Cleanup refuses another runtime's
registration or shortcut and leaves files, documents and user data in place.
This is portable runtime integration; a qualified production installer and full
dependency distribution audit remain separate work.

Use the matching Windows x64 archive. The Visual C++ runtime may be required by
native dependencies. Toolbar/Ctrl+P printing uses the Windows system print dialog
and GDI, compiled into the print child. Native Microsoft Print to PDF passed the hosted Windows desktop lane for
v0.1.17; physical printer output and Windows test machine Windows 11 qualification remain. Optional Quick Print requires a separately installed
[SumatraPDF](https://www.sumatrapdfreader.org/docs/Command-line-arguments):

```powershell
$env:DPDF_PRINT_HELPER = 'C:\Program Files\SumatraPDF\SumatraPDF.exe'
```

The helper is not required for System dialog printing. Run the native Windows
print diagnostic in docs/TESTING.md on Windows test machine; do not infer native acceptance from
Linux checks. Previously reported Windows test machine native builds are recorded by exact version
under docs/validation; hosted v0.1.17 results are recorded in docs/validation/windows-hosted-v0.1.17.md.

`cargo check --locked --features check-windows-print` and the corresponding strict
Clippy command compile the Win32 adapter during metadata checks on remote build service without
linking or executing Windows libraries. This feature is for checking only: a Linux
executable built with it is not a Windows binary. Default Linux builds do not link
Windows APIs. Standard release checks include the adapter's strict Clippy check.

## Releases

The owner repository is https://github.com/w3fans/dPDF. Git authority is the
`/app/dPDF/source` build/release checkout; editable source remains `/zDev/dPDF`.
The configured SSH origin is `git@github-w3f:w3fans/dPDF.git`, using the `w3fans`
identity and host settings from `${HOME}/.ssh/config`. Do not use the default
GitHub identity or assume GitHub CLI is signed in.

After the standard build and checks, push a development checkpoint with:

```sh
./release.sh checkpoint
```

This syncs through the installed `rsyn` alias and release workstation's configured app module,
protects Git/private runtime state, checks destination cleanliness and upstream
history, uses standard leases/hooks/validation and a release lock, and pushes
from the build/release folder. It does not bump versions, tag, or publish a binary
release. Private agent folders, credentials and build outputs stay excluded.

`release.sh` also delegates version checks to the installed standard release controller (or the
project's standard portable fallback). It accepts the same commands, for example:

```sh
./release.sh next patch 0.1.0 .agents/release-policy.yaml
./release.sh preflight patch 0.1.0 .agents/release-policy.yaml "$PWD"
```

These calculate/check a candidate only. They do not update Cargo.toml, tag, push,
package, or publish. Keep patch development below 0.2.0 until the stable milestone
gates pass. Before tagged releases, connect version files and qualification evidence; then use named release intent.

## Required patch release after each development prompt

Use `./release.sh patch` from editable source with the validated PDFium/qpdf
runtime variables set. This is the completion path; `checkpoint` is intermediate.
The existing standard controller calculates the next version. The project adapter
updates Cargo.toml/Cargo.lock, README, changelog, release.json and versioned notes.
It performs locked tests, Clippy, the standard optimized remote build service build and executable
reader/page/mark corpora, then synchronizes with standard rsyn. Only /app/dPDF/source
commits/tags/pushes. The existing release lock, leases and privacy hooks apply;
main and the annotated tag are pushed atomically, and existing tags are never moved.
A failed build leaves a resumable private pending-release marker; retry the same
command after correcting the failure.

The tag workflow publishes a GitHub development preview using GitHub CLI and its
repository-scoped ephemeral Actions token.

### Windows installer and portable zip

`.github/workflows/windows-installer.yml` runs for every `v*` tag on a hosted
Windows runner. It builds `dpdf.exe`, downloads the pinned PDFium 8125 and
qpdf 12.4.2 Windows archives (SHA-256 checked by `scripts/ci/runtime.py`), stages
them with `packaging/windows/stage.ps1 -QpdfBin`, and builds with WiX 5.0.2
(`packaging/windows/build-msi.ps1`, source `packaging/windows/dpdf.wxs`):

- `dPDF-X.Y.Z-windows-x64.msi`: per-user by default (no administrator rights),
  installed to `%LOCALAPPDATA%\Programs\dPDF`, with Start Menu, Open With and App
  Paths registration identical to `register.ps1`. It never sets the `.pdf`
  default. Upgrades replace older versions. An administrator can install for all
  users with `msiexec /i dPDF-X.Y.Z-windows-x64.msi MSIINSTALLPERUSER=""`.
- `dPDF-X.Y.Z-windows-x64-portable.zip`: the same runtime without installation.
  Run `dpdf.exe` directly, or `register.ps1` for per-user integration.
- `SHA256SUMS.txt`.

Before upload, `packaging/windows/test-msi.ps1` installs the MSI silently on the
clean runner. It runs the installed executable (version, PDFium probe, bundled
qpdf page extraction), checks registration and the shortcut, uninstalls, and
checks cleanup. A separate Ubuntu job with write permission attaches the files to
the tag's development-preview release. The outcome is also written to a
report-only branch `validation/ci-windows-installer-RUN-ATTEMPT`. Pushing the
branch `ci/windows-installer` runs the same build and test without attaching
anything.

These are **unsigned development previews**: Windows SmartScreen may warn until
code signing is configured. An installer build does not qualify a milestone;
Windows test machine/user acceptance of installation, file association and uninstall remains
separate.
After API readback, it stores a small publication receipt in the Git notes ref
`refs/notes/dpdf-releases`; SSH-only maintainers can fetch this ref and verify the
release URL, tag, API ID and exact source commit without a personal HTTP token.
This automated publisher is part of release authority; compilation remains on
the configured build hosts. Notes updates never modify main.

Follow the current owner-approved quota instruction in the development session.
The stable v0.2.0/v0.4.0/v0.6.0 milestones retain their qualification gates.

The bounded remote build service release gate runs Rust tests in the optimized profile
(`cargo test --locked --release`). GNU debug linking was observed to thrash at
the existing memory-high limit; resource caps stay unchanged. dPDF-only dev/test
profiles retain source-line debug information without full type DWARF.

The release profile overrides optimization level only for dPDF (`opt-level = 1`),
leaving dependencies at their existing defaults. The earlier optimization-2 comparison
reduced compiler peak by about 110 MiB, but later application growth again exceeded
the existing soft limit. Optimization 1 is being evaluated under the same caps. Generated headless reader
measurements retain fast first-page delivery; see the
[profile comparison](validation/release-profile-memory.md). These measurements
are local observations, not guaranteed build or GUI timings.

The Windows icon is generated from the existing `ui/assets/dpdf.svg` with
`scripts/generate-icon.py` using an already installed developer ImageMagick tool.
It contains six PNG-compressed sizes (16–256 px) with metadata stripped. This is a
developer asset step; runtime staging and the application never fetch/run that
converter. Native Windows regression checks load the resulting ICO through the
system icon API.
