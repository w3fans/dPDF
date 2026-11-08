#!/usr/bin/env bash
# Run only inside the Fedora buildroot, using Fedora compilers and packaged GN.
set -euo pipefail
source_tree="$(realpath "${1:?PDFium source directory}")"
gn="$(command -v gn)"
"$gn" --version
cd "$source_tree"
python3 - <<'PY'
from pathlib import Path
import subprocess
p = Path('BUILD.gn')
text = p.read_text()
assert text.count('component("pdfium") {') == 1
p.write_text(text.replace('component("pdfium") {', 'shared_library("pdfium") {'))
p = Path('public/fpdfview.h')
text = p.read_text()
assert text.count('#if defined(COMPONENT_BUILD)') == 1
p.write_text(text.replace('#if defined(COMPONENT_BUILD)', '#if 1  // Export the standalone shared PDFium API.'))
Path('build/config/gclient_args.gni').write_text('''android_ndk_version = ""
build_with_chromium = false
checkout_android = false
checkout_skia = false
''')
# Chromium's bundled-toolchain defaults use a different LLVM version/triple.
# Resolve compiler-rt through the installed Fedora compiler, never a download.
resource = Path(subprocess.check_output(['clang', '-print-resource-dir'], text=True).strip()).resolve()
builtins = Path(subprocess.check_output(['clang', '--rtlib=compiler-rt', '--print-libgcc-file-name'], text=True).strip()).resolve()
assert resource.name.isdigit() and builtins.is_file(), 'Install Fedora compiler-rt for the selected clang'
assert builtins.name == 'libclang_rt.builtins.a' and builtins.is_relative_to(resource)
p = Path('build/config/clang/BUILD.gn')
text = p.read_text()
old = '_clang_lib_dir = "$clang_base_path/lib/clang/$clang_version/lib"'
assert text.count(old) == 1
text = text.replace(old, '_clang_lib_dir = "' + (resource / 'lib').as_posix() + '"')
old = '_dir = "x86_64-unknown-linux-gnu"'
assert text.count(old) == 1
p.write_text(text.replace(old, '_dir = "' + builtins.parent.name + '"'))
Path('dpdf-toolchain.gni').write_text('clang_version = "' + resource.name + '"\n')
# The Chromium toolchain is newer/customized. Keep real sanitizer/trap flags;
# adapt only unsupported diagnostics and feature-reporting controls. Preserve
# the conservative lifetime-DSE intent through Clang's supported frontend flag.
def supports(*flags):
    return subprocess.run(['clang++', '-Werror', '-x', 'c++', '-fsyntax-only', *flags, '-'],
                          input=b'', stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
p = Path('build/config/compiler/BUILD.gn')
text = p.read_text()
if not supports('-fdiagnostics-show-inlining-chain'):
    old = 'cflags += [ "-fdiagnostics-show-inlining-chain" ]'
    assert text.count(old) == 1
    text = text.replace(old, '# Inlining-chain diagnostics unavailable in the distro compiler.')
if not supports('-fno-lifetime-dse'):
    assert supports('-Xclang', '-disable-lifetime-markers')
    old = 'cflags += [ "-fno-lifetime-dse" ]'
    assert text.count(old) == 1
    text = text.replace(old, 'cflags += [ "-Xclang", "-disable-lifetime-markers" ]')
p.write_text(text)
if not supports('-fsanitize-ignore-for-ubsan-feature=array-bounds'):
    p = Path('build/config/sanitizers/sanitizers.gni')
    text = p.read_text()
    old = '"-fsanitize-ignore-for-ubsan-feature=${invoker.sanitizer}",'
    assert text.count(old) == 1
    p.write_text(text.replace(old, '# Selected sanitizer and trap flags remain enabled.'))
PY
mkdir -p out/dpdf
cat > out/dpdf/args.gn <<'EOF'
is_debug = false
symbol_level = 0
target_os = "linux"
target_cpu = "x64"
# Upstream defines this solely as enabling its test-binary graph. The RPM
# builds the shared library and runs dPDF's engine acceptance checks separately.
pdf_is_standalone = false
is_component_build = false
pdf_use_partition_alloc = false
pdf_enable_v8 = false
pdf_enable_xfa = false
pdf_use_skia = false
pdf_enable_fontations = false
pdf_enable_rust_png = false
enable_rust = false
clang_base_path = "/usr"
clang_use_chrome_plugins = false
use_sysroot = false
use_glib = false
use_custom_libcxx = false
use_custom_libcxx_for_host = false
use_lld = true
use_remoteexec = false
use_siso = false
treat_warnings_as_errors = false
EOF
cat dpdf-toolchain.gni >> out/dpdf/args.gn
"$gn" gen out/dpdf
# Configuration-only checks run in a disposable Fedora root before COPR upload.
if [[ "${2:-}" == "--configure-only" ]]; then exit 0; fi
ninja -C out/dpdf -j 1 pdfium
test -f out/dpdf/libpdfium.so
