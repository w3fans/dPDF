#!/usr/bin/env python3
"""Independent Poppler checks for replacement, visual marks and review exports."""
import argparse
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/dpdf'))
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    parser.add_argument('--pdftoppm', default='pdftoppm')
    parser.add_argument('--pdftotext', default='pdftotext')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location('fixture', Path(__file__).with_name('check-pages.py'))
    fixture = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(fixture)
    binary = str(args.binary.resolve())
    env = dict(os.environ, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()),
               DPDF_QPDF_BINARY=str(args.qpdf.resolve()))

    def run(command):
        result = subprocess.run(list(map(str, command)), env=env, capture_output=True, timeout=90)
        if result.returncode:
            raise RuntimeError(result.stderr.decode('utf-8', errors='replace'))
        return result.stdout

    def text(path):
        return run([args.pdftotext, '-layout', '-enc', 'UTF-8', path, '-'])

    def raster(path, page=1, hide=False):
        options = ['-hide-annotations'] if hide else []
        data = run([args.pdftoppm, '-cropbox', '-singlefile', '-r', '96',
                    '-f', page, '-l', page, *options, path])
        magic, dimensions, maximum, pixels = data.split(b'\n', 3)
        width, height = map(int, dimensions.split())
        assert magic == b'P6' and maximum == b'255' and len(pixels) == width * height * 3
        return width, height, pixels

    version = subprocess.run([args.pdftoppm, '-v'], capture_output=True, text=True, check=True)
    report = dict(scope='generated_Poppler_export_interoperability_only',
                  binary_version=run([binary, '--version']).decode().strip(),
                  binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                  poppler=(version.stdout + version.stderr).splitlines()[0], cases=[],
                  unqualified=['interactive external-viewer editing', 'physical printer output',
                               'Windows Poppler execution', 'genuine secure region baking'])
    with tempfile.TemporaryDirectory(prefix='dpdf-poppler-') as directory:
        root = Path(directory)
        image = root / 'synthetic-mark.png'
        image.write_bytes(fixture.png(120, 80))
        original_image = hashlib.sha256(image.read_bytes()).digest()
        for cropped in (False, True):
            for rotation in (0, 90, 180, 270):
                case = root / f'crop-{cropped}-rotation-{rotation}'
                case.mkdir()
                original = case / 'original.pdf'
                original.write_bytes(fixture.sample.make_pdf(1, rotation=rotation, cropped=cropped))
                original_hash = hashlib.sha256(original.read_bytes()).digest()
                marked, placements = case / 'marked.pdf', case / 'placements.json'
                placements.write_text(json.dumps([dict(page=1, image=str(image), opacity=.7,
                    geometry=dict(x_mm=25, y_mm=40, width_mm=40, height_mm=20))]))
                run([binary, '--overlay', original, marked, placements])
                assert text(original) == text(marked), (cropped, rotation, 'text changed')
                before, after = raster(original), raster(marked)
                assert before[:2] == after[:2] and before[2] != after[2]
                width, height = before[:2]
                x0, x1 = 25 / 25.4 * 96 - 2, 65 / 25.4 * 96 + 2
                y0, y1 = 40 / 25.4 * 96 - 2, 60 / 25.4 * 96 + 2
                outside = sum(before[2][(y*width+x)*3:(y*width+x)*3+3] !=
                              after[2][(y*width+x)*3:(y*width+x)*3+3]
                              for y in range(height) for x in range(width)
                              if not (x0 <= x <= x1 and y0 <= y <= y1))
                assert outside == 0, (cropped, rotation, outside)
                model = dict(schema_version=2, workspace='aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee',
                    revisions=[dict(id=1, label='Revision 1'), dict(id=2, label='Revision 2')],
                    current=2, visible=True, filter='all', remarks=[
                        dict(id=1, page=0, revision=1, kind='note', text='Generated review note',
                             bounds=dict(x=.72,y=.7,width=.04,height=.04)),
                        dict(id=2, page=0, revision=2, kind='highlight', text='Generated highlight',
                             bounds=dict(x=.1,y=.4,width=.4,height=.05)),
                        dict(id=3, page=0, revision=2, kind='underline', text='Generated underline',
                             bounds=dict(x=.1,y=.5,width=.4,height=.05))])
                recipe = case / 'review.json'
                recipe.write_text(json.dumps(model))
                visible = case / 'visible.pdf'
                run([binary, '--review', 'export', marked, visible, recipe, 'visible'])
                assert raster(visible)[2] != after[2]
                assert raster(visible, hide=True) == after
                hidden_model = copy.deepcopy(model)
                hidden_model['visible'] = False
                recipe.write_text(json.dumps(hidden_model))
                hidden = case / 'hidden.pdf'
                run([binary, '--review', 'export', visible, hidden, recipe, 'current'])
                clean = case / 'clean.pdf'
                run([binary, '--review', 'export', visible, clean, recipe, 'clean'])
                assert raster(hidden) == after and raster(clean) == after
                assert text(hidden) == text(marked) == text(clean)
                assert hashlib.sha256(original.read_bytes()).digest() == original_hash
                report['cases'].append(dict(cropped=cropped, rotation=rotation, status='PASS',
                                            outside_changed_pixels=outside,
                                            checks=['overlay physical placement/alpha/text', 'visible native remarks',
                                                    'hide annotations', 'hidden remarks', 'clean copy retains marks']))
                print(f'PASS: Poppler crop={cropped}, rotation={rotation}: mark geometry/text, visible/hidden/clean remarks', flush=True)
        original = root / 'contract-10-pages.pdf'
        original.write_bytes(fixture.sample.make_pdf(10))
        replacement = root / 'signed-page-copy.pdf'
        run([binary, '--assemble', 'replace', original, replacement, '10', '1', image])
        for page in (1, 9):
            assert raster(original, page) == raster(replacement, page)
        assert raster(original, 10) != raster(replacement, 10)
        assert text(original).count(b'payment due') == 10
        assert text(replacement).count(b'payment due') == 9
        assert hashlib.sha256(image.read_bytes()).digest() == original_image
        report['cases'].append(dict(name='contract page-10 replacement', status='PASS',
                                    checks=['pages 1/9 identical', 'page 10 replaced', 'outside text count preserved']))
        print('PASS: Poppler signed-page replacement preserves untouched pages and outside text', flush=True)
    report['status'] = 'PASS'
    if args.output:
        args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
