#!/usr/bin/env python3
"""Native clipboard/placement fixtures on private X11 or an explicitly enabled Windows test desktop."""
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
    parser.add_argument('--pdfium', type=Path)
    parser.add_argument('--xvfb', default='Xvfb')
    parser.add_argument('--marks', action='store_true', help='Test image paste into the mark studio')
    parser.add_argument('--placement', action='store_true', help='Also test on-page dragging/resizing and copy export')
    parser.add_argument('--qpdf', type=Path)
    parser.add_argument('--share-snapshot', type=Path, help='Capture the fixture share panel for visual inspection')
    parser.add_argument('--scale', type=float, choices=[1, 1.5, 2], default=1)
    parser.add_argument('--allow-native-clipboard', action='store_true',
                        help='Allow generated clipboard content in a dedicated Windows test desktop')
    args = parser.parse_args()
    if os.name == 'nt' and not args.allow_native_clipboard:
        parser.error('Windows clipboard tests require --allow-native-clipboard in a dedicated test desktop')
    if args.placement and (args.marks or not args.qpdf):
        parser.error('--placement requires --qpdf and the reader diagnostic')
    if not args.marks and not args.pdfium:
        parser.error('--pdfium is required for the PDF reader diagnostic')
    sample = module('sample', 'make-sample.py')
    private = module('display', 'private-display.py')
    with tempfile.TemporaryDirectory(prefix='dpdf-clipboard-acceptance-') as directory:
        folder = Path(directory)
        fixture = folder / 'contract.pdf'
        fixture.write_bytes(sample.make_pdf(50))
        command = '--smoke-ui'
        if args.marks:
            fixture = folder / 'mark-fixture.jpg'
            fixture.write_bytes(Path(__file__).with_name('fixtures').joinpath('page-photo.jpg').read_bytes())
            command = '--smoke-marks'
        with private.display(folder, args.xvfb) as env:
            env.update(DPDF_SMOKE_CLIPBOARD='1', SLINT_SCALE_FACTOR=str(args.scale))
            if args.share_snapshot and not args.marks:
                env['DPDF_SMOKE_SHARE_SNAPSHOT'] = str(args.share_snapshot.resolve())
            if args.pdfium:
                env['DPDF_PDFIUM_LIBRARY'] = str(args.pdfium.resolve())
            if args.placement:
                mark_image = folder / 'placement.jpg'
                mark_image.write_bytes(Path(__file__).with_name('fixtures').joinpath('page-photo.jpg').read_bytes())
                env.update(DPDF_SMOKE_PLACEMENT_IMAGE=str(mark_image), DPDF_QPDF_BINARY=str(args.qpdf.resolve()))
            if args.marks:
                env['DPDF_SMOKE_MARKS_CLIPBOARD'] = '1'
            result = subprocess.run([str(args.binary.resolve()), command, str(fixture)],
                                    env=env, capture_output=True, text=True, encoding='utf-8',
                                    errors='replace', timeout=60)
            print(result.stdout, end='')
            if result.returncode:
                print(result.stderr, end='')
            result.check_returncode()
            expected = ('native clipboard image snapshot survived later clipboard changes' if args.marks
                        else 'independent native clipboard readback matched')
            if not args.marks and ('saved-file sharing copied exact path' not in result.stdout or
                                         'share panel rendered and dispatched Escape closed it' not in result.stdout):
                raise RuntimeError('The executable did not exercise saved-file path sharing')
            if not args.marks:
                for milestone in (
                    'held thumbnail drag scrolled beyond the viewport and dropped in the exact output slot',
                    'upward thumbnail edge scroll stopped at the first page; cancelled drag retained output order',
                ):
                    if milestone not in result.stdout:
                        raise RuntimeError('Thumbnail edge scrolling was not verified: ' + milestone)
            if args.placement:
                for milestone in (
                    'cancelled placement left saved asset usage unchanged',
                    'saved asset usage recorded on placement without changing its default size',
                    'unsaved signature sharing refused without clipboard changes',
                    'exported marked copy is ready for sharing',
                    'dispatched Undo/Redo/Delete restored physical placement and opacity',
                    'numeric text-field Undo retained native editing behavior',
                    'failed save retained drafts and history for retry',
                    'older export retained edits made while saving',
                    'worker exported marked copy and reopened it',
                ):
                    if milestone not in result.stdout:
                        raise RuntimeError('Placement diagnostic did not complete: ' + milestone)
            if expected not in result.stdout:
                raise RuntimeError('The executable did not exercise the native clipboard diagnostic')
            if args.marks and 'arming a saved asset did not record page usage' not in result.stdout:
                raise RuntimeError('Studio diagnostic did not verify unused asset metadata')
            context = 'explicit Windows fixture desktop' if os.name == 'nt' else 'private authenticated X11 display'
            print(f'PASS: native clipboard {"image" if args.marks else "text/placement"}, scale={args.scale}, {context}')


if __name__ == '__main__':
    main()
