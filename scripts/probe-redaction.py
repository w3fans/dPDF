#!/usr/bin/env python3
"""Investigate an external MuPDF engine with generated adversarial PDF fixtures.

This is a capability probe, not dPDF secure export or engine qualification.
Exit 3 means the probe completed and found a concrete qualification blocker.
MuPDF is supplied separately; this script never installs or bundles an engine.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zlib


def load_sample():
    spec = importlib.util.spec_from_file_location('sample', Path(__file__).with_name('make-sample.py'))
    sample = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(sample)
    return sample


def stream(content, extra=b''):
    return b'<< /Length '+str(len(content)).encode()+b' '+extra+b'>>\nstream\n'+content+b'\nendstream'


def objects(content, resources=b'', extra=()):
    return [b'<< /Type /Catalog /Pages 2 0 R >>',
            b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
            b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 300] /Resources << /Font << /F1 4 0 R >> '+resources+b' >> /Contents 5 0 R >>',
            b'<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>', stream(content), *extra]


SCRIPT = '''import * as mupdf from "mupdf"
var doc = mupdf.Document.openDocument(scriptArgs[0]);
var page = doc.loadPage(0);
var annotation = page.createAnnotation("Redact");
annotation.setRect(JSON.parse(scriptArgs[2]));
page.applyRedactions(false, 2, Number(scriptArgs[3]), 0);
doc.save(scriptArgs[1], "garbage=deduplicate,compress,encrypt=none");
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mutool', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=Path('target/release/dpdf'))
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    mutool, qpdf, binary = map(lambda p: str(p.resolve()), [args.mutool, args.qpdf, args.binary])
    env = dict(os.environ, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()))
    sample = load_sample()
    report = {'scope':'generated_MuPDF_capability_probe_only', 'dPDF_secure_export_enabled':False,
              'qualification':'incomplete', 'cases':[], 'blockers':[],
              'unverified':['crop/rotation/UserUnit redaction coordinates', 'clipping/transparency/soft masks',
                            'widgets/annotations/review isolation', 'tag trees/alternate extraction payloads',
                            'shared images and arbitrary nested resources', 'final baked-block assembly/editor interoperability',
                            'native Windows execution', 'engine distribution/license choice']}
    with tempfile.TemporaryDirectory(prefix='dpdf-redaction-probe-') as directory:
        folder = Path(directory)
        script = folder / 'probe.js'
        script.write_text(SCRIPT, encoding='utf-8')

        def run(command):
            result = subprocess.run(command, env=env, capture_output=True, timeout=30)
            if result.returncode:
                raise RuntimeError(result.stderr.decode('utf-8', errors='replace'))
            return result.stdout

        report['engine'] = run([mutool, '-v']).decode(errors='replace').strip()
        # Mutool writes its version to stderr on some builds.
        if not report['engine']:
            version = subprocess.run([mutool, '-v'], env=env, capture_output=True, timeout=10)
            report['engine'] = version.stderr.decode(errors='replace').strip()

        def search(path, text):
            return json.loads(run([binary, '--probe-search', str(path), text]))['matches']

        def raster(path):
            output = folder / 'frame.ppm'
            run([binary, '--render-page', str(path), '1', str(output)])
            _, size, _, pixels = output.read_bytes().split(b'\n', 3)
            w, h = map(int, size.split())
            return w, h, pixels

        def outside_changes(before, after, rect):
            width, height, old = before
            new_width, new_height, new = after
            if (width,height) != (new_width,new_height):
                raise AssertionError('Redaction changed page dimensions')
            # Ignore only a two-raster-pixel border for sub-pixel edge rounding.
            x0,y0,x1,y1 = rect
            x0,x1 = x0/400*width-2,x1/400*width+2
            y0,y1 = y0/300*height-2,y1/300*height+2
            return sum(old[(y*width+x)*3:(y*width+x)*3+3] != new[(y*width+x)*3:(y*width+x)*3+3]
                       for y in range(height) for x in range(width) if not (x0 <= x <= x1 and y0 <= y <= y1))

        image = bytes([30,100,210,200,20,40])*200
        fixtures = {
            'crossing-text': (objects(b'BT /F1 10 Tf 50 150 Td (OUTSIDE BEFORE SECRET AFTER OUTSIDE) Tj ET'), [139,140,176,160]),
            'crossing-vector': (objects(b'1 0 0 RG 2 w 20 150 m 380 150 l S'), [150,130,250,170]),
            'shared-form': (objects(b'q 1 0 0 1 100 200 cm /Fm Do Q q 1 0 0 1 100 100 cm /Fm Do Q',
                                   b'/XObject << /Fm 6 0 R >>', [stream(b'BT /F1 10 Tf 0 0 Td (SECRET) Tj ET',
                                   b'/Type /XObject /Subtype /Form /BBox [0 -20 100 20] /Resources << /Font << /F1 4 0 R >> >> ')]), [95,85,145,110]),
            'actualtext': (objects(b'/Span << /ActualText (SECRET ACTUALTEXT) >> BDC BT /F1 10 Tf 100 150 Td (SECRET ACTUALTEXT) Tj ET EMC'), [95,135,210,165]),
            'covered-image-pixels': (objects(b'q 300 0 0 200 50 50 cm /Im Do Q', b'/XObject << /Im 6 0 R >>',
                                      [stream(zlib.compress(image), b'/Type /XObject /Subtype /Image /Width 20 /Height 20 /BitsPerComponent 8 /ColorSpace /DeviceRGB /Filter /FlateDecode ')]), [150,100,250,200]),
        }
        for name, (data, rect) in fixtures.items():
            source = folder / (name+'.pdf')
            source.write_bytes(sample.write_pdf(data))
            digest = hashlib.sha256(source.read_bytes()).digest()
            before = raster(source)
            for mode in [1,2]:
                output = folder / f'{name}-{mode}.pdf'
                run([mutool, 'run', str(script), str(source), str(output), json.dumps(rect), str(mode)])
                run([qpdf, '--check', str(output)])
                decoded = run([qpdf, '--qdf', '--object-streams=disable', '--stream-data=uncompress', str(output), '-'])
                case = {'name':name, 'line_art_method':mode,
                        'outside_pixels_changed':outside_changes(before,raster(output),rect)}
                if name == 'crossing-text':
                    case.update(secret_matches=search(output,'SECRET'), outside_matches=search(output,'OUTSIDE'))
                    assert case['secret_matches'] == 0 and case['outside_matches'] == 2, case
                elif name == 'crossing-vector':
                    case['original_live_path_remains'] = b'380 150 l' in decoded
                    if case['original_live_path_remains'] or case['outside_pixels_changed']:
                        report['blockers'].append({'case':name,'line_art_method':mode,
                            'reason':'Live crossing path remains inside region' if case['original_live_path_remains'] else 'Whole-path removal changes visible content outside region'})
                elif name == 'shared-form':
                    case['remaining_secret_instances'] = search(output,'SECRET')
                    assert case['remaining_secret_instances'] == 1 and case['outside_pixels_changed'] == 0, case
                elif name == 'actualtext':
                    case['visible_secret_matches'] = search(output,'SECRET')
                    case['actualtext_payload_remains'] = b'/ActualText' in decoded and b'SECRET ACTUALTEXT' in decoded
                    assert case['visible_secret_matches'] == 0, case
                    if case['actualtext_payload_remains']:
                        report['blockers'].append({'case':name,'line_art_method':mode,
                            'reason':'Original ActualText payload remains reachable after visible glyphs are erased'})
                elif name == 'covered-image-pixels':
                    case['original_image_bytes_remain'] = image in decoded
                    assert not case['original_image_bytes_remain'], case
                    if case['outside_pixels_changed']:
                        report['blockers'].append({'case':name,'line_art_method':mode,
                            'reason':'Blanking intersecting coarse image pixels changes visible pixels outside the region'})
                assert hashlib.sha256(source.read_bytes()).digest() == digest
                report['cases'].append(case)
    report['qualification'] = 'blocked' if report['blockers'] else 'incomplete'
    encoded = json.dumps(report, indent=2)+'\n'
    if args.output:
        args.output.write_text(encoded, encoding='utf-8')
    print(encoded, end='')
    return 3 if report['blockers'] else 0


if __name__ == '__main__':
    raise SystemExit(main())
