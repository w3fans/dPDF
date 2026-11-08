# COPR upstream preview package. Sources and locked Cargo dependencies are in SRPM.
%global debug_package %{nil}
%global toolchain clang
%global __provides_exclude_from ^%{_libexecdir}/dpdf/.*$
Name:           dpdf
Version:        @VERSION@
Release:        @RELEASE@%{?dist}
Summary:        Fast native PDF reader and practical PDF workspace
License:        GPL-3.0-only AND Apache-2.0 AND BSD-3-Clause AND (Apache-2.0 WITH LLVM-exception)
URL:            https://github.com/w3fans/dPDF
Source0:        dpdf-%{version}.tar.gz
Source1:        dpdf-%{version}-vendor.tar.gz
Source2:        pdfium-7881-source.tar.gz
ExclusiveArch:  x86_64
BuildRequires:  cargo >= 1.92
BuildRequires:  rust >= 1.92
BuildRequires:  gcc-c++
BuildRequires:  clang
BuildRequires:  compiler-rt
BuildRequires:  lld
BuildRequires:  llvm
BuildRequires:  ninja-build
BuildRequires:  gn >= 2598
BuildRequires:  pkgconfig(fontconfig)
BuildRequires:  desktop-file-utils
BuildRequires:  python3
BuildRequires:  qpdf >= 11
Requires:       qpdf >= 11
Requires:       gtk3
Requires:       cups-client
Requires:       fontconfig
Requires:       libX11
Requires:       libXcursor
Requires:       libXi
Requires:       libXrandr
Requires:       libxcb
Requires:       libxkbcommon
Requires:       libxkbcommon-x11
Requires:       xdg-utils
Requires:       xdg-desktop-portal
Requires:       xdg-desktop-portal-gtk
Requires:       hicolor-icon-theme

%description
dPDF is a native PDF workspace with fast viewing, printing, tabs, search,
page replacement and assembly, visual signatures and independent review remarks.
This is a development preview, not a qualified stable milestone. Secure signature
block production export remains disabled. dPDF-owned source is Apache-2.0;
Slint uses its GPL-3.0-only option for this open-source application distribution.

%prep
%setup -q
tar -xzf %{SOURCE1}
tar -xzf %{SOURCE2}
cat >> .cargo/config.toml <<'EOF'

[source.crates-io]
replace-with = "vendored-sources"
[source.vendored-sources]
directory = "vendor"
EOF

%build
export CARGO_HOME="$PWD/.cargo-home"
# No network access or mutable dependency resolution in the RPM build.
bash scripts/build-pdfium-source.sh "$PWD/pdfium-source"
cargo build --frozen --release --jobs 1

%install
install -Dpm 0755 target/release/dpdf %{buildroot}%{_libexecdir}/dpdf/dpdf
install -Dpm 0755 pdfium-source/out/dpdf/libpdfium.so %{buildroot}%{_libexecdir}/dpdf/libpdfium.so
install -d %{buildroot}%{_bindir}
cat > %{buildroot}%{_bindir}/dpdf <<'EOF'
#!/bin/sh
exec /usr/libexec/dpdf/dpdf "$@"
EOF
chmod 0755 %{buildroot}%{_bindir}/dpdf
desktop-file-install --dir=%{buildroot}%{_datadir}/applications \
    --set-key=Exec --set-value='dpdf %%F' packaging/linux/net.dpdf.dPDF.desktop
install -Dpm 0644 ui/assets/dpdf.svg \
    %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/net.dpdf.dPDF.svg
install -Dpm 0644 SOURCE-REVISION %{buildroot}%{_datadir}/dpdf/SOURCE-REVISION
install -d %{buildroot}%{_licensedir}/dpdf/pdfium
cp -a pdfium-source/LICENSE pdfium-source/dpdf-source-notices/. %{buildroot}%{_licensedir}/dpdf/pdfium/
install -Dpm 0644 %{_licensedir}/compiler-rt/LICENSE.TXT \
    %{buildroot}%{_licensedir}/dpdf/compiler-rt/LICENSE.TXT

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/net.dpdf.dPDF.desktop
DPDF_PDFIUM_LIBRARY="$PWD/pdfium-source/out/dpdf/libpdfium.so" target/release/dpdf --version
PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-cargo-notices.py --target linux
PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-reader.py \
    --binary target/release/dpdf --pdfium "$PWD/pdfium-source/out/dpdf/libpdfium.so"

%files
%license LICENSE NOTICE packaging/licenses/slint packaging/licenses/rust
%license %{_licensedir}/dpdf/pdfium
%license %{_licensedir}/dpdf/compiler-rt
%doc README.md docs/LICENSING.md docs/MILESTONES.md
%{_bindir}/dpdf
%dir %{_libexecdir}/dpdf
%{_libexecdir}/dpdf/dpdf
%{_libexecdir}/dpdf/libpdfium.so
%{_datadir}/applications/net.dpdf.dPDF.desktop
%{_datadir}/icons/hicolor/scalable/apps/net.dpdf.dPDF.svg
%dir %{_datadir}/dpdf
%{_datadir}/dpdf/SOURCE-REVISION
