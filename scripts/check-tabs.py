#!/usr/bin/env python3
"""Exercise overflowing document tabs in an isolated real window."""
import argparse
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
    parser.add_argument('--xvfb', default='Xvfb')
    parser.add_argument('--scale', type=float, choices=[1, 1.5, 2], default=1)
    parser.add_argument('--snapshot', type=Path, help='Capture the overflowing fixture tab strip')
    args = parser.parse_args()
    sample = module('sample_tabs', 'make-sample.py')
    private = module('display_tabs', 'private-display.py')
    with tempfile.TemporaryDirectory(prefix='dpdf-tabs-acceptance-') as directory:
        folder = Path(directory)
        fixture = folder / 'Original — tabs.pdf'
        fixture.write_bytes(sample.make_pdf(2))
        original = fixture.read_bytes()
        with private.display(folder, args.xvfb) as env:
            env.update(DPDF_SMOKE_TABS='1', SLINT_SCALE_FACTOR=str(args.scale),
                       DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()))
            if args.snapshot:
                env['DPDF_SMOKE_TAB_SNAPSHOT'] = str(args.snapshot.resolve())
            result = subprocess.run([str(args.binary.resolve()), '--smoke-ui', str(fixture)],
                                    env=env, capture_output=True, text=True, encoding='utf-8',
                                    errors='replace', timeout=60)
            print(result.stdout, end='')
            if result.returncode:
                print(result.stderr, end='')
            result.check_returncode()
            for milestone in (
                'opening twelve PDFs reveals the active tab and complete close control',
                'dispatched Ctrl+Tab and Ctrl+Shift+Tab wrap and reveal their selected document',
                'switching from page and zoom text cancels drafts and restores reader focus',
                'resizing, active/background closure and empty-to-reopen preserve tab visibility and identity',
            ):
                if milestone not in result.stdout:
                    raise RuntimeError('Document tabs diagnostic did not complete: ' + milestone)
            if fixture.read_bytes() != original:
                raise RuntimeError('Document tabs diagnostic changed its source PDF')
        context = 'explicit Windows fixture desktop' if os.name == 'nt' else 'private authenticated X11 display'
        print(f'PASS: overflowing document tabs, scale={args.scale}, {context}')


if __name__ == '__main__':
    main()
