# Fedora RPM and COPR releases

The targets are Fedora 44 and 45 x86_64 in the public
[zvegelj/dpdf COPR project](https://copr.fedorainfracloud.org/coprs/zvegelj/dpdf/).
The current source-built preview includes the compact native CUPS printing work
and has been installed and tested from the real COPR feed on both targets. See
[the current targeted build and validation record](validation/fedora-copr-v0.1.49.md).
The unchanged runtime's [full application/printing baseline](validation/fedora-copr-v0.1.48.md)
remains explicitly recorded for v0.1.48.
The [initial Fedora 44 record](validation/fedora-copr-v0.1.45.md) preserves the
earlier packaging and upgrade evidence.

On Fedora 44 or 45 x86_64:

```sh
sudo dnf copr enable zvegelj/dpdf
sudo dnf install dpdf
```

The RPM installs the launcher, application and pinned PDFium runtime, desktop
entry, icon and dependency notices. It depends on Fedora's qpdf and printing/UI
libraries and does not change PDF defaults or start services. The source revision
is installed at `/usr/share/dpdf/SOURCE-REVISION`.
The selected rfd file-dialog backend uses XDG Desktop Portal. The RPM requires
the portal broker and GTK fallback so Open/import/save dialogs also work on a
minimal desktop without a preinstalled portal backend.

## Build inputs

`.copr/Makefile` runs only in COPR's source-preparation sandbox. It invokes
`scripts/build-srpm.py`, which takes a committed product tree, reads its existing
version, vendors the locked Cargo sources and gathers pinned PDFium source snapshots.
The SRPM includes those inputs and the exact source revision. The PDFium source
manifest pins the native engine and its dependency sources. Fedora's maintained
`gn` package generates the build, and Fedora's Clang compiles PDFium. No upstream
binary engine or toolchain download is used.
No product version is assigned by RPM tooling. An untagged packaging preview
uses a `0.<commit-time>.git<source>` RPM release suffix, orders newer snapshots
after older ones, and never claims to be a tagged release.
Compilation happens inside the Fedora buildroot with
`cargo build --frozen`; buildroot networking remains disabled.

The source assembler can also run with an isolated download cache:

```sh
python3 scripts/build-srpm.py --outdir /path/to/srpms --cargo-home /path/to/download-cache
```

It assembles sources, not application binaries. Never invoke COPR's package-install
Makefile on a source workstation. License selection and corresponding-source
availability are described in [LICENSING.md](LICENSING.md).

## Release automation

Private tags are submitted and monitored by the local operator using persistent
receipts; GitHub does not allocate an Ubuntu runner for the long COPR wait.
See [BUILD-INFRASTRUCTURE.md](BUILD-INFRASTRUCTURE.md) for the controlled worker,
qualified baseline, idempotent recovery and resource limits. `fedora-rpm.yml`
is a public-only opt-in relay (`DPDF_HOSTED_COPR=true`), not a second submitter.
The publisher requests both Fedora 44 and Fedora 45 x86_64 from one source RPM.
It checks each target's distribution/architecture separately and requires one
matching binary per target plus verified corresponding source before attachment.
COPR's rebuilt source RPM for each target is checked against the same source SHA;
a shared uploaded source RPM is also supported when target downloads omit sources.
An explicit `--chroot fedora-44-x86_64` or `--chroot fedora-45-x86_64` can select a
single target, including when resuming an older single-target receipt.
The local operator uses its existing protected COPR configuration. Only an opted-in
public hosted relay uses Actions secrets `COPR_LOGIN` and `COPR_TOKEN`; keep them
out of source, logs and build arguments. The publisher
builds the exact commit in COPR, waits for both target results, verifies RPM name,
version, distribution, architecture and embedded source revision in both the
binary and the source RPM's application archive, and writes
the RPM, source RPM, checksums and receipt to the operator output directory.
With separate scoped GitHub API authority, `--attach` also attaches those verified
artifacts to the existing GitHub preview. COPR publication does not itself create
a GitHub Release page or prove attachment.
Windows MSI/portable release jobs remain independent.
Actions keeps only the small publication receipt and checksum file as recovery
artifacts for seven days; packages remain on COPR and are attached to the release.
It does not duplicate the large SRPM in the Actions artifact allowance.

Qualified `v0.1.x` development releases should use this publication lane routinely;
they do not wait for stable `v0.2.0` acceptance or unrelated feature completion.
Required CI, source/license validation and the existing release-authority locks
still apply. A source contributor merges through approved CI, and the designated
release authority publishes through the normal version controller and scripts.

Local publication uses the existing COPR client configuration:

```sh
python3 scripts/publish-copr.py --tag "$(git describe --exact-match --tags HEAD)" \
  --output /path/to/fedora-rpm --attach
```

Keep the output directory to resume the recorded build without creating another.
`--build-id` can resume an existing build; the downloaded RPM must still match the
checked-out release's version and source revision. Pending or failed builds never
produce a successful publication claim. Physical printing, native desktop and
stable-milestone acceptance remain separate gates.

## Untagged development previews

An owner-authorized COPR development preview is separate from merging its pull
request or publishing a numbered GitHub patch release. Build a reviewed committed
tree with `build-srpm.py` after the local application, source/privacy and license
checks pass. Its existing manifest version and commit-based RPM release suffix
identify the snapshot; do not assign a new product version outside the release
controller. A hosted PR dispatch outage does not prohibit this preview build.
Application test failures and source/license failures remain gates.

Record the real COPR build, target, RPM identity, source SHA and disposable
installation results before presenting it as installable. Required PR checks
still apply before merging, and the release authority keeps the normal patch/tag
workflow. Never attach an untagged snapshot to a different existing release tag.
The tagged publisher intentionally requires the exact checked-out release tag.

## Public-source privacy

Repository visibility stays private until the owner changes it. COPR source RPMs
are public too: inspect a sanitized product export before submitting one.
`scripts/check-public-export.py` takes a private JSON regex denylist supplied
outside tracked source, and reports only matching paths and line numbers:

```sh
python3 scripts/check-public-export.py --patterns-file /private/publication-patterns.json
```

Machine-specific build commands and transport targets belong in ignored local
configuration. Historical product notes use explicit `[private]` placeholders
where personal machine or user details were removed. This working-tree cleanup
does not rewrite older Git commits, existing tags, remote validation branches or
already published release assets. Those remain private-review items before the
owner makes the repository public; shared history must not be force-rewritten
as a side effect of packaging.
