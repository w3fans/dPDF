#!/usr/bin/env python3
"""Verify physical page units through the real reader metadata and window path."""
import argparse
import importlib.util
import os
import math
import re
import json
from pathlib import Path
import subprocess
import tempfile


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def fixture(sample, mode):
    units = (2, .5, -1 if mode == 'invalid-unit' else 1)
    catalog = b'<< /Type /Catalog /Pages 2 0 R '
    if mode == 'bad-review':
        catalog += b'/dPDFReview (not-json) '
    objects = [catalog+b'>>', b'<< /Type /Pages /Count 3 /Kids [3 0 R 5 0 R 7 0 R] >>']
    content = b'20 20 40 20 re 0.5 g f\n'
    for index, unit in enumerate(units):
        width, height = 612/abs(unit), 792/abs(unit)
        if mode == 'rotated-crop':
            boxes = f'/MediaBox [-30 -20 {width+30} {height+20}] /CropBox [0 0 {width} {height}] /Rotate 90 '
        else:
            boxes = f'/MediaBox [0 0 {width} {height}] '
        objects += [(f'<< /Type /Page /Parent 2 0 R {boxes}/UserUnit {unit} /Resources << >> /Contents {4+2*index} 0 R >>').encode(),
                    f'<< /Length {len(content)} >>\nstream\n'.encode()+content+b'endstream']
    return sample.write_pdf(objects)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    parser.add_argument('--xvfb', default='Xvfb')
    parser.add_argument('--scale', type=float, choices=[1, 1.5, 2], default=1)
    args = parser.parse_args()
    sample = module('physical_sample', 'make-sample.py')
    private = module('physical_display', 'private-display.py')
    with tempfile.TemporaryDirectory(prefix='dpdf-physical-reader-') as directory:
        root = Path(directory)
        for mode in ['normal', 'rotated-crop', 'bad-review', 'missing-helper', 'invalid-unit', 'recovery-write', 'recovery-read']:
            folder = root/('recovery' if mode.startswith('recovery-') else mode)
            folder.mkdir(exist_ok=True)
            path = folder/'Mixed physical pages.pdf'
            original = fixture(sample, mode)
            path.write_bytes(original)
            with private.display(folder, args.xvfb) as env:
                env.update(DPDF_SMOKE_PHYSICAL=mode, SLINT_SCALE_FACTOR=str(args.scale),
                           DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()),
                           DPDF_QPDF_BINARY=str(args.qpdf.resolve()) if mode != 'missing-helper' else str(folder/'missing-qpdf'),
                           DPDF_DATA_DIR=str(folder/'data'))
                if mode.startswith('recovery-'):
                    env['DPDF_SMOKE_RECOVERY'] = mode.removeprefix('recovery-')
                if mode == 'recovery-read':
                    env['DPDF_SMOKE_PHYSICAL_EXPORT'] = str(folder/'Recovered signed copy.pdf')
                if os.name != 'nt':
                    env['DBUS_SESSION_BUS_ADDRESS'] = 'unix:path='+str(folder/'no-bus')
                result = subprocess.run([str(args.binary.resolve()), '--smoke-ui', str(path)],
                    env=env, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=45)
                print(result.stdout, end='')
                if result.returncode:
                    print(result.stderr, end='')
                result.check_returncode()
                expected = 'unavailable dimensions retain reader frames' if mode in ('missing-helper', 'invalid-unit') else 'background and closed-document facts preserve active tab'
                if mode == 'recovery-write':
                    expected = 'custom-unit mark checkpoint committed'
                elif mode == 'recovery-read':
                    expected = 'custom-unit recovery retains a mark beyond raw coordinate bounds'
                if expected not in result.stdout:
                    raise RuntimeError('Physical reader diagnostic did not finish: '+mode)
            if mode == 'recovery-read':
                output = folder/'Recovered signed copy.pdf'
                subprocess.run([str(args.qpdf.resolve()), '--check', str(output)], capture_output=True, check=True)
                data = json.loads(subprocess.check_output([str(args.qpdf.resolve()), '--json', '--json-stream-data=none', str(output)]))
                page = data['pages'][0]
                unit = data['qpdf'][1]['obj:'+page['object']]['value']['/UserUnit']
                assert unit == 2
                content = b' '.join(subprocess.check_output([str(args.qpdf.resolve()), '--show-object='+','.join(ref.split()[:2]), '--filtered-stream-data', str(output)]) for ref in page['contents'])
                matrix = re.findall(rb'([-+\d.eE]+)\s+([-+\d.eE]+)\s+([-+\d.eE]+)\s+([-+\d.eE]+)\s+([-+\d.eE]+)\s+([-+\d.eE]+)\s+cm', content)[-1]
                a,b,c,d,x,y = map(float, matrix)
                assert abs(math.hypot(a,b)*unit*25.4/72-40)<.02 and abs(math.hypot(c,d)*unit*25.4/72-20)<.02
                assert abs(x*unit*25.4/72-87.95)<.02
                print('PASS: recovered mark PDF retains physical size, position and page units')
            if path.read_bytes() != original:
                raise RuntimeError('Physical reader changed the input: '+mode)
            print(f'PASS: physical reader {mode}, scale={args.scale}', flush=True)


if __name__ == '__main__':
    main()
