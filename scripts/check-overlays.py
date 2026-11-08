#!/usr/bin/env python3
"""Check ordinary mark overlays, physical geometry, alpha and page preservation."""
import argparse
import copy
import hashlib
import importlib.util
import json
import math
import re
import os
from pathlib import Path
import struct
import subprocess
import tempfile

spec = importlib.util.spec_from_file_location('page_checks', Path(__file__).with_name('check-pages.py'))
fixtures = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixtures)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/dpdf'))
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    args = parser.parse_args()
    binary, qpdf = str(args.binary.resolve()), str(args.qpdf.resolve())
    env = dict(os.environ, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()), DPDF_QPDF_BINARY=qpdf)

    def run(command, valid=True):
        result = subprocess.run(command, env=env, capture_output=True, timeout=60)
        if valid and result.returncode:
            raise AssertionError(result.stderr.decode(errors='replace'))
        return result

    def pages(path):
        return json.loads(run([qpdf, '--json', '--json-key=pages', str(path)]).stdout)['pages']

    def streams(path):
        return [[run([qpdf, '--show-object=' + ','.join(ref.split()[:2]), '--raw-stream-data', str(path)]).stdout
                 for ref in page['contents']] for page in pages(path)]

    with tempfile.TemporaryDirectory(prefix='dpdf-overlay-') as directory:
        folder = Path(directory)
        original, output, recipe, image = [folder / name for name in ('original.pdf', 'marked.pdf', 'placements.json', 'mark.png')]
        original.write_bytes(fixtures.sample.make_pdf(10))
        image.write_bytes(fixtures.png(120, 80))
        expected_inputs = {path: hashlib.sha256(path.read_bytes()).digest() for path in (original, image)}
        original_streams = streams(original)
        mark = dict(page=10, image=image.name, geometry=dict(x_mm=25, y_mm=40, width_mm=40, height_mm=20), opacity=1.0)

        def export(marks, source=original, destination=output, valid=True):
            recipe.write_text(json.dumps(marks))
            result = run([binary, '--overlay', str(source), str(destination), str(recipe)], valid)
            if valid:
                run([qpdf, '--check', str(destination)])
            return result

        def raster(source, page):
            ppm = folder / 'frame.ppm'
            run([binary, '--render-page', str(source), str(page), str(ppm)])
            magic, dimensions, maximum, data = ppm.read_bytes().split(b'\n', 3)
            width, height = map(int, dimensions.split())
            assert magic == b'P6' and maximum == b'255' and len(data) == width * height * 3
            return width, height, data

        export([mark])
        assert streams(output)[:9] == original_streams[:9]
        assert len(pages(output)[9]['images']) == 1
        for page in range(1, 10):
            assert raster(original, page) == raster(output, page), page
        assert json.loads(run([binary, '--probe-search', str(original), 'payment']).stdout)['matches'] == json.loads(run([binary, '--probe-search', str(output), 'payment']).stdout)['matches']
        print('PASS: marked page 10 retains text; pages 1-9 keep exact raw streams and pixels', flush=True)

        # Intrinsic page rotation/crop and independent mark rotation; check color
        # quadrants, alpha blending, true physical size and unchanged outside pixels.
        colors = ((190, 40, 30, 255), (30, 60, 180, 255), (190, 40, 30, 128), (30, 60, 180, 128))
        for unit in (1, 2, .5):
            for cropped in (False, True):
                for rotation in (0, 90, 180, 270):
                    source = folder / 'rotated.pdf'
                    source.write_bytes(fixtures.sample.make_pdf(1, rotation=rotation, cropped=cropped, user_unit=unit))
                    before = raster(source, 1)
                    width_pt, height_pt = (532, 692) if cropped else (612, 792)
                    if rotation in (90, 270):
                        width_pt, height_pt = height_pt, width_pt
                    width_mm, height_mm = width_pt * unit * 25.4 / 72, height_pt * unit * 25.4 / 72
                    for turn in range(4):
                        for opacity in (1.0, 0.5):
                            placed = copy.deepcopy(mark)
                            placed['page'] = 1
                            placed['geometry']['quarter_turns'] = turn
                            placed['opacity'] = opacity
                            export([placed], source)
                            details = json.loads(run([qpdf, '--json', '--json-stream-data=none', str(output)]).stdout)
                            dictionary = details['qpdf'][1]['obj:'+details['pages'][0]['object']]['value']
                            assert dictionary.get('/UserUnit', 1) == unit
                            content = b' '.join(run([qpdf, '--show-object='+','.join(ref.split()[:2]), '--filtered-stream-data', str(output)]).stdout for ref in details['pages'][0]['contents'])
                            matrices = re.findall(rb'([-+\d.eE]+)\s+([-+\d.eE]+)\s+([-+\d.eE]+)\s+([-+\d.eE]+)\s+([-+\d.eE]+)\s+([-+\d.eE]+)\s+cm', content)
                            a, b, c, d, _, _ = map(float, matrices[-1])
                            assert abs(math.hypot(a,b)*unit*25.4/72-40) < .02
                            assert abs(math.hypot(c,d)*unit*25.4/72-20) < .02
                            w, h, pixels = raster(output, 1)
                            assert (w, h) == before[:2]
                            gw, gh = (40, 20) if turn % 2 == 0 else (20, 40)
                            left, top = 25 / width_mm * w, 40 / height_mm * h
                            bw, bh = gw / width_mm * w, gh / height_mm * h
                            for index, color in enumerate(colors):
                                u, v = (0.25 if index % 2 == 0 else 0.75), (0.25 if index < 2 else 0.75)
                                for _ in range(turn):
                                    u, v = 1-v, u
                                x, y = int(left + u * bw), int(top + v * bh)
                                offset = (y * w + x) * 3
                                alpha = round(color[3] * opacity) / 255
                                expected = [round(c * alpha + b * (1-alpha)) for c, b in zip(color[:3], before[2][offset:offset+3])]
                                actual = list(pixels[offset:offset+3])
                                assert all(abs(a-b) <= 3 for a, b in zip(actual, expected)), (cropped, rotation, turn, opacity, x, y, actual, expected)
                            for y in range(h):
                                spans = [(0, w)] if not top-2 <= y <= top+bh+2 else [(0, max(0, int(left)-2)), (min(w, int(left+bw)+3), w)]
                                for start, end in spans:
                                    lo, hi = (y*w+start)*3, (y*w+end)*3
                                    assert pixels[lo:hi] == before[2][lo:hi], (cropped, rotation, turn, y, start, end)
        print('PASS: 192 unit/crop/rotation/turn/alpha cases; physical PDF matrix size and unchanged outside pixels', flush=True)

        another = copy.deepcopy(mark)
        another['page'] = 1
        third = copy.deepcopy(mark)
        third['geometry']['x_mm'] = 80
        export([mark, another, third])
        assert len(pages(output)[0]['images']) == 1 and len(pages(output)[9]['images']) == 2
        assert streams(output)[1:9] == original_streams[1:9]
        print('PASS: multiple marks/pages and unchanged intermediate page streams')

        # Exercise bounded resampling and JPEG phone orientation through this API.
        large = folder / 'large.png'
        large.write_bytes(fixtures.png(1400, 1000))
        small = copy.deepcopy(mark)
        small['image'] = large.name
        small['geometry']['width_mm'], small['geometry']['height_mm'] = 10, 8
        export([small])
        embedded = pages(output)[9]['images'][0]
        assert embedded['width'] <= 158 and embedded['height'] <= 126, embedded
        jpeg = Path(__file__).with_name('fixtures').joinpath('page-photo.jpg').read_bytes()
        exif = b'Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0'
        photo = folder / 'phone.jpg'
        photo.write_bytes(jpeg[:2] + b'\xff\xe1' + struct.pack('>H', len(exif)+2) + exif + jpeg[2:])
        oriented = copy.deepcopy(mark)
        oriented['image'] = photo.name
        oriented['geometry']['height_mm'] = 40
        export([oriented])
        embedded = pages(output)[9]['images'][0]
        assert (embedded['width'], embedded['height']) == (200, 400), embedded
        print('PASS: print-resolution downsampling and JPEG phone orientation')

        sentinel = folder / 'previous.pdf'
        sentinel.write_bytes(b'previous output')
        invalid = [[], [dict(mark, page=0)], [dict(mark, page=11)], [dict(mark, opacity=1.1)], [dict(mark, opacity=float('nan'))]]
        for field, value in (('x_mm', -1), ('width_mm', 0), ('quarter_turns', 4), ('y_mm', 900)):
            bad = copy.deepcopy(mark)
            bad['geometry'][field] = value
            invalid.append([bad])
        for marks in invalid:
            assert export(marks, destination=sentinel, valid=False).returncode != 0
            assert sentinel.read_bytes() == b'previous output'
        corrupt = folder / 'corrupt.png'
        corrupt.write_bytes(b'not an image')
        assert export([dict(mark, image=corrupt.name)], destination=sentinel, valid=False).returncode != 0
        assert sentinel.read_bytes() == b'previous output'
        for destination in (original, image, recipe):
            assert export([mark], destination=destination, valid=False).returncode != 0
        for path, digest in expected_inputs.items():
            assert hashlib.sha256(path.read_bytes()).digest() == digest
        print('PASS: invalid placements retain previous output; PDF/image/recipe overwrite refused')


if __name__ == '__main__':
    main()
