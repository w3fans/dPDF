#!/usr/bin/env python3
"""Qualify adjacent PDFium/qpdf in a complete generated Windows portable bundle."""
import argparse
import hashlib
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    args = parser.parse_args()
    if os.name != 'nt':
        raise RuntimeError('This staging qualification requires native Windows')
    source = Path(__file__).resolve().parent.parent
    spec = importlib.util.spec_from_file_location('sample', source / 'scripts' / 'make-sample.py')
    sample = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(sample)
    with tempfile.TemporaryDirectory(prefix='dpdf-staging-') as directory:
        root = Path(directory)
        staged = root / 'portable Ž Ω with spaces'
        subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
                        '-File', str(source / 'packaging' / 'windows' / 'stage.ps1'),
                        '-BinaryPath', str(args.binary.resolve()),
                        '-PdfiumDistribution', str(args.pdfium.resolve().parent.parent),
                        '-QpdfBin', str(args.qpdf.resolve().parent),
                        '-OutputDirectory', str(staged)], check=True, timeout=90)
        import json
        inventory = json.loads((staged / 'licenses' / 'rust' / 'inventory.json').read_text())
        assert inventory['target'] == 'x86_64-pc-windows-msvc' and not inventory['missing_license_text']
        for component in inventory['components']:
            for notice in component['notices']:
                assert hashlib.sha256((staged / 'licenses' / 'rust' / notice['text']).read_bytes()).hexdigest() == notice['sha256']
        expected = ['register.ps1', 'dpdf.ico', 'LICENSE', 'NOTICE', 'README.md', 'licenses/pdfium/LICENSE',
                    'licenses/slint/LicenseRef-Slint-Royalty-free-2.0.md',
                    'licenses/slint/GPL-3.0-only.txt', 'docs/LICENSING.md',
                    'licenses/rust/inventory.json', 'qpdf.exe', 'qpdf30.dll',
                    'licenses/qpdf/NOTICE.md']
        assert all((staged / name).is_file() for name in expected), expected
        def digest(path):
            return hashlib.sha256(path.read_bytes()).digest()
        assert digest(staged / 'dpdf.exe') == digest(args.binary)
        assert digest(staged / 'pdfium.dll') == digest(args.pdfium)
        assert digest(staged / 'LICENSE') == digest(source / 'LICENSE')
        qpdf = args.qpdf.resolve()
        assert digest(staged / 'qpdf.exe') == digest(qpdf)
        for library in qpdf.parent.glob('*.dll'):
            assert digest(staged / library.name) == digest(library), library.name
        unrelated = root / 'unrelated Ω working directory'
        unrelated.mkdir()
        fixture = unrelated / 'generated Ž contract.pdf'
        fixture.write_bytes(sample.make_pdf(50))
        source_digest = digest(fixture)
        env = {key: value for key, value in os.environ.items()
               if not key.startswith(('DPDF_', 'SLINT_'))}
        env.update(DPDF_CONFIG_DIR=str(root / 'config'), DPDF_DATA_DIR=str(root / 'data'))
        # Adjacent loading must succeed without either override or a PDFium PATH entry.
        env['PATH'] = str(Path(os.environ['SystemRoot']) / 'System32')
        # Bare-name CreateProcess can also search these locations without PATH.
        # Refuse an unsuitable fixture host instead of claiming bundled-helper
        # absence while silently testing a system installation.
        system = Path(os.environ['SystemRoot'])
        assert not any((folder / 'qpdf.exe').exists() for folder in
                       (system, system / 'System32', system / 'System')), 'Require no qpdf in Windows system search directories'
        probe = subprocess.run([str(staged / 'dpdf.exe'), '--probe', str(fixture)], cwd=unrelated,
                               env=env, check=True, text=True, encoding='utf-8', errors='replace',
                               capture_output=True, timeout=60)
        assert 'pages=50 ' in probe.stdout and 'rgba_checksum=' in probe.stdout, probe.stdout
        version = subprocess.check_output([str(staged / 'dpdf.exe'), '--version'], cwd=unrelated,
                                          env=env, text=True, timeout=20)
        expected_version = tomllib.loads((source / 'Cargo.toml').read_text())['package']['version']
        assert version.startswith('dPDF ' + expected_version + ' '), version
        extracted = unrelated / 'extracted Ž pages.pdf'
        assemble = [str(staged / 'dpdf.exe'), '--assemble', 'extract', str(fixture),
                    str(extracted), '1-3', '']
        subprocess.run(assemble, cwd=unrelated, env=env, check=True,
                       capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=60)
        result = subprocess.run([str(staged / 'dpdf.exe'), '--probe', str(extracted)],
                                cwd=unrelated, env=env, check=True, capture_output=True,
                                text=True, encoding='utf-8', errors='replace', timeout=60)
        assert 'pages=3 ' in result.stdout, result.stdout
        output_digest = digest(extracted)
        bundled_qpdf = staged / 'qpdf.exe'
        # Mutate only the temporary bundle. A missing helper must not be found
        # through a developer override/PATH, or destroy an existing destination.
        try:
            bundled_qpdf.unlink()
            for invalid in (False, True):
                if invalid:
                    bundled_qpdf.write_bytes(b'generated invalid executable')
                failure = subprocess.run(assemble, cwd=unrelated, env=env,
                                         capture_output=True, text=True, encoding='utf-8',
                                         errors='replace', timeout=60)
                assert failure.returncode != 0 and 'qpdf' in failure.stderr.lower(), failure.stderr
                assert digest(extracted) == output_digest, 'Failed helper replaced the destination'
                assert digest(fixture) == source_digest, 'Failed helper changed the source'
        finally:
            shutil.copy2(qpdf, bundled_qpdf)
        assert digest(fixture) == source_digest, 'Portable assembly changed the source'
        assert digest(bundled_qpdf) == digest(qpdf)
        receipt = dict(schema=1, scope='generated portable runtime; not MSI/operator acceptance',
                       binary_sha256=digest(staged / 'dpdf.exe').hex(),
                       pdfium_sha256=digest(staged / 'pdfium.dll').hex(),
                       qpdf_sha256=digest(bundled_qpdf).hex(),
                       qpdf_dll_sha256={library.name: digest(staged / library.name).hex()
                                        for library in sorted(qpdf.parent.glob('*.dll'))},
                       rust_notice_inventory_sha256=digest(staged / 'licenses/rust/inventory.json').hex(),
                       source_sha256=source_digest.hex(), output_sha256=output_digest.hex(),
                       extracted_pages=3, failed_helper_cases=['missing', 'invalid executable'],
                       source_and_destination_preserved=True)
        print('portable-runtime-receipt: ' + json.dumps(receipt, sort_keys=True))
        print('PASS: complete portable runtime/notices and adjacent PDFium/qpdf from Unicode paths without runtime overrides')
        print('PASS: missing/invalid bundled qpdf fail visibly and preserve source and pre-existing destination')


if __name__ == '__main__':
    main()
