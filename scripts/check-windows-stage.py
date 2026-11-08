#!/usr/bin/env python3
"""Stage an existing Windows build and probe it from an unrelated working folder."""
import argparse
import hashlib
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--pdfium', type=Path, required=True)
    args = parser.parse_args()
    if os.name != 'nt':
        raise RuntimeError('This staging qualification requires native Windows')
    source = Path(__file__).resolve().parent.parent
    spec = importlib.util.spec_from_file_location('sample', source / 'scripts' / 'make-sample.py')
    sample = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(sample)
    with tempfile.TemporaryDirectory(prefix='dpdf-staging-') as directory:
        root = Path(directory)
        staged = root / 'package with spaces'
        subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
                        '-File', str(source / 'packaging' / 'windows' / 'stage.ps1'),
                        '-BinaryPath', str(args.binary.resolve()),
                        '-PdfiumDistribution', str(args.pdfium.resolve().parent.parent),
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
                    'licenses/rust/inventory.json']
        assert all((staged / name).is_file() for name in expected), expected
        def digest(path):
            return hashlib.sha256(path.read_bytes()).digest()
        assert digest(staged / 'dpdf.exe') == digest(args.binary)
        assert digest(staged / 'pdfium.dll') == digest(args.pdfium)
        assert digest(staged / 'LICENSE') == digest(source / 'LICENSE')
        unrelated = root / 'unrelated working directory'
        unrelated.mkdir()
        fixture = unrelated / 'contract.pdf'
        fixture.write_bytes(sample.make_pdf(50))
        env = {key: value for key, value in os.environ.items()
               if not key.startswith(('DPDF_', 'SLINT_'))}
        env.update(DPDF_CONFIG_DIR=str(root / 'config'), DPDF_DATA_DIR=str(root / 'data'))
        # Adjacent loading must succeed without either override or a PDFium PATH entry.
        env['PATH'] = str(Path(os.environ['SystemRoot']) / 'System32')
        probe = subprocess.run([str(staged / 'dpdf.exe'), '--probe', str(fixture)], cwd=unrelated,
                               env=env, check=True, text=True, capture_output=True, timeout=60)
        assert 'pages=50 ' in probe.stdout and 'rgba_checksum=' in probe.stdout, probe.stdout
        version = subprocess.check_output([str(staged / 'dpdf.exe'), '--version'], cwd=unrelated,
                                          env=env, text=True, timeout=20)
        expected_version = tomllib.loads((source / 'Cargo.toml').read_text())['package']['version']
        assert version.startswith('dPDF ' + expected_version + ' '), version
        print('PASS: staged custom-target binary and notices; adjacent PDFium loads from unrelated directory without runtime overrides')


if __name__ == '__main__':
    main()
