#!/usr/bin/env bash
# Stage an already-built runtime; compilation stays with ./build on remote build service.
set -euo pipefail
cd "$(dirname "$0")/.."
output="${1:-/app/dPDF}"
binary="${DPDF_BINARY:-target/release/dpdf}"
: "${DPDF_PDFIUM_DISTRIBUTION:?Set DPDF_PDFIUM_DISTRIBUTION to the extracted PDFium 7881 distribution}"
[[ -x "$binary" && -f "$DPDF_PDFIUM_DISTRIBUTION/lib/libpdfium.so" && -d "$DPDF_PDFIUM_DISTRIBUTION/licenses" ]] || {
    echo 'Build dPDF and provide a complete matching PDFium distribution first.' >&2; exit 2;
}
python3 scripts/check-cargo-notices.py --target linux
mkdir -p "$output/licenses/pdfium" "$output/share/applications" "$output/share/icons/hicolor/scalable/apps" "$output/docs"
install -m 755 "$binary" "$output/dpdf"
install -m 755 "$DPDF_PDFIUM_DISTRIBUTION/lib/libpdfium.so" "$output/libpdfium.so"
if [[ -n "${DPDF_QPDF_BINARY:-}" ]]; then
    : "${DPDF_QPDF_LICENSE_DIR:?Set DPDF_QPDF_LICENSE_DIR when bundling a qpdf helper}"
    [[ -x "$DPDF_QPDF_BINARY" && -d "$DPDF_QPDF_LICENSE_DIR" ]] || { echo 'Missing qpdf helper or license directory.' >&2; exit 2; }
    mkdir -p "$output/licenses/qpdf"
    install -m 755 "$DPDF_QPDF_BINARY" "$output/qpdf"
    cp -a "$DPDF_QPDF_LICENSE_DIR/." "$output/licenses/qpdf/"
fi
install -m 644 "$DPDF_PDFIUM_DISTRIBUTION/LICENSE" "$output/licenses/pdfium/LICENSE"
cp -a "$DPDF_PDFIUM_DISTRIBUTION/licenses/." "$output/licenses/pdfium/"
install -m 644 README.md "$output/"
install -m 644 LICENSE NOTICE "$output/"
mkdir -p "$output/licenses/slint"
cp -a packaging/licenses/slint/. "$output/licenses/slint/"
mkdir -p "$output/licenses/rust"
cp -a packaging/licenses/rust/texts "$output/licenses/rust/"
install -m 644 packaging/licenses/rust/inventory-linux.json "$output/licenses/rust/inventory.json"
install -m 644 packaging/licenses/rust/README.md "$output/licenses/rust/README.md"
cp -a docs/. "$output/docs/"
install -m 644 packaging/linux/net.dpdf.dPDF.desktop "$output/share/applications/"
install -m 755 scripts/install-desktop.py "$output/install-desktop.py"
install -m 644 ui/assets/dpdf.svg "$output/share/icons/hicolor/scalable/apps/net.dpdf.dPDF.svg"
printf 'Staged development runtime: %s\n' "$output"
