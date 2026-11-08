#!/usr/bin/env python3
"""Check explicit/bundled engine discovery using generated PDFs and isolated copies."""
import argparse
import ctypes.util
import hashlib
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--pdfium', type=Path, required=True)
    args = parser.parse_args()
    binary, library = args.binary.resolve(), args.pdfium.resolve()
    spec = importlib.util.spec_from_file_location('engine_sample', Path(__file__).with_name('make-sample.py'))
    sample = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(sample)
    with tempfile.TemporaryDirectory(prefix='dpdf-engine-paths-') as directory:
        root = Path(directory)
        clean, documents, trusted, bundled = [root / name for name in
                                             ('clean executable', 'document folder Ω', 'trusted library Ω', 'bundled')]
        for folder in (clean, documents, trusted, bundled):
            folder.mkdir()
        name = 'dpdf.exe' if os.name == 'nt' else 'dpdf'
        bare, adjacent = clean/name, bundled/name
        shutil.copy2(binary, bare)
        shutil.copy2(binary, adjacent)
        engine_name = 'pdfium.dll' if os.name == 'nt' else 'libpdfium.so'
        shutil.copy2(library, trusted/engine_name)
        shutil.copy2(library, bundled/engine_name)
        shutil.copy2(library, documents/engine_name)
        fixture = documents/'contract.pdf'
        fixture.write_bytes(sample.make_pdf(50))
        source_digest = hashlib.sha256(fixture.read_bytes()).digest()
        env = {key:value for key,value in os.environ.items() if not key.startswith(('DPDF_', 'SLINT_'))}
        env.update(DPDF_CONFIG_DIR=str(root/'config'), DPDF_DATA_DIR=str(root/'data'))
        if os.name == 'nt':
            env['PATH'] = str(Path(os.environ['SystemRoot'])/'System32')

        def probe(executable, cwd, overrides, success, marker=''):
            result = subprocess.run([str(executable), '--probe', str(fixture)], cwd=cwd,
                                    env={**env, **overrides}, capture_output=True, text=True,
                                    encoding='utf-8', errors='replace', timeout=60)
            if success:
                if result.returncode or 'pages=50 ' not in result.stdout or 'rgba_checksum=' not in result.stdout:
                    raise RuntimeError('Configured engine did not load: '+result.stderr)
            elif result.returncode == 0 or marker not in result.stderr:
                raise RuntimeError('Unconfigured engine search was not refused: '+result.stdout+result.stderr)

        # A valid, harmless engine in the document directory would make the old
        # relative override load successfully; the new policy must reject it.
        probe(bare, documents, {'DPDF_PDFIUM_LIBRARY':'./'+engine_name}, False, 'absolute path')
        probe(bare, documents, {'DPDF_PDFIUM_LIBRARY':str(trusted/engine_name)}, True)
        probe(bare, clean, {'DPDF_PDFIUM_LIBRARY':str(root/'missing'/engine_name)}, False, 'DPDF_PDFIUM_LIBRARY')
        if os.name != 'nt' and not ctypes.util.find_library('pdfium'):
            probe(bare, clean, {}, False, 'PDFium is unavailable')
        (documents/engine_name).write_bytes(b'not a library')
        probe(adjacent, documents, {}, True)
        if os.name == 'nt':
            shutil.copy2(library, documents/engine_name)
            probe(bare, documents, {}, False, 'On Windows, place pdfium.dll beside dPDF')
            probe(bare, clean, {'PATH':str(documents)+os.pathsep+env['PATH']}, False,
                  'On Windows, place pdfium.dll beside dPDF')
        if hashlib.sha256(fixture.read_bytes()).digest() != source_digest:
            raise RuntimeError('Engine discovery changed the generated PDF')
        print('PASS: absolute engine override and adjacent runtime; relative and missing engines refused clearly')
        if os.name == 'nt':
            print('PASS: valid PDFium decoys in document directory and PATH are not loaded without configuration')


if __name__ == '__main__':
    main()
