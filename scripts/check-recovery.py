#!/usr/bin/env python3
"""Crash/restart local draft recovery against generated PDFs on a private display."""
import argparse
import hashlib
import importlib.util
import os
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
    private = module('private_display', 'private-display.py')
    with tempfile.TemporaryDirectory(prefix='dpdf-recovery-') as directory:
        folder = Path(directory)
        with private.display(folder, args.xvfb) as display_env:
            for scenario in ('changed', 'missing', 'discard', 'clear'):
                case = folder / scenario
                case.mkdir()
                source = case / 'contract.pdf'
                source.write_bytes(sample.make_pdf(50))
                data = case / 'data'
                output = case / 'recovered-copy.pdf'
                env = dict(display_env, SLINT_BACKEND='winit-software',
                           DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()),
                           DPDF_QPDF_BINARY=str(args.qpdf.resolve()),
                           DPDF_DATA_DIR=str(data), DPDF_SMOKE_RECOVERY='write',
                           DPDF_SMOKE_PLACEMENT_IMAGE=str(Path(__file__).with_name('fixtures').joinpath('page-photo.jpg').resolve()),
                           DPDF_SMOKE_RECOVERY_OUTPUT=str(output))
                for key in ('DPDF_SMOKE_CLIPBOARD', 'DPDF_SMOKE_PREFERENCES', 'DPDF_SMOKE_ASSEMBLY_SOURCE',
                            'DPDF_SMOKE_PLACEMENT_SNAPSHOT', 'DPDF_SMOKE_SNAPSHOT'):
                    env.pop(key, None)

                def run(phase):
                    env['DPDF_SMOKE_RECOVERY'] = phase
                    result = subprocess.run([str(args.binary.resolve()), '--smoke-ui', str(source)],
                                            env=env, capture_output=True, text=True, timeout=45)
                    if result.returncode:
                        raise RuntimeError(result.stdout + result.stderr)
                    return result.stdout

                assert 'durable checkpoint written' in run('write')
                drafts = list((data / 'recovery').glob('*.dpdf'))
                assert len(drafts) == 1
                if scenario == 'changed':
                    source.write_bytes(sample.make_pdf(3))
                if scenario == 'missing':
                    source.unlink()
                before = hashlib.sha256(source.read_bytes()).hexdigest() if source.exists() else None
                # An unsupported file cannot hide a valid draft or be deleted as collateral.
                unknown = data / 'recovery' / 'future.dpdf'
                unknown.write_bytes(b'future or unsupported recovery data')
                if scenario == 'clear':
                    # Explicit "delete all" removes every draft, including unsupported ones.
                    assert 'delete-all removed them while off and on' in run('clear')
                    remaining = sorted(p.name for p in (data / 'recovery').iterdir())
                    # Only the lock remains: drafts and the crashed session's nonce folder are gone.
                    assert remaining == ['recovery.lock'], remaining
                    assert (hashlib.sha256(source.read_bytes()).hexdigest()) == before
                    print('PASS: recovery can be disabled without deleting drafts and cleared on request')
                    continue
                report = run('discard' if scenario == 'discard' else 'read')
                assert ('explicit discard removed' if scenario == 'discard' else 'saved recovered copy') in report
                assert [p for p in (data / 'recovery').glob('*.dpdf') if p != unknown] == []
                assert unknown.read_bytes() == b'future or unsupported recovery data'
                assert (hashlib.sha256(source.read_bytes()).hexdigest() if source.exists() else None) == before
                if scenario != 'discard':
                    result = subprocess.run([str(args.qpdf.resolve()), '--show-npages', str(output)],
                                            capture_output=True, text=True, check=True)
                    assert result.stdout.strip() == '50'
                    subprocess.run([str(args.qpdf.resolve()), '--check', str(output)], capture_output=True, check=True)
                print(f'PASS: interrupted-process recovery / {scenario} source, marks and source protection')


if __name__ == '__main__':
    main()
