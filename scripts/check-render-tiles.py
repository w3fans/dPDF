#!/usr/bin/env python3
"""Native tile/reference comparison with strict diagnostics and bounded native edge rounding."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def form_fixture(sample):
    text = b'BT /Helv 18 Tf 40 340 Td (Generated form and annotation fixture) Tj ET\n'
    appearance = b'q 0.8 0.9 0.3 rg 0 0 340 60 re f Q\nBT /Helv 18 Tf 12 22 Td (FORM FIELD ALPHA) Tj ET\n'
    return sample.write_pdf([
        b'<< /Type /Catalog /Pages 2 0 R /AcroForm 9 0 R >>',
        b'<< /Type /Pages /Count 1 /Kids [4 0 R] >>',
        b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 400] /Resources << /Font << /Helv 3 0 R >> >> /Contents 5 0 R /Annots [6 0 R 7 0 R] >>',
        f'<< /Length {len(text)} >>\nstream\n'.encode() + text + b'endstream',
        b'<< /Type /Annot /Subtype /Widget /FT /Tx /T (GeneratedField) /V (FORM FIELD ALPHA) /Rect [80 220 420 280] /P 4 0 R /F 4 /DA (/Helv 18 Tf 0 g) /AP << /N 8 0 R >> >>',
        b'<< /Type /Annot /Subtype /Square /Rect [80 70 420 150] /C [0 0 1] /Border [0 0 2] /F 4 >>',
        f'<< /Type /XObject /Subtype /Form /BBox [0 0 340 60] /Resources << /Font << /Helv 3 0 R >> >> /Length {len(appearance)} >>\nstream\n'.encode() + appearance + b'endstream',
        b'<< /Fields [6 0 R] /DR << /Font << /Helv 3 0 R >> >> /DA (/Helv 18 Tf 0 g) >>'])


def jpeg_fixture(sample, source):
    # Existing generated photo fixture, also used by check-pages.py (400x200).
    jpeg = (source / 'fixtures' / 'page-photo.jpg').read_bytes()
    content = b'q 532 0 0 266 40 200 cm /Photo Do Q\n'
    return sample.write_pdf([
        b'<< /Type /Catalog /Pages 2 0 R >>',
        b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /XObject << /Photo 4 0 R >> >> /Contents 5 0 R >>',
        f'<< /Type /XObject /Subtype /Image /Width 400 /Height 200 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {len(jpeg)} >>\nstream\n'.encode() + jpeg + b'\nendstream',
        f'<< /Length {len(content)} >>\nstream\n'.encode() + content + b'endstream'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    source = Path(__file__).resolve().parent
    sample = load('tile_sample', source / 'make-sample.py')
    bench = load('tile_bench', source / 'benchmark-reader.py')
    env = dict(os.environ, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()))
    binary = str(args.binary.resolve())
    cases = []
    for intrinsic in (0, 90, 180, 270):
        for cropped in (False, True):
            for unit in (0.5, 2):
                cases.append((f'geometry-{intrinsic}-{cropped}-{unit}',
                              bench.geometry_fixture(sample, intrinsic, cropped, True, unit)))
    cases.extend([('large-image', bench.image_fixture(sample, 1)), ('jpeg-image', jpeg_fixture(sample, source)), ('form-annotation', form_fixture(sample))])
    count = 0
    observations = dict(schema=1, scope='bounded_native_tile_comparison_review_gated',
                        binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                        pdfium_sha256=hashlib.sha256(args.pdfium.read_bytes()).hexdigest(), status='IN_PROGRESS', cases=[])
    with tempfile.TemporaryDirectory(prefix='dpdf-render-tiles-') as directory:
        root = Path(directory)
        for name, content in cases:
            path = root / (name + '.pdf')
            path.write_bytes(content)
            before = hashlib.sha256(content).hexdigest()
            for turn in (0, 1, 2, 3):
                command = [binary, '--check-render-tiles', str(path), '1', '3600', '2400', str(turn)]
                reference = root / 'form-reference.ppm'
                if name == 'form-annotation' and turn == 0:
                    command.append(str(reference))
                case_result = dict(name=name, turn=turn, input_sha256=before, status='FAIL')
                observations['cases'].append(case_result)
                result = None
                try:
                    result = subprocess.run(command, env=env, text=True, capture_output=True, timeout=90)
                    report = json.loads(result.stdout)
                    # Strict CLI remains exact and refuses every changed pixel. The
                    # corpus contract separately bounds independently reproduced
                    # native origin-dependent text-edge rounding; never hide a crash.
                    changed = report['changed_pixels']
                    controls = report['native_origin_controls']
                    assert [item['offset'] for item in controls] == ([1, 127, 509] if changed else []), (name, turn, report)
                    assert (result.returncode == 0 if changed == 0 else
                            result.returncode == 1 and 'Detail tiles differ from the native full-page reference' in result.stderr), (name, turn, result.stderr, report)
                    case_result.update(strict_exit_code=result.returncode, comparison=report)
                    assert report['forms_enabled'] is True
                    # PDFium does not rasterize a clipped tile bit-identically to a
                    # full page: allow edge-only antialiasing and one-pixel fill
                    # snapping (each larger delta must exist within one reference
                    # pixel), alpha unchanged, at most 0.2% of pixels.
                    assert report['alpha_changed_pixels'] == 0 and report['non_edge_changed_pixels'] == 0, (name, turn, report)
                    assert report['unexplained_pixels'] == 0, (name, turn, report)
                    assert report['native_raster_limit_pixels'] == 3600 * 2400 // 500
                    assert changed <= report['native_raster_limit_pixels'], (name, turn, report)
                    assert report['within_native_raster_tolerance'] is True, (name, turn, report)
                    if name in ('large-image', 'jpeg-image'):
                        assert changed == 0, (name, turn, report)
                    assert 0 < report['peak_tile_bytes'] <= 512 * 512 * 4
                    assert report['tiles'] > 1
                    if reference.exists():
                        magic, size, maximum, rgb = reference.read_bytes().split(b'\n', 3)
                        assert (magic, size, maximum) == (b'P6', b'3600 2400', b'255')
                        assert len(rgb) == 3600 * 2400 * 3
                        field = b''.join(rgb[(y * 3600 + 480) * 3:(y * 3600 + 2520) * 3]
                                         for y in range(720, 1080))
                        pixels = list(zip(field[0::3], field[1::3], field[2::3]))
                        assert sum(g > 180 and r > 150 and b < 130 for r, g, b in pixels) > 10000, 'form background absent'
                        assert sum(max(p) < 80 for p in pixels) > 500, 'form value absent'
                    case_result['status'] = 'PASS_BOUNDED_COMPARISON'
                except (AssertionError, ValueError, OSError, subprocess.SubprocessError) as error:
                    case_result['error'] = str(error)
                    if result is not None:
                        case_result['stderr_tail'] = result.stderr[-4000:]
                    print(f'FAIL: {name} rotation={turn}: {error}', flush=True)
                finally:
                    reference.unlink(missing_ok=True)
                    if args.report:
                        args.report.write_text(json.dumps(observations, indent=2) + '\n')
                assert hashlib.sha256(path.read_bytes()).hexdigest() == before
                count += 1
        # Invalid regions must refuse before allocating a diagnostic full page.
        result = subprocess.run([binary, '--check-render-tiles', str(path), '1', '999999', '2400', '0'],
                                env=env, text=True, capture_output=True, timeout=20)
        assert result.returncode != 0 and 'diagnostic budget' in result.stderr
    failures = [case for case in observations['cases'] if case['status'] != 'PASS_BOUNDED_COMPARISON']
    observations['status'] = 'FAIL' if failures else 'PASS_BOUNDED_COMPARISON_REQUIRES_RENDERER_REVIEW'
    if args.report:
        args.report.write_text(json.dumps(observations, indent=2) + '\n')
    assert not failures, [(case['name'], case['turn'], case.get('error')) for case in failures]
    print(f'PASS: {count} bounded native tile/full comparisons, exact image cases, visible form appearance/value, strict mode retained and input preservation')


if __name__ == '__main__':
    main()
