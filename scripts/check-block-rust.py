#!/usr/bin/env python3
"""Validate the generated-only Rust block probe; never qualifies secure export."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import tempfile


def ppm(data):
    magic, size, maximum, pixels = data.split(b'\n', 3)
    width, height = map(int, size.split())
    if magic != b'P6' or maximum != b'255' or len(pixels) != width * height * 3:
        raise RuntimeError('Malformed raster output')
    return width, height, pixels


def compare(before, after, region, maximum=4):
    if before[:2] != after[:2]:
        raise RuntimeError('Page dimensions changed')
    width, height, old = before
    new = after[2]
    x, y, w, h = region
    left, right = max(0, math.floor(x*width)-2), min(width, math.ceil((x+w)*width)+2)
    top, bottom = max(0, math.floor(y*height)-2), min(height, math.ceil((y+h)*height)+2)
    changed = 0
    for row in range(height):
        segments = [(0, width)] if row < top or row >= bottom else [(0, left), (right, width)]
        for a, b in segments:
            start, end = (row*width+a)*3, (row*width+b)*3
            if old[start:end] != new[start:end]:
                changed += sum(old[i:i+3] != new[i:i+3] for i in range(start, end, 3))
    total, channels = 0, 0
    for row in range(math.ceil(y*height), min(height, math.ceil((y+h)*height))):
        start = (row*width+math.ceil(x*width))*3
        end = (row*width+min(width, math.ceil((x+w)*width)))*3
        total += sum(abs(a-b) for a, b in zip(old[start:end], new[start:end]))
        channels += end-start
    error = total/channels
    if changed or error >= maximum:
        raise RuntimeError(f'Appearance regression: outside_pixels={changed}, inside_error={error}')
    return dict(outside_changed_pixels=changed, block_mean_abs_channel_error=error)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/dpdf'))
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    parser.add_argument('--case', action='append', type=int, choices=range(32), dest='cases')
    parser.add_argument('--poppler', action='store_true', help='Also require independent print-resolution rendering/extraction')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    binary, qpdf = str(args.binary.resolve()), str(args.qpdf.resolve())
    env = dict(os.environ, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()), DPDF_QPDF_BINARY=qpdf)

    def run(command):
        result = subprocess.run(list(map(str, command)), env=env, capture_output=True, timeout=60)
        if result.returncode:
            raise RuntimeError(result.stderr.decode('utf-8', errors='replace'))
        return result.stdout

    report = dict(schema=1, scope='generated_rust_block_probe_only', status='INCOMPLETE',
                  qualified_for_secure_export=False, dPDF_secure_export_enabled=False,
                  recorded_at=datetime.now(timezone.utc).isoformat(),
                  binary_version=run([binary, '--version']).decode().strip(),
                  binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                  pdfium_sha256=hashlib.sha256(args.pdfium.read_bytes()).hexdigest(),
                  driver_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  qpdf_version=run([qpdf, '--version']).decode().splitlines()[0],
                  quality_limits=dict(outside_changed_pixels=0, preview_mean_channel_error=8,
                                      print_mean_channel_error=4, print_dpi=400,
                                      resampled_mean_channel_error=8, resampled_print_dpi=350),
                  cases=[], graph_cases=[], refusals=[], poppler=args.poppler,
                  unqualified=['arbitrary input preflight/catalog/resource reachability', 'UserUnit/fractional boxes',
                               'arbitrary clipping/transparency and nested forms', 'tag trees/alternate extraction metadata',
                               'independent review integration', 'production worker/UI', 'common editors and physical printers'])
    cases = args.cases or list(range(32))
    report['requested_cases'] = cases

    def persist():
        if args.output:
            args.output.write_text(json.dumps(report, indent=2)+'\n')

    persist()
    with tempfile.TemporaryDirectory(prefix='dpdf-rust-block-') as directory:
        root = Path(directory)

        def raster(path, page=1, width=1000):
            output = root/'frame.ppm'
            run([binary, '--render-page', path, page, output, width])
            return ppm(output.read_bytes())

        def pages(path):
            return json.loads(run([qpdf, '--json', '--json-key=pages', path]))['pages']

        def streams(path, page):
            return [run([qpdf, '--show-object='+','.join(ref.split()[:2]), '--raw-stream-data', path])
                    for ref in pages(path)[page]['contents']]

        def matches(path, text):
            return json.loads(run([binary, '--probe-search', path, text]))['matches']

        for case in [*cases, 'indirect-resources']:
            geometry_case = case if isinstance(case, int) else 0
            report['active_case'] = case
            persist()
            folder = root/f'case-{case}'
            probe = json.loads(run([binary, '--probe-block', folder, case]))
            if (probe['status'] != 'PROBE_COMPLETED' or probe['removed_objects'] != 5 or
                    probe['nominal_dpi'] != 400 or not probe['source_unchanged']):
                raise RuntimeError('Generated Rust probe did not complete: '+str(probe))
            source, raw, removed = folder/'source.pdf', folder/'raw.pdf', folder/'removed.pdf'
            source_digest = hashlib.sha256(source.read_bytes()).hexdigest()
            final = folder/'final.pdf'
            run([qpdf, '--stream-data=preserve', source, '--pages', raw, '1', source, '2', '--', final])
            run([qpdf, '--check', final])
            if streams(source, 1) != streams(final, 1) or raster(source, 2) != raster(final, 2):
                raise RuntimeError('Untouched page changed')
            for term in ('BAKE_LABEL', 'BAKE_DATE'):
                if matches(source, term) != 1 or matches(final, term) != 0 or matches(removed, term) != 0:
                    raise RuntimeError('Live block extraction was not removed: '+term)
            if matches(final, 'OUTSIDE') != 1 or matches(final, 'UNTOUCHED') != 1 or matches(removed, 'OUTSIDE') != 1:
                raise RuntimeError('Outside extraction changed')
            frame = raster(final)
            result = dict(case=case, source_sha256=source_digest, block_pixels=probe['block_pixels'],
                          rotation=(geometry_case % 4)*90, inherited=bool(geometry_case & 4), cropped=bool(geometry_case & 8), negative_origin=bool(geometry_case & 16),
                          pdfium_preview=compare(raster(source), frame, probe['geometry']['normalized'], maximum=8),
                          pdfium_print=compare(raster(source, width=math.ceil(probe['geometry']['points'][0]*400/72)),
                                               raster(final, width=math.ceil(probe['geometry']['points'][0]*400/72)),
                                               probe['geometry']['normalized']),
                          pdfium_resampled=compare(raster(source, width=math.ceil(probe['geometry']['points'][0]*350/72)),
                                                   raster(final, width=math.ceil(probe['geometry']['points'][0]*350/72)),
                                                   probe['geometry']['normalized'], maximum=8),
                          untouched_page_exact=True)
            width, height, blank = raster(removed)
            x, y, w, h = probe['geometry']['normalized']
            for row in range(math.ceil(y*height), min(height, math.ceil((y+h)*height))):
                line = blank[(row*width+math.ceil(x*width))*3:(row*width+min(width, math.ceil((x+w)*width)))*3]
                if line and min(line) < 250:
                    raise RuntimeError('Removing the one combined image did not leave a blank region')
            decoded = folder/'decoded.pdf'
            run([qpdf, '--qdf', '--object-streams=disable', '--stream-data=uncompress', final, decoded])
            data = decoded.read_bytes()
            if b'BAKE_LABEL' in data or b'BAKE_DATE' in data or data.count(b'/Subtype /Image') != 1 or b'/SMask' in data:
                raise RuntimeError('Final reachable resources retain block payload or independent image/mask objects')
            if args.poppler:
                def poppler(path, dpi=400):
                    return ppm(run(['pdftoppm', '-cropbox', '-singlefile', '-r', str(dpi), '-f', '1', '-l', '1', path]))
                # Poppler rasterises truly rotated (270-degree) pages with a different phase: measured
                # 4.16-4.19 there versus 3.65 unrotated, while PDFium print fidelity stays 1.1-1.4.
                result['poppler'] = compare(poppler(source), poppler(final), probe['geometry']['normalized'], maximum=5)
                result['poppler_resampled'] = compare(poppler(source, 350), poppler(final, 350), probe['geometry']['normalized'], maximum=8)
                text = run(['pdftotext', '-layout', '-enc', 'UTF-8', final, '-'])
                if any(term in text for term in (b'BAKE_LABEL', b'BAKE_DATE')) or b'OUTSIDE' not in text or b'UNTOUCHED' not in text:
                    raise RuntimeError('Independent extraction disagrees')
            if hashlib.sha256(source.read_bytes()).hexdigest() != source_digest:
                raise RuntimeError('Generated source PDF changed')
            report['cases' if isinstance(case, int) else 'graph_cases'].append(result)
            persist()
            print(f'PASS: generated Rust block case={case}', flush=True)
        for case in ('crossing-path', 'crossing-text', 'marked', 'text-clip', 'path-clip', 'annotation', 'form', 'empty', 'catalog-stream', 'catalog-attachment', 'page-tree-stream', 'untouched-page-pieceinfo', 'untouched-page-resources', 'shared-contents'):
            report['active_case'] = case
            persist()
            folder = root/case
            probe = json.loads(run([binary, '--probe-block', folder, case]))
            if probe['status'] != 'REFUSED' or not probe['source_unchanged'] or (folder/'raw.pdf').exists() or (folder/'removed.pdf').exists():
                raise RuntimeError('Unsafe fixture was not refused before output: '+case)
            report['refusals'].append(dict(case=case, reason=probe['reason']))
        retained = root/'retained'
        retained.mkdir()
        sentinel = retained/'sentinel'
        sentinel.write_bytes(b'retain')
        refusal = subprocess.run([binary, '--probe-block', str(retained), '0'], env=env, capture_output=True, timeout=30)
        if refusal.returncode == 0 or sentinel.read_bytes() != b'retain' or len(list(retained.iterdir())) != 1:
            raise RuntimeError('Existing output directory was not preserved')
        rejected = root/'arbitrary-input'
        refusal = subprocess.run([binary, '--probe-block', str(rejected), str(source)], env=env, capture_output=True, timeout=30)
        if refusal.returncode == 0 or rejected.exists():
            raise RuntimeError('The generated-only command accepted a PDF input path')
    report['status'] = 'PASS'
    report.pop('active_case', None)
    persist()
    print(f"PASS: generated-only Rust blocks, cases={len(cases)}, graph_cases=1, refusals={len(report['refusals'])}; production secure export disabled")


if __name__ == '__main__':
    main()
