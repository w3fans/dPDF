#!/usr/bin/env python3
"""Generated physical-unit checks for safe mark export and scan-page replacement."""
import argparse
import hashlib
import importlib.util
import json
import math
import re
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location('unit_fixtures', Path(__file__).with_name('check-pages.py'))
    fixtures = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(fixtures)
    env = dict(os.environ, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()),
               DPDF_QPDF_BINARY=str(args.qpdf.resolve()))
    binary, qpdf = str(args.binary.resolve()), str(args.qpdf.resolve())

    def run(command):
        return subprocess.run(list(map(str, command)), env=env, capture_output=True, timeout=45)

    def checked(command):
        result = run(command)
        if result.returncode:
            raise RuntimeError(result.stderr.decode(errors='replace'))
        return result.stdout

    def metadata(path):
        return json.loads(checked([qpdf, '--json', '--json-stream-data=none', path]))

    def page_streams(path, data, index):
        return [checked([qpdf, '--show-object='+','.join(ref.split()[:2]), '--raw-stream-data', path])
                for ref in data['pages'][index]['contents']]

    def source(unit):
        page = b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 300] /Resources << /Font << /F1 7 0 R >> >> '
        first = b'BT /F1 14 Tf 30 250 Td (FIRST PAGE) Tj ET\n'
        second = b'BT /F1 14 Tf 30 250 Td (UNTOUCHED PAGE) Tj ET\n'
        def stream(data):
            return f'<< /Length {len(data)} >>\nstream\n'.encode()+data+b'endstream'
        return fixtures.sample.write_pdf([
            b'<< /Type /Catalog /Pages 2 0 R >>',
            b'<< /Type /Pages /Count 2 /Kids [3 0 R 5 0 R] >>',
            page+(b'' if unit is None else b'/UserUnit '+unit+b' ')+b'/Contents 4 0 R >>',
            stream(first), page+b'/UserUnit 2 /Contents 6 0 R >>', stream(second),
            b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>', b'1', b'2',
        ])

    with tempfile.TemporaryDirectory(prefix='dpdf-page-units-') as directory:
        root = Path(directory)
        original, image, recipe, output = [root/name for name in ('source.pdf', 'mark.png', 'recipe.json', 'copy.pdf')]
        image.write_bytes(fixtures.png(32, 16))
        recipe.write_text(json.dumps([dict(page=1, image=image.name,
            geometry=dict(x_mm=10, y_mm=10, width_mm=40, height_mm=20))]))
        previous = fixtures.sample.make_pdf(1)
        for name, value, allowed in [('default', None, True), ('one', b'1', True),
                ('null', b'null', True), ('indirect-one', b'8 0 R', True),
                ('double', b'2', True), ('half', b'0.5', True), ('indirect-double', b'9 0 R', True),
                ('zero', b'0', False), ('negative', b'-2', False), ('boolean', b'true', False),
                ('precision', b'75000', False)]:
            original.write_bytes(source(value))
            before = {path: hashlib.sha256(path.read_bytes()).digest() for path in (original, image, recipe)}
            output.write_bytes(previous)
            result = run([binary, '--overlay', original, output, recipe])
            if allowed:
                if result.returncode:
                    raise RuntimeError(name+': '+result.stderr.decode(errors='replace'))
                checked([qpdf, '--check', output])
                old, new = metadata(original), metadata(output)
                if page_streams(original, old, 1) != page_streams(output, new, 1):
                    raise RuntimeError('Untouched custom-unit page changed')
                value = old['qpdf'][1]['obj:'+old['pages'][0]['object']]['value'].get('/UserUnit', 1)
                while isinstance(value, str):
                    value = old['qpdf'][1]['obj:'+value]['value']
                unit = 1 if value is None else value
                content = b' '.join(checked([qpdf, '--show-object='+','.join(ref.split()[:2]), '--filtered-stream-data', output]) for ref in new['pages'][0]['contents'])
                matrices = re.findall(rb'([-+\d.eE]+)\s+([-+\d.eE]+)\s+([-+\d.eE]+)\s+([-+\d.eE]+)\s+([-+\d.eE]+)\s+([-+\d.eE]+)\s+cm', content)
                a,b,c,d,x,y = map(float, matrices[-1])
                if abs(math.hypot(a,b)*unit*25.4/72-40)>.02 or abs(math.hypot(c,d)*unit*25.4/72-20)>.02:
                    raise RuntimeError('Exported physical mark size differs from requested mm')
                if abs(x*unit*25.4/72-10)>.02 or abs((300-y-d)*unit*25.4/72-10)>.02:
                    raise RuntimeError('Exported physical mark position differs from requested mm')
                page = new['qpdf'][1]['obj:'+new['pages'][1]['object']]['value']
                if page.get('/UserUnit') != 2 or len(new['pages'][0]['images']) != 1:
                    raise RuntimeError('Untouched scaling or placed image changed')
            else:
                if result.returncode == 0 or (b'UserUnit' not in result.stderr and b'coordinate precision' not in result.stderr):
                    raise RuntimeError(name+': expected an invalid/unsupported precision refusal before export')
                if output.read_bytes() != previous:
                    raise RuntimeError('Refused custom-unit export changed the previous copy')
                output.unlink()
                refused = run([binary, '--overlay', original, output, recipe])
                if refused.returncode == 0 or output.exists():
                    raise RuntimeError('Refused custom-unit export created a new copy')
            if any(hashlib.sha256(path.read_bytes()).digest() != digest for path, digest in before.items()):
                raise RuntimeError('Physical-unit preflight changed an input')
            print('PASS: mark units '+name+', '+('physical-size export and untouched scaling preserved' if allowed else 'invalid units or precision refused; prior/new outputs preserved'), flush=True)
        image.write_bytes(fixtures.png(80, 40))
        for unit in (2, 0.5):
            original.write_bytes(fixtures.sample.make_physical_unit_pdf(unit))
            digest = hashlib.sha256(original.read_bytes()).digest()
            old = metadata(original)
            checked([binary, '--assemble', 'replace', original, output, '3', '1', image])
            checked([qpdf, '--check', output])
            new = metadata(output)
            for index in range(3):
                page = new['qpdf'][1]['obj:'+new['pages'][index]['object']]['value']
                box = page['/MediaBox']
                if page.get('/UserUnit', 1) != unit:
                    raise RuntimeError('Scan replacement lost target/untouched page units')
                if abs((box[2]-box[0])*unit-612) > .01 or abs((box[3]-box[1])*unit-792) > .01:
                    raise RuntimeError('Scan replacement changed physical page dimensions')
                if index < 2 and page_streams(original, old, index) != page_streams(output, new, index):
                    raise RuntimeError('Scan replacement changed an untouched page stream')
            if hashlib.sha256(original.read_bytes()).digest() != digest:
                raise RuntimeError('Scan replacement changed its input')
            print(f'PASS: scan replacement retains UserUnit {unit}, physical letter dimensions and untouched pages', flush=True)

        original.write_bytes(fixtures.sample.write_pdf([
            b'<< /Type /Catalog /Pages 2 0 R >>',
            b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
            b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 12 12] /UserUnit 2 /Resources << >> /Contents 4 0 R >>',
            b'<< /Length 0 >>\nstream\nendstream',
        ]))
        checked([binary, '--assemble', 'replace', original, output, '1', '1', image])
        data = metadata(output)
        dimensions = [(item['width'], item['height']) for item in data['pages'][0]['images']]
        if (80, 40) not in dimensions:
            raise RuntimeError('Image sampling used raw coordinates instead of physical print dimensions: '+str(dimensions))
        print('PASS: physical print-resolution budget retains an 80x40 image below its 100x50 limit', flush=True)
    print('PASS: physical mark geometry, invalid/precision refusals, scan page size, untouched pages and image sampling')


if __name__ == '__main__':
    main()
