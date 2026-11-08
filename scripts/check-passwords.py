#!/usr/bin/env python3
"""Generated protected-PDF reader/retry/edit checks; passwords never enter command arguments."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/dpdf'))
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    parser.add_argument('--xvfb', default='Xvfb')
    args = parser.parse_args()
    sample = module('sample', 'make-sample.py')
    private = module('private', 'private-display.py')
    binary, qpdf = str(args.binary.resolve()), str(args.qpdf.resolve())
    with tempfile.TemporaryDirectory(prefix='dpdf-passwords-') as directory:
        folder = Path(directory)
        plain = folder / 'plain.pdf'
        plain.write_bytes(sample.make_pdf(50))
        with private.display(folder, args.xvfb) as display_env:
            def encrypted(name, password, bits):
                output = folder / (name + '.pdf')
                job = {'inputFile': str(plain), 'outputFile': str(output),
                       'encrypt': {'userPassword': password, 'ownerPassword': 'synthetic-owner-' + password,
                                   f'{bits}bit': ({'useAes': 'y'} if bits == 128 else {})}}
                job_path = folder / 'synthetic-encryption-job.json'
                job_path.write_text(json.dumps(job), encoding='utf-8')
                job_path.chmod(0o600)
                try:
                    subprocess.run([qpdf, '--job-json-file=' + str(job_path)], capture_output=True, check=True)
                finally:
                    job_path.unlink()
                return output

            for name, password, bits in (('AES256', 'synthetic test value', 256),
                                         ('Unicode spaces Žig', '  Žig čšž "$` fixture  ', 256),
                                         ('AES128', 'legacy test fixture', 128)):
                source = encrypted(name, password, bits)
                before = source.read_bytes()
                copy = folder / (name + '-unchanged.pdf')
                edited = folder / (name + '-marked.pdf')
                env = dict(display_env, SLINT_BACKEND='winit-software',
                           DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()), DPDF_QPDF_BINARY=qpdf,
                           DPDF_DATA_DIR=str(folder / 'data'), DPDF_CONFIG_DIR=str(folder / 'config'),
                           DPDF_SMOKE_PASSWORD='unlock', DPDF_SMOKE_PASSWORD_VALUE=password,
                           DPDF_SMOKE_PASSWORD_COPY=str(copy), DPDF_SMOKE_PASSWORD_EDITED_OUTPUT=str(edited),
                           DPDF_SMOKE_PLACEMENT_IMAGE=str(Path(__file__).with_name('fixtures').joinpath('page-photo.jpg').resolve()))
                for key in ('DPDF_SMOKE_CLIPBOARD', 'DPDF_SMOKE_PREFERENCES', 'DPDF_SMOKE_ASSEMBLY_SOURCE', 'DPDF_SMOKE_RECOVERY'):
                    env.pop(key, None)

                def run(mode):
                    env['DPDF_SMOKE_PASSWORD'] = mode
                    result = subprocess.run([binary, '--smoke-ui', str(source)], env=env,
                                            capture_output=True, text=True, timeout=45)
                    if result.returncode:
                        raise RuntimeError(result.stdout + result.stderr)
                    assert password not in result.stdout + result.stderr
                    return result.stdout

                assert 'incorrect-password retry succeeded' in run('unlock')
                assert copy.read_bytes() == before
                assert subprocess.run([qpdf, '--requires-password', str(copy)], capture_output=True).returncode == 0
                assert subprocess.run([qpdf, '--is-encrypted', str(edited)], capture_output=True).returncode == 2
                count = subprocess.run([qpdf, '--show-npages', str(edited)], capture_output=True, text=True, check=True)
                assert count.stdout.strip() == '50'
                subprocess.run([qpdf, '--check', str(edited)], capture_output=True, check=True)
                assert source.read_bytes() == before
                print(f'PASS: {bits}-bit password retry, reader/search/placement, exact encrypted Save As and edited copy')
                if bits == 128:
                    continue
                assert 'Escape cancelled opening' in run('cancel')
                second = encrypted(name + '-queued', password, bits)
                env['DPDF_SMOKE_PASSWORD_SECOND'] = str(second)
                assert 'queued prompts retained in-progress text' in run('queue')
                assert hashlib.sha256(source.read_bytes()).digest() == hashlib.sha256(before).digest()
                print('PASS: password cancellation and queued prompts preserve source and in-progress entry')
                if name == 'AES256':
                    env['DPDF_SMOKE_RECOVERY'] = 'write'
                    env['DPDF_SMOKE_RECOVERY_OUTPUT'] = str(folder / 'protected-recovered.pdf')
                    assert 'durable checkpoint written' in run('unlock')
                    drafts = list((folder / 'data' / 'recovery').glob('*.dpdf'))
                    assert len(drafts) == 1
                    assert password.encode() not in drafts[0].read_bytes()
                    env['DPDF_SMOKE_RECOVERY'] = 'read'
                    assert 'saved recovered copy' in run('unlock')
                    assert not list((folder / 'data' / 'recovery').glob('*.dpdf'))
                    assert source.read_bytes() == before
                    print('PASS: protected draft restart requires password re-entry, exports safely and stores no password')
                    env.pop('DPDF_SMOKE_RECOVERY')



if __name__ == '__main__':
    main()
