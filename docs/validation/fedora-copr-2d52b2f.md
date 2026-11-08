# Fedora 44 and 45 development preview

Published and tested on 2026-11-06. Source:
`2d52b2fe303d1e5a1ea8427c3e6fbb62c5ecbb35`.
The manifest remains `0.1.45`; the existing commit-based RPM release identifies
this new development snapshot. GitHub tag `v0.1.45` identifies the earlier
`d8425df31fe9ddd525e1079a1aa7622103bd1307` release.

| Target | Successful COPR build | Installed RPM |
| --- | --- | --- |
| Fedora 44 x86_64 | [11268483](https://copr.fedorainfracloud.org/coprs/build/11268483) | `dpdf-0.1.45-0.1793917085.git2d52b2fe303d.fc44.x86_64` |
| Fedora 45 x86_64 | [11268490](https://copr.fedorainfracloud.org/coprs/build/11268490) | `dpdf-0.1.45-0.1793917085.git2d52b2fe303d.fc45.x86_64` |

Actual build logs: [Fedora 44 directory](https://download.copr.fedorainfracloud.org/results/zvegelj/dpdf/fedora-44-x86_64/11268483-dpdf/),
[Fedora 45 log](https://download.copr.fedorainfracloud.org/results/zvegelj/dpdf/fedora-45-x86_64/11268490-dpdf/builder-live.log.gz).
Both builds compiled the pinned PDFium 7881 engine from source (1121 native steps),
then the locked Rust application with buildroot networking disabled. System qpdf
is used. The selected full-application license remains GPL-3.0-only, owned source
Apache-2.0, with engine/compiler/Rust dependency notices retained.

## Install or update

On Fedora 44 or 45 x86_64:

```sh
sudo dnf copr enable zvegelj/dpdf
sudo dnf install dpdf
```

For an existing RPM installation:

```sh
sudo dnf upgrade --refresh dpdf
```

Launch `/usr/bin/dpdf` to select the RPM installation explicitly when development
launchers also exist. `/usr/share/dpdf/SOURCE-REVISION` contains the source above.

## Installed-package qualification

Separate disposable Fedora targets installed the actual signed COPR packages
through DNF. Tests used dedicated non-root accounts and generated documents.
Each target passed **34 application checks**: launcher/runtime discovery,
render/search/selection, page assembly, geometry and physical units, marks,
overlays, review, aliases, independent Poppler checks, clipboard/window behavior
at scales 1/1.5/2, passwords, recovery and controlled native file printing.

Each also passed **nine loopback-only CUPS cases**: all/single/contiguous selection,
disjoint vector selection, pathname mutation, cancellation, HTTP refusal,
HTTP-200 IPP refusal and forced subset-preparation failure with renderer fallback.

| Disjoint pages 1,7,8,9,50 | Client spool bytes | Receiver PDF bytes | Receiver pages | Text retained | Raster images |
| --- | ---: | ---: | ---: | --- | ---: |
| Fedora 44 | 3,133 | 4,075 | 5 | Yes | 0 |
| Fedora 45 | 3,133 | 13,142 | 5 | Yes | 0 |

The earlier renderer spool was 507,204 bytes: the vector subset is 99.4% smaller.
Both servers retained the selected text and document order; receiver sizes differ
between distribution filter stacks. No job was produced for cancellation or
controlled refusal. Preparation failure fell back before sending; submission
errors never triggered duplicate automatic retries.

The Fedora 44 feed upgrade preserved all eight seeded user files byte-for-byte.
Uninstall/reinstall preserved eight files on Fedora 44 and seven on Fedora 45,
including preferences, synthetic mark libraries and the existing PDF association.
Package launch/menu files disappeared on removal and the same package returned
on reinstall. The actual installed desktop entry and Open portal chooser passed
on both targets; the prior PDF viewer remained the default.

## Artifact identity

Binary RPM sizes and SHA256:

- Fedora 44: 11,321,769 bytes,
  `f0859b845a1c7934825c9b1cf224ab403b9076e9ac2e0283771c16daea1a8242`.
- Fedora 45: 11,353,618 bytes,
  `609057e5505352029317495722800f2f2668a7a5af43648d825bacc58668fe20`.

Rebuilt source RPM sizes and SHA256:

- Fedora 44: 234,006,550 bytes,
  `b108f8f940800666bb4416c468029a8027cb6715d7f7431b71ea00220df14a37`.
- Fedora 45: 234,006,556 bytes,
  `b1a0ee4fc4dd458469fd65fe5726d2abe016c6c7c23c4d67d83015acda4c29c5`.

The publisher verified both real binary/source RPM pairs against their target
distributions and embedded source revision. COPR target downloads include rebuilt
SRPMs as well as binaries; each source scope is checked independently. Ten
packaging contracts pass, including missing, duplicate and misrouted artifact
refusals. The tagged release publisher now requests both Fedora targets and keeps
only small recovery receipts/checksums in Actions artifacts.

## Qualification limits

This is an owner-authorized untagged development preview. Required PR merge and
numbered-release gates remain in place. Physical printers, real ruler/office
acceptance, Wayland and native Windows hardware are unqualified here. Production
Secure Signature Block remains disabled. Repository visibility remains private
under the owner's instruction; COPR contains the reviewed current source export.
