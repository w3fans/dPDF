#!/usr/bin/env python3
"""Generated-fixture PDFium object-removal investigation, NOT a secure export tool.

Only whole contained objects are removed. Crossing objects are refused. No signature
is inserted or baked. Exit 3 records an observed qualification blocker.
"""
import argparse
import ctypes as C
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import subprocess
import tempfile


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


class Engine:
    def __init__(self, path):
        self.dll = C.CDLL(str(path))
        p, i, u = C.c_void_p, C.c_int, C.c_ulong
        fp = C.POINTER(C.c_float)
        signatures = {
            'FPDF_InitLibrary': ([], None), 'FPDF_DestroyLibrary': ([], None),
            'FPDF_LoadMemDocument64': ([p, C.c_size_t, C.c_char_p], p),
            'FPDF_LoadPage': ([p, i], p), 'FPDF_ClosePage': ([p], None),
            'FPDF_CloseDocument': ([p], None), 'FPDFPage_CountObjects': ([p], i),
            'FPDFPage_GetObject': ([p, i], p), 'FPDFPageObj_GetType': ([p], i),
            'FPDFPageObj_CountMarks': ([p], i),
            'FPDFPageObj_GetBounds': ([p, fp, fp, fp, fp], i),
            'FPDFPage_RemoveObject': ([p, p], i), 'FPDFPageObj_Destroy': ([p], None),
            'FPDFPage_GenerateContent': ([p], i),
        }
        for name, (args, result) in signatures.items():
            function = getattr(self.dll, name)
            function.argtypes, function.restype = args, result
        self.dll.FPDF_InitLibrary()
        callback = C.CFUNCTYPE(C.c_int, p, p, u)

        class Writer(C.Structure):
            _fields_ = [('version', C.c_int), ('WriteBlock', callback)]

        self.callback, self.Writer = callback, Writer
        self.dll.FPDF_SaveAsCopy.argtypes = [p, C.POINTER(Writer), u]
        self.dll.FPDF_SaveAsCopy.restype = i

    def remove_contained(self, source, output, region):
        data = C.create_string_buffer(source.read_bytes())
        document = self.dll.FPDF_LoadMemDocument64(data, len(data.raw) - 1, None)
        if not document:
            raise RuntimeError('PDFium could not open generated fixture')
        page = None
        try:
            page = self.dll.FPDF_LoadPage(document, 0)
            if not page:
                raise RuntimeError('PDFium could not load generated fixture page')
            objects, selected, crossing = [], [], []
            left, bottom, right, top = region
            for index in range(self.dll.FPDFPage_CountObjects(page)):
                obj = self.dll.FPDFPage_GetObject(page, index)
                values = [C.c_float() for _ in range(4)]
                if not self.dll.FPDFPageObj_GetBounds(obj, *[C.byref(value) for value in values]):
                    raise RuntimeError('Object bounds unavailable')
                x0, y0, x1, y1 = [value.value for value in values]
                if not all(math.isfinite(value) for value in (x0, y0, x1, y1)):
                    raise RuntimeError('Nonfinite object bounds')
                intersects = x1 > left and x0 < right and y1 > bottom and y0 < top
                contained = x0 >= left and x1 <= right and y0 >= bottom and y1 <= top
                info = dict(index=index, type=self.dll.FPDFPageObj_GetType(obj),
                            bounds=[x0, y0, x1, y1], marks=self.dll.FPDFPageObj_CountMarks(obj),
                            intersects=intersects, contained=contained)
                objects.append(info)
                if intersects:
                    (selected if contained else crossing).append(obj)
            result = dict(objects=objects, selected=len(selected), crossing=len(crossing))
            if crossing:
                result['action'] = 'refused_crossing_object'
                return result
            for obj in selected:
                if not self.dll.FPDFPage_RemoveObject(page, obj):
                    raise RuntimeError('Object removal failed')
                self.dll.FPDFPageObj_Destroy(obj)
            if not self.dll.FPDFPage_GenerateContent(page):
                raise RuntimeError('Page regeneration failed')
            blocks = []

            def write(_self, pointer, length):
                blocks.append(C.string_at(pointer, length))
                return 1

            callback = self.callback(write)
            writer = self.Writer(1, callback)
            if not self.dll.FPDF_SaveAsCopy(document, C.byref(writer), 2):
                raise RuntimeError('Nonincremental PDFium save failed')
            output.write_bytes(b''.join(blocks))
            result['action'] = 'removed_whole_objects_only'
            return result
        finally:
            if page:
                self.dll.FPDF_ClosePage(page)
            self.dll.FPDF_CloseDocument(document)

    def close(self):
        self.dll.FPDF_DestroyLibrary()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=Path('target/release/dpdf'))
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    fixtures = module('fixture', 'probe-redaction.py')
    sample = module('sample', 'make-sample.py')
    env = dict(os.environ, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()))
    binary, qpdf = str(args.binary.resolve()), str(args.qpdf.resolve())
    # Coordinates are unrotated PDF points, bottom-left origin, on generated 400x300 pages.
    form = fixtures.stream(b'BT /F1 10 Tf 0 0 Td (SECRET) Tj ET',
                           b'/Type /XObject /Subtype /Form /BBox [0 -20 100 20] /Resources << /Font << /F1 4 0 R >> >> ')
    cases = {
        'contained-text': (fixtures.objects(b'BT /F1 10 Tf 30 240 Td (OUTSIDE) Tj ET BT /F1 10 Tf 100 150 Td (SECRET) Tj ET'), [90,130,210,175], 0),
        'crossing-text': (fixtures.objects(b'BT /F1 10 Tf 30 150 Td (OUTSIDE BEFORE SECRET AFTER OUTSIDE) Tj ET'), [120,130,180,175], None),
        'contained-path': (fixtures.objects(b'BT /F1 10 Tf 30 240 Td (OUTSIDE) Tj ET 100 150 m 200 150 l S'), [90,130,210,175], 0),
        'crossing-path': (fixtures.objects(b'2 w 20 150 m 380 150 l S'), [90,130,210,175], None),
        'contained-actualtext': (fixtures.objects(b'/Span << /ActualText (SECRET ACTUALTEXT) >> BDC BT /F1 10 Tf 100 150 Td (SECRET ACTUALTEXT) Tj ET EMC'), [90,130,230,175], 0),
        'shared-actualtext': (fixtures.objects(b'/Span << /ActualText (OUTSIDE AND SECRET) >> BDC BT /F1 10 Tf 30 240 Td (OUTSIDE) Tj ET BT /F1 10 Tf 100 150 Td (SECRET) Tj ET EMC'), [90,130,210,175], 0),
        'unused-form-resource': (fixtures.objects(b'q 1 0 0 1 100 150 cm /Fm Do Q', b'/XObject << /Fm 6 0 R >>', [form]), [90,125,210,175], 0),
        'shared-form-placement': (fixtures.objects(b'q 1 0 0 1 100 150 cm /Fm Do Q q 1 0 0 1 100 240 cm /Fm Do Q', b'/XObject << /Fm 6 0 R >>', [form]), [90,125,210,175], 1),
        'unused-image-resource': (fixtures.objects(b'q 100 0 0 30 100 140 cm /Im Do Q', b'/XObject << /Im 6 0 R >>',
                                  [fixtures.stream(bytes([20,40,210]) * 100, b'/Type /XObject /Subtype /Image /Width 10 /Height 10 /ColorSpace /DeviceRGB /BitsPerComponent 8 ')]), [90,130,210,175], 0),
    }
    report = dict(scope='generated_PDFium_whole_object_removal_probe_only', dPDF_secure_export_enabled=False,
                  qualified_for_secure_export=False, pdfium_sha256=hashlib.sha256(args.pdfium.read_bytes()).hexdigest(),
                  cases=[], blockers=[], unverified=['partial text/path/image surgery', 'nested form mutation and shared resources',
                  'crop/rotation/UserUnit coordinates', 'clipping/transparency/soft masks', 'tag trees/annotations/forms/review isolation',
                  'raster compositing and final baked-block assembly', 'Windows execution and external editor interoperability'])
    def render(path):
        frame = path.with_suffix('.ppm')
        subprocess.run([binary, '--render-page', str(path), '1', str(frame)], env=env,
                       check=True, capture_output=True, timeout=30)
        _, size, _, pixels = frame.read_bytes().split(b'\n', 3)
        width, height = map(int, size.split())
        assert len(pixels) == width * height * 3
        return width, height, pixels

    def outside_changes(before, after, region):
        width, height, old = before
        assert before[:2] == after[:2]
        new = after[2]
        left, bottom, right, top = region
        x0, x1 = left / 400 * width - 2, right / 400 * width + 2
        y0, y1 = (300 - top) / 300 * height - 2, (300 - bottom) / 300 * height + 2
        return sum(old[(y * width + x) * 3:(y * width + x) * 3 + 3] !=
                   new[(y * width + x) * 3:(y * width + x) * 3 + 3]
                   for y in range(height) for x in range(width)
                   if not (x0 <= x <= x1 and y0 <= y <= y1))

    engine = Engine(args.pdfium.resolve())
    try:
        with tempfile.TemporaryDirectory(prefix='dpdf-pdfium-removal-probe-') as directory:
            root = Path(directory)
            for name, (objects, region, expected) in cases.items():
                source, raw, cleaned = [root / (name + suffix) for suffix in ('.pdf', '-removed.pdf', '-qdf.pdf')]
                source.write_bytes(sample.write_pdf(objects))
                before_hash = hashlib.sha256(source.read_bytes()).hexdigest()
                result = dict(name=name, region=region, **engine.remove_contained(source, raw, region))
                if expected is None:
                    assert result['action'] == 'refused_crossing_object' and not raw.exists(), result
                else:
                    assert result['selected'] and result['action'] == 'removed_whole_objects_only', result
                    subprocess.run([qpdf, '--qdf', '--object-streams=disable', '--stream-data=uncompress',
                                    str(raw), str(cleaned)], check=True, capture_output=True, timeout=30)
                    probe = subprocess.run([binary, '--probe-search', str(cleaned), 'SECRET'],
                                           env=env, check=True, capture_output=True, text=True, timeout=30)
                    result['search_matches_after'] = json.loads(probe.stdout)['matches']
                    if result['search_matches_after'] != expected:
                        report['blockers'].append(name + ': removed-region text remains searchable through retained metadata/content')
                    result['outside_changed_pixels'] = outside_changes(render(source), render(cleaned), region)
                    if result['outside_changed_pixels']:
                        report['blockers'].append(name + ': rendering changed outside the region')
                    content = cleaned.read_bytes()
                    result['secret_payload_retained'] = b'SECRET' in content
                    result['image_resource_retained'] = b'/Subtype /Image' in content
                    if expected == 0 and result['secret_payload_retained']:
                        report['blockers'].append(name + ': live/extractable SECRET payload remains after object removal and qpdf rewrite')
                    if name == 'unused-image-resource' and result['image_resource_retained']:
                        report['blockers'].append(name + ': removed image remains referenced by page resources')
                assert hashlib.sha256(source.read_bytes()).hexdigest() == before_hash
                report['cases'].append(result)
                print(name + ': ' + result['action'])
    finally:
        engine.close()
    args.output.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(blockers=report['blockers'], qualified_for_secure_export=False)))
    return 3 if report['blockers'] else 0


if __name__ == '__main__':
    raise SystemExit(main())
