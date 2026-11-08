#!/usr/bin/env python3
"""Exercise reading-position preservation in an isolated real window."""
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
    args = parser.parse_args()
    sample = module('sample_viewport', 'make-sample.py')
    private = module('display_viewport', 'private-display.py')
    with tempfile.TemporaryDirectory(prefix='dpdf-viewport-acceptance-') as directory:
        folder = Path(directory)
        fixture = folder / 'Viewport fixture.pdf'
        fixture.write_bytes(sample.make_pdf(50))
        original = fixture.read_bytes()
        with private.display(folder, args.xvfb) as env:
            env.update(DPDF_SMOKE_VIEWPORT='1', SLINT_SCALE_FACTOR=str(args.scale),
                       DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()))
            if os.name != 'nt':
                # This suite needs no portal. Accidental picker activation must
                # never reach the normal desktop's session bus.
                env['DBUS_SESSION_BUS_ADDRESS'] = 'unix:path=' + str(folder / 'no-session-bus')
            try:
                result = subprocess.run([str(args.binary.resolve()), '--smoke-ui', str(fixture)],
                                        env=env, capture_output=True, text=True, encoding='utf-8',
                                        errors='replace', timeout=60)
            except subprocess.TimeoutExpired as error:
                for output in (error.stdout, error.stderr):
                    if output:
                        print(output.decode('utf-8', errors='replace') if isinstance(output, bytes) else output)
                raise
            print(result.stdout, end='')
            if result.returncode:
                print(result.stderr, end='')
            result.check_returncode()
            if (result.stdout.count('viewport: case=') != 12 or
                    result.stdout.count('viewport: zoom case=') != 12 or
                    'numeric zoom entry rejects an invalid value' not in result.stdout or
                    'uncommitted numeric zoom restores on Escape and focus loss' not in result.stdout or
                    'preserve page-relative reading position in all cases' not in result.stdout):
                raise RuntimeError('Viewport diagnostic did not complete all twelve cases')
            if fixture.read_bytes() != original:
                raise RuntimeError('Viewport diagnostic changed its source PDF')
        context = 'explicit Windows fixture desktop' if os.name == 'nt' else 'private authenticated X11 display'
        print(f'PASS: reading-position preservation, scale={args.scale}, {context}')


if __name__ == '__main__':
    main()
