#!/usr/bin/env python3
"""Generated-only combined-block experiment. NOT a production secure export backend."""
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
SCRIPTS = Path(__file__).resolve().parent

def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / filename)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value

class Refused(Exception):
    pass

class Matrix(C.Structure):
    _fields_ = [(name, C.c_float) for name in ('a', 'b', 'c', 'd', 'e', 'f')]

class Clip(C.Structure):
    _fields_ = [(name, C.c_float) for name in ('left', 'top', 'right', 'bottom')]
Base = module('removal', 'probe-pdfium-removal.py').Engine

class Engine(Base):

    def __init__(self, path):
        super().__init__(path)
        p, i = (C.c_void_p, C.c_int)
        signatures = {
            'FPDFBitmap_CreateEx': ([i, i, i, p, i], p),
            'FPDFBitmap_Destroy': ([p], None),
            'FPDFBitmap_FillRect': ([p, i, i, i, i, C.c_ulong], i),
            'FPDF_RenderPageBitmapWithMatrix': ([p, p, C.POINTER(Matrix), C.POINTER(Clip), i], None),
            'FPDF_GetPageWidthF': ([p], C.c_float),
            'FPDF_GetPageHeightF': ([p], C.c_float),
            'FPDFPage_GetRotation': ([p], i),
            'FPDFPage_GetAnnotCount': ([p], i),
            'FPDFPageObj_NewImageObj': ([p], p),
            'FPDFImageObj_SetBitmap': ([C.POINTER(p), i, p, p], i),
            'FPDFPageObj_Transform': ([p, *[C.c_double] * 6], None),
            'FPDFPage_InsertObject': ([p, p], None),
            'FPDFPageObj_GetClipPath': ([p], p),
            'FPDFClipPath_CountPaths': ([p], i),
            'FPDFTextObj_GetTextRenderMode': ([p], i),
        }
        signatures.update({'FPDF_PageToDevice': ([p, *[i]*5, C.c_double, C.c_double, C.POINTER(i), C.POINTER(i)], i),
                           'FPDF_DeviceToPage': ([p, *[i]*7, C.POINTER(C.c_double), C.POINTER(C.c_double)], i)})
        for name, (args, result) in signatures.items():
            fn = getattr(self.dll, name)
            fn.argtypes = args
            fn.restype = result

    def project_region(self, page, region):
        page_width = self.dll.FPDF_GetPageWidthF(page)
        page_height = self.dll.FPDF_GetPageHeightF(page)
        if not all(math.isfinite(v) and 0 < v <= 2000 for v in (page_width, page_height)):
            raise Refused('unsupported generated page dimensions')
        left, bottom, right, top = region
        points = [(left, bottom), (right, bottom), (right, top), (left, top)]
        device = []
        for x, y in points:
            dx, dy = C.c_int(), C.c_int()
            if not self.dll.FPDF_PageToDevice(page, 0, 0, 10000, 10000, 0, x, y, C.byref(dx), C.byref(dy)):
                raise Refused('page-to-device mapping failed')
            device.append((dx.value, dy.value))
        x0, x1 = min(x for x, _ in device), max(x for x, _ in device)
        y0, y1 = min(y for _, y in device), max(y for _, y in device)
        if not (0 <= x0 < x1 <= 10000 and 0 <= y0 < y1 <= 10000):
            raise Refused('block does not fit the native displayed page')
        corners = []
        # Image unit-square origin is bottom-left; source bitmap rows start at top.
        for x, y in [(x0, y1), (x1, y1), (x0, y0), (x1, y0)]:
            px, py = C.c_double(), C.c_double()
            if not self.dll.FPDF_DeviceToPage(page, 0, 0, 10000, 10000, 0, x, y, C.byref(px), C.byref(py)):
                raise Refused('device-to-page mapping failed')
            corners.append((px.value, py.value))
        bl, br, tl, _ = corners
        transform = [br[0]-bl[0], br[1]-bl[1], tl[0]-bl[0], tl[1]-bl[1], bl[0], bl[1]]
        effective = [min(x for x, _ in corners), min(y for _, y in corners),
                     max(x for x, _ in corners), max(y for _, y in corners)]
        return dict(normalized=[x0/10000, y0/10000, (x1-x0)/10000, (y1-y0)/10000],
                    effective_pdf_region=effective, image_matrix=transform,
                    page_points=[page_width, page_height],
                    displayed_block_points=[(x1-x0)/10000*page_width, (y1-y0)/10000*page_height])

    def region_geometry(self, source, region):
        data = C.create_string_buffer(source.read_bytes())
        document = self.dll.FPDF_LoadMemDocument64(data, len(data.raw)-1, None)
        if not document:
            raise RuntimeError('Cannot read generated page geometry')
        page = None
        try:
            page = self.dll.FPDF_LoadPage(document, 0)
            if not page:
                raise RuntimeError('Cannot load generated page geometry')
            return self.project_region(page, region)
        finally:
            if page:
                self.dll.FPDF_ClosePage(page)
            self.dll.FPDF_CloseDocument(document)

    def bake_generated(self, source, output, region):
        data = C.create_string_buffer(source.read_bytes())
        document = self.dll.FPDF_LoadMemDocument64(data, len(data.raw) - 1, None)
        if not document:
            raise RuntimeError('Could not open generated experiment')
        page = bitmap = pending_image = None
        try:
            page = self.dll.FPDF_LoadPage(document, 0)
            if not page:
                raise RuntimeError('Could not load experiment page')
            geometry = self.project_region(page, region)
            if self.dll.FPDFPage_GetAnnotCount(page):
                raise Refused('annotations require independent review handling')
            left, bottom, right, top = geometry["effective_pdf_region"]
            selected = []
            types = []
            count = self.dll.FPDFPage_CountObjects(page)
            if not 0 < count <= 4000:
                raise Refused("empty or oversized page object list")
            for index in range(count):
                obj = self.dll.FPDFPage_GetObject(page, index)
                kind = self.dll.FPDFPageObj_GetType(obj)
                if kind not in (1, 2, 3):
                    raise Refused('form/shading or unsupported page object')
                if self.dll.FPDFPageObj_CountMarks(obj):
                    raise Refused('marked content / alternate extraction text')
                if kind == 1 and self.dll.FPDFTextObj_GetTextRenderMode(obj) >= 4:
                    raise Refused('text clipping requires unsupported graphics-state handling')
                clip_path = self.dll.FPDFPageObj_GetClipPath(obj)
                # A non-null handle alone does not establish a clip: PDFium also
                # returns handles whose CountPaths is -1. Zero can represent a
                # text clip and must still be refused before GenerateContent.
                if clip_path and self.dll.FPDFClipPath_CountPaths(clip_path) >= 0:
                    raise Refused('clipping requires unsupported graphics-state handling')
                values = [C.c_float() for _ in range(4)]
                if not self.dll.FPDFPageObj_GetBounds(obj, *[C.byref(value) for value in values]):
                    raise Refused('object bounds unavailable')
                x0, y0, x1, y1 = [value.value for value in values]
                if not all((math.isfinite(value) for value in (x0, y0, x1, y1))):
                    raise Refused('nonfinite bounds')
                intersects = x1 > left and x0 < right and (y1 > bottom) and (y0 < top)
                contained = x0 >= left and x1 <= right and (y0 >= bottom) and (y1 <= top)
                if intersects and (not contained):
                    raise Refused('object crosses block boundary')
                if intersects:
                    selected.append(obj)
                    types.append(kind)
            if not selected:
                raise Refused('empty selected block')
            block_width, block_height = geometry["displayed_block_points"]
            width = math.ceil(block_width * 350 / 72)
            height = math.ceil(block_height * 350 / 72)
            if not (0 < width <= 4096 and 0 < height <= 4096 and width * height <= 8 * 1024 ** 2):
                raise Refused('block exceeds the generated experiment raster budget')
            # Opaque BGR: the result has no independently removable alpha mask.
            bitmap = self.dll.FPDFBitmap_CreateEx(width, height, 2, None, 0)
            if not bitmap:
                raise RuntimeError('Could not allocate block bitmap')
            self.dll.FPDFBitmap_FillRect(bitmap, 0, 0, width, height, 4294967295)
            sx, sy = (width / block_width, height / block_height)
            # PDFium first applies the default page-to-device display transform.
            # This additional matrix crops/scales in that device coordinate space.
            nx, ny, _, _ = geometry["normalized"]
            pw, ph = geometry["page_points"]
            matrix = Matrix(sx, 0, 0, sy, -nx * pw * sx, -ny * ph * sy)
            clip = Clip(0, 0, width, height)
            self.dll.FPDF_RenderPageBitmapWithMatrix(bitmap, page, C.byref(matrix), C.byref(clip), 0)
            for obj in selected:
                if not self.dll.FPDFPage_RemoveObject(page, obj):
                    raise RuntimeError('Object removal failed')
                self.dll.FPDFPageObj_Destroy(obj)
            pending_image = self.dll.FPDFPageObj_NewImageObj(document)
            if not pending_image:
                raise RuntimeError('Could not create combined image')
            page_array = (C.c_void_p * 1)(page)
            if not self.dll.FPDFImageObj_SetBitmap(page_array, 1, pending_image, bitmap):
                raise RuntimeError('Could not populate combined image')
            self.dll.FPDFPageObj_Transform(pending_image, *geometry["image_matrix"])
            self.dll.FPDFPage_InsertObject(page, pending_image)
            pending_image = None
            if not self.dll.FPDFPage_GenerateContent(page):
                raise RuntimeError('Could not regenerate page')
            blocks = []

            def write(_self, pointer, length):
                blocks.append(C.string_at(pointer, length))
                return 1
            callback = self.callback(write)
            writer = self.Writer(1, callback)
            if not self.dll.FPDF_SaveAsCopy(document, C.byref(writer), 2):
                raise RuntimeError('Nonincremental save failed')
            output.write_bytes(b''.join(blocks))
            return dict(removed_objects=len(selected), removed_types=types, block_pixels=[width, height], nominal_dpi=350, geometry=geometry)
        finally:
            if pending_image:
                self.dll.FPDFPageObj_Destroy(pending_image)
            if page:
                self.dll.FPDF_ClosePage(page)
            self.dll.FPDF_CloseDocument(document)
            if bitmap:
                self.dll.FPDFBitmap_Destroy(bitmap)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('/app/dPDF/dpdf'))
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    parser.add_argument('--artifacts', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not __debug__:
        parser.error('Run diagnostics without Python optimization so validation checks remain enabled')
    sample = module('sample', 'make-sample.py')
    images = module('page_fixture', 'check-pages.py')
    binary, qpdf = (str(args.binary.resolve()), str(args.qpdf.resolve()))
    env = dict(os.environ, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()), DPDF_QPDF_BINARY=qpdf)

    def run(command):
        return subprocess.run(command, env=env, check=True, capture_output=True, timeout=60)

    def fixture(main, annot=False, form=False, rotation=0):
        other = b'BT /F1 16 Tf 30 240 Td (UNTOUCHED second page) Tj ET\n'

        def stream(data):
            return f'<< /Length {len(data)} >>\nstream\n'.encode() + data + b'endstream'
        objects = [b'<< /Type /Catalog /Pages 2 0 R >>', b'<< /Type /Pages /Count 2 /Kids [3 0 R 6 0 R] >>', b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 300] /Resources << /Font << /F1 4 0 R >> ' + (b'/XObject << /Fm 8 0 R >> ' if form else b'') + b'>> /Rotate ' + str(rotation).encode() + b' /Contents 5 0 R ' + (b'/Annots [8 0 R] ' if annot else b'') + b'>>', b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>', stream(main), b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 300] /Resources << /Font << /F1 4 0 R >> >> /Contents 7 0 R >>', stream(other)]
        if form:
            objects.append(stream(b'BT /F1 10 Tf 0 10 Td (FORM LABEL) Tj ET').replace(b'<< /Length', b'<< /Type /XObject /Subtype /Form /BBox [0 0 100 30] /Resources << /Font << /F1 4 0 R >> >> /Length'))
        if annot:
            objects.append(b'<< /Type /Annot /Subtype /Text /Rect [100 100 120 120] /Contents (independent remark) /P 3 0 R >>')
        return sample.write_pdf(objects)
    report = dict(scope='generated_combined_block_prototype_only', qualified_for_secure_export=False, dPDF_secure_export_enabled=False, binary_version=run([binary, '--version']).stdout.decode().strip(), binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(), pdfium_sha256=hashlib.sha256(args.pdfium.read_bytes()).hexdigest(), checks=[], unverified=['arbitrary input preflight/catalog references and resource reachability', 'arbitrary/fractional geometry and UserUnit; clipping/transparency', 'nested forms and shared resources', 'tag trees and alternate extraction metadata', 'independent remarks export integration', 'Windows execution and editor interoperability', 'production Rust worker/UI integration and safety review'])
    region = [90, 60, 280, 140]
    with tempfile.TemporaryDirectory(prefix='dpdf-combined-block-') as directory:
        root = Path(directory)
        original = root / 'original.pdf'
        overlay = root / 'overlay.pdf'
        raw = root / 'raw.pdf'
        final = root / 'baked.pdf'
        content = b'BT /F1 16 Tf 30 240 Td (OUTSIDE searchable text) Tj ET\nBT /F1 10 Tf 100 105 Td (BAKE_LABEL Authorized person:) Tj ET\n100 90 m 270 90 l S\nBT /F1 10 Tf 100 70 Td (BAKE_DATE 2026) Tj ET\n'
        original.write_bytes(fixture(content))
        mark = root / 'mark.png'
        mark.write_bytes(images.png(120, 80))
        recipe = root / 'placements.json'
        recipe.write_text(json.dumps([dict(page=1, image=mark.name, geometry=dict(x_mm=125 * 25.4 / 72, y_mm=175 * 25.4 / 72, width_mm=100 * 25.4 / 72, height_mm=40 * 25.4 / 72), opacity=0.6)]))
        stamp = root / 'stamp.png'
        stamp.write_bytes(images.png(80, 120))
        specs = json.loads(recipe.read_text())
        specs.append(dict(page=1, image=stamp.name, geometry=dict(x_mm=180 * 25.4 / 72, y_mm=175 * 25.4 / 72, width_mm=70 * 25.4 / 72, height_mm=50 * 25.4 / 72), opacity=0.4))
        recipe.write_text(json.dumps(specs))
        run([binary, '--overlay', str(original), str(overlay), str(recipe)])
        original_hash = hashlib.sha256(original.read_bytes()).hexdigest()
        overlay_hash = hashlib.sha256(overlay.read_bytes()).hexdigest()

        def pages(path):
            return json.loads(run([qpdf, '--json', '--json-key=pages', str(path)]).stdout)['pages']

        def streams(path, page):
            return [run([qpdf, '--show-object=' + ','.join(ref.split()[:2]), '--raw-stream-data', str(path)]).stdout for ref in pages(path)[page]['contents']]

        def raster(path, page=1):
            ppm = root / 'frame.ppm'
            run([binary, '--render-page', str(path), str(page), str(ppm)])
            _, size, _, data = ppm.read_bytes().split(b'\n', 3)
            width, height = map(int, size.split())
            assert len(data) == width * height * 3
            return (width, height, data)

        def search(path, text):
            return json.loads(run([binary, '--probe-search', str(path), text]).stdout)['matches']
        before = raster(overlay)
        engine = Engine(args.pdfium.resolve())
        try:
            report['bake'] = engine.bake_generated(overlay, raw, region)
            run([qpdf, '--stream-data=preserve', str(original), '--pages', str(raw), '1', str(original), '2', '--', str(final)])
            run([qpdf, '--check', str(final)])
            assert streams(final, 1) == streams(original, 1) and raster(final, 2) == raster(original, 2)
            assert search(overlay, 'BAKE_LABEL') == 1 and search(final, 'BAKE_LABEL') == 0
            assert search(final, 'BAKE_DATE') == 0 and search(final, 'OUTSIDE') == 1
            after = raster(final)
            assert before[:2] == after[:2]
            width, height = before[:2]
            old, new = (before[2], after[2])
            left, bottom, right, top = region
            x0, x1 = (left / 400 * width - 2, right / 400 * width + 2)
            y0, y1 = ((300 - top) / 300 * height - 2, (300 - bottom) / 300 * height + 2)
            outside_changes = sum((old[(y * width + x) * 3:(y * width + x) * 3 + 3] != new[(y * width + x) * 3:(y * width + x) * 3 + 3] for y in range(height) for x in range(width) if not (x0 <= x <= x1 and y0 <= y <= y1)))
            assert outside_changes == 0, outside_changes
            differences = [abs(a - b) for a, b in zip(old, new)]
            mae = sum(differences) / len(differences)
            inside = [(y * width + x) * 3 for y in range(height) for x in range(width) if left / 400 * width <= x < right / 400 * width and (300 - top) / 300 * height <= y < (300 - bottom) / 300 * height]
            inside_mae = sum((abs(old[index + c] - new[index + c]) for index in inside for c in range(3))) / (len(inside) * 3)
            assert inside_mae < 4, inside_mae
            assert len(pages(final)[0]['images']) == 1
            removed = root / 'removed-combined.pdf'
            effective = report['bake']['geometry']['effective_pdf_region']
            result = engine.remove_contained(final, removed, [effective[0]-.02, effective[1]-.02, effective[2]+.02, effective[3]+.02])
            assert result['selected'] == 1 and [o['type'] for o in result['objects'] if o['intersects']] == [3]
            assert search(removed, 'BAKE_LABEL') == 0 and search(removed, 'OUTSIDE') == 1
            removed_pixels = raster(removed)[2]
            assert all((removed_pixels[index + c] >= 250 for index in inside for c in range(3)))
            decoded = root / 'decoded.pdf'
            run([qpdf, '--qdf', '--object-streams=disable', '--stream-data=uncompress', str(final), str(decoded)])
            reachable = decoded.read_bytes()
            assert b'BAKE_LABEL' not in reachable and b'BAKE_DATE' not in reachable
            assert reachable.count(b'/Subtype /Image') == 1
            text = run(['pdftotext', '-layout', '-enc', 'UTF-8', str(final), '-']).stdout
            assert b'BAKE_LABEL' not in text and b'BAKE_DATE' not in text and (b'OUTSIDE' in text) and (b'UNTOUCHED' in text)
            poppler = []
            for path in (overlay, final):
                data = run(['pdftoppm', '-cropbox', '-singlefile', '-r', '350', '-f', '1', '-l', '1', str(path)]).stdout
                magic, size, maximum, pixels = data.split(b'\n', 3)
                pw, ph = map(int, size.split())
                assert magic == b'P6' and maximum == b'255' and (len(pixels) == pw * ph * 3)
                poppler.append((pw, ph, pixels))
            assert poppler[0][:2] == poppler[1][:2]
            po, pn = (poppler[0][2], poppler[1][2])
            outside = 0
            total = 0
            channels = 0
            for y in range(ph):
                for x in range(pw):
                    index = (y * pw + x) * 3
                    if left / 400 * pw - 2 <= x <= right / 400 * pw + 2 and (300 - top) / 300 * ph - 2 <= y <= (300 - bottom) / 300 * ph + 2:
                        total += sum((abs(po[index + c] - pn[index + c]) for c in range(3)))
                        channels += 3
                    else:
                        outside += po[index:index + 3] != pn[index:index + 3]
            assert outside == 0, outside
            if args.artifacts:
                args.artifacts.mkdir(parents=True, exist_ok=False)
                for name, frame in [('pdfium-before', before), ('pdfium-after', after), ('poppler-before', poppler[0]), ('poppler-after', poppler[1])]:
                    w, h, pixels = frame
                    (args.artifacts / (name + '.ppm')).write_bytes(f'P6\n{w} {h}\n255\n'.encode() + pixels)
                import shutil
                for path in (original, overlay, final):
                    shutil.copyfile(path, args.artifacts / path.name)
            assert total / channels < 4, total / channels
            report['poppler'] = dict(render_dpi=350, outside_changed_pixels=outside, block_mean_abs_channel_error=total / channels, extraction_inside_absent_outside_present=True)
            report['checks'].append(dict(name='combined_raster_replaces_live_label_line_date_and_overlay', result='PASS', outside_changed_pixels=outside_changes, whole_page_mean_abs_channel_error=mae, block_mean_abs_channel_error=inside_mae, output_images=1, removable_objects_in_block=1, untouched_page_exact=True))
            for name, body, annot, form, rotation in [('crossing-path', b'20 90 m 380 90 l S', False, False, 0), ('crossing-text', b'BT /F1 10 Tf 20 100 Td (OUTSIDE BEFORE LABEL AFTER OUTSIDE) Tj ET', False, False, 0), ('shared-ActualText', b'/Span << /ActualText (OUTSIDE BAKE_LABEL) >> BDC ' + content + b' EMC', False, False, 0), ('independent-remark', content, True, False, 0), ('form', b'q 1 0 0 1 100 80 cm /Fm Do Q', False, True, 0), ('empty-block', b'BT /F1 16 Tf 30 240 Td (OUTSIDE only) Tj ET', False, False, 0), ('shared-text-clip', b'BT /F1 16 Tf 30 240 Td (OUTSIDE) Tj ET q BT /F1 20 Tf 100 100 Td 7 Tr (HIDDEN_CLIP) Tj ET 30 240 10 10 re f 100 80 100 50 re f Q', False, False, 0), ('path-clip', b'q 90 60 190 80 re W n 0 0 400 300 re f Q', False, False, 0)]:
                source = root / (name + '.pdf')
                output = root / (name + '-output.pdf')
                source.write_bytes(fixture(body, annot, form, rotation))
                digest = hashlib.sha256(source.read_bytes()).hexdigest()
                try:
                    engine.bake_generated(source, output, region)
                except Refused as error:
                    report['checks'].append(dict(name=name, result='REFUSED before mutation', reason=str(error)))
                else:
                    raise AssertionError('Unsupported fixture was not refused: ' + name)
                assert not output.exists() and hashlib.sha256(source.read_bytes()).hexdigest() == digest
            assert hashlib.sha256(original.read_bytes()).hexdigest() == original_hash and hashlib.sha256(overlay.read_bytes()).hexdigest() == overlay_hash
        finally:
            engine.close()
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))
if __name__ == '__main__':
    main()
