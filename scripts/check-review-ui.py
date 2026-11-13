#!/usr/bin/env python3
"""Graphical revision remarks, mixed signature export and interrupted-session recovery."""
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
    parser.add_argument('--scale', type=float, choices=[1, 1.5, 2],
                        help='Override fixture scale; otherwise retain the validation lane setting')
    args = parser.parse_args()
    sample, private = module('sample', 'make-sample.py'), module('display', 'private-display.py')
    binary, qpdf = str(args.binary.resolve()), str(args.qpdf.resolve())
    with tempfile.TemporaryDirectory(prefix='dpdf-review-window-') as directory:
        folder = Path(directory)
        with private.display(folder, args.xvfb) as display_env:
            for case_name in ('edit', 'mixed-restart', 'notes-restart'):
                case = folder / case_name
                case.mkdir()
                source = case / 'contract.pdf'
                source.write_bytes(sample.make_pdf(50))
                output = case / 'reviewed-copy.pdf'
                env = dict(display_env, SLINT_BACKEND='winit-software',
                           DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()), DPDF_QPDF_BINARY=qpdf,
                           DPDF_DATA_DIR=str(case / 'data'), DPDF_SMOKE_RECOVERY='review',
                           DPDF_SMOKE_REVIEW_OUTPUT=str(output),
                           DPDF_SMOKE_PLACEMENT_IMAGE=str(Path(__file__).with_name('fixtures').joinpath('page-photo.jpg').resolve()))
                if args.scale is not None:
                    env['SLINT_SCALE_FACTOR'] = str(args.scale)
                for key in ('DPDF_SMOKE_PASSWORD', 'DPDF_SMOKE_CLIPBOARD', 'DPDF_SMOKE_PREFERENCES', 'DPDF_SMOKE_ASSEMBLY_SOURCE'):
                    env.pop(key, None)

                def run(mode):
                    env['DPDF_SMOKE_REVIEW'] = mode
                    result = subprocess.run([binary, '--smoke-ui', str(source)], env=env,
                                            capture_output=True, text=True, timeout=60)
                    if result.returncode:
                        raise RuntimeError(result.stdout + result.stderr)
                    return result.stdout

                if case_name.endswith('restart'):
                    report = run('write-notes' if case_name == 'notes-restart' else 'write')
                    assert 'mixed checkpoint durable' in report
                    assert 'remark Undo/Redo restored remarks and revisions' in report
                    checkpoints = list((case / 'data' / 'recovery').glob('*.dpdf'))
                    assert len(checkpoints) == 1
                    source.write_bytes(sample.make_pdf(3))
                if case_name=='edit':
                    real_qpdf=env['DPDF_QPDF_BINARY']
                    env['DPDF_QPDF_BINARY']=str(case / 'missing-qpdf-helper')
                    assert 'unavailable helper and retry preserve reading' in run('unavailable')
                    env['DPDF_QPDF_BINARY']=real_qpdf
                    print('PASS: missing review helper exposes retry without disabling the reader', flush=True)
                before = hashlib.sha256(source.read_bytes()).digest()
                report = run('edit' if case_name == 'edit' else 'read-notes' if case_name == 'notes-restart' else 'read')
                assert 'clean copy, failed retry, older-export guard, mixed save/reopen' in report
                if case_name == 'edit':
                    assert 'remark Undo/Redo restored remarks and revisions' in report
                    assert 'revision selector state follows current filter and preserves hidden remarks and marks' in report
                subprocess.run([qpdf, '--check', str(output)], capture_output=True, check=True)
                result = subprocess.run([binary, '--review', 'inspect', str(output)], env=env,
                                        capture_output=True, text=True, check=True)
                review = json.loads(result.stdout)
                assert len(review['revisions']) == 3 and len(review['remarks']) == 3
                assert 'after older export' in review['remarks'][0]['text']
                result = subprocess.run([qpdf, '--json', '--json-key=pages', str(output)], capture_output=True, check=True)
                pages = json.loads(result.stdout)['pages']
                assert len(pages) == 50
                assert bool(pages[0]['images']) == (case_name != 'notes-restart')
                assert hashlib.sha256(source.read_bytes()).digest() == before
                assert not list((case / 'data' / 'recovery').glob('*.dpdf'))
                print(f'PASS: graphical revisions, independent marks, clean/current copies, stale saves and recovery / {case_name}', flush=True)


if __name__ == '__main__':
    main()
