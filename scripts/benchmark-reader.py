#!/usr/bin/env python3
"""Measure generated reader fixtures and verify lazy geometry against loaded pages.

Numbers describe headless engine delivery, not GUI/paper/real-office qualification.
No machine-specific timing threshold is used as a correctness test.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
import platform
from pathlib import Path
import statistics
import subprocess
import tempfile


def sample_module():
    spec = importlib.util.spec_from_file_location('sample', Path(__file__).with_name('make-sample.py'))
    sample = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(sample)
    return sample


def geometry_fixture(sample, rotation, cropped, inherited, user_unit):
    boxes = b'/MediaBox [-30 -20 370 280] ' + (b'/CropBox [0 0 340 240] ' if cropped else b'')
    boxes += f'/Rotate {rotation} '.encode()
    content = b'BT /F1 16 Tf 40 120 Td (Reader geometry) Tj ET\n'
    return sample.write_pdf([
        b'<< /Type /Catalog /Pages 2 0 R >>',
        b'<< /Type /Pages /Count 1 /Kids [3 0 R] ' + (boxes if inherited else b'') + b'>>',
        b'<< /Type /Page /Parent 2 0 R ' + (b'' if inherited else boxes) +
        f'/UserUnit {user_unit} '.encode() + b'/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>',
        b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
        f'<< /Length {len(content)} >>\nstream\n'.encode() + content + b'endstream',
    ])


def vector_fixture(sample, count=50, detail=1000):
    content = b'BT /F1 20 Tf 40 740 Td (Generated vector contract) Tj ET\n'
    content += b'0.6 0.6 0.6 RG 0.2 w\n'
    content += b''.join(f'{40+i%50*10} {160+i//50*14} 8 10 re S\n'.encode() for i in range(detail))
    objects = [b'<< /Type /Catalog /Pages 2 0 R >>', b'', b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>']
    pages = []
    for _ in range(count):
        index = len(objects)+1
        pages.append(index)
        objects.append(f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents {index+1} 0 R >>'.encode())
        objects.append(f'<< /Length {len(content)} >>\nstream\n'.encode()+content+b'endstream')
    objects[1] = f'<< /Type /Pages /Count {count} /Kids [{" ".join(f"{n} 0 R" for n in pages)}] >>'.encode()
    return sample.write_pdf(objects)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/dpdf'))
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--repeat', type=int, default=3)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--geometry-only', action='store_true')
    args = parser.parse_args()
    if not 1 <= args.repeat <= 10:
        parser.error('--repeat must be between 1 and 10')
    binary = str(args.binary.resolve())
    env = dict(os.environ, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()))
    sample = sample_module()
    version = subprocess.check_output([binary, '--version'], env=env, text=True, timeout=20).strip()
    report = {'schema': 1, 'scope': 'generated_headless_reader_engine', 'os': os.name,
              'platform': platform.platform(), 'recorded_at': datetime.now(timezone.utc).isoformat(),
              'binary_version': version, 'binary_sha256': hashlib.sha256(Path(binary).read_bytes()).hexdigest(),
              'pdfium_sha256': hashlib.sha256(args.pdfium.read_bytes()).hexdigest(),
              'timing_is_gui_presentation': False, 'cases': []}
    with tempfile.TemporaryDirectory(prefix='dpdf-reader-benchmark-') as directory:
        folder = Path(directory)

        def run(path, memory=False):
            command = [binary, '--benchmark-reader', str(path), '--compare-eager']
            usage = folder / 'memory.txt'
            measured = memory and os.name == 'posix' and Path('/usr/bin/time').is_file()
            if measured:
                command = ['/usr/bin/time', '-f', '%M', '-o', str(usage), *command]
            digest = hashlib.sha256(path.read_bytes()).digest()
            result = subprocess.run(command, env=env, capture_output=True, text=True, encoding='utf-8', timeout=90)
            assert result.returncode == 0, result.stderr
            data = json.loads(result.stdout)
            assert hashlib.sha256(path.read_bytes()).digest() == digest
            assert data['comparison']['geometry_matches_loaded'], data['comparison']
            assert data['cache_bytes'] <= data['cache_budget_bytes']
            assert data['first_frame_bytes'] <= 3072 * 4096 * 4
            assert all(s['frame_bytes'] <= 3072 * 4096 * 4 for s in data['samples'])
            if measured:
                data['process_peak_kib'] = int(usage.read_text().strip())
            return data

        for inherited in [False, True]:
            for cropped in [False, True]:
                for rotation in [0, 90, 180, 270]:
                    for unit in [1, 2]:
                        path = folder / 'geometry.pdf'
                        path.write_bytes(geometry_fixture(sample, rotation, cropped, inherited, unit))
                        data = run(path)
                        assert data['pages'] == 1
        print('PASS: lazy versus loaded page sizes at all quarter turns, crop/negative origins, inherited boxes and UserUnit 1/2', flush=True)

        if not args.geometry_only:
            for name, content in [('50-office', sample.make_pdf(50)), ('1000-text', sample.make_pdf(1000)), ('50-vector', vector_fixture(sample))]:
                path = folder / (name+'.pdf')
                path.write_bytes(content)
                measurements = [run(path, memory=True) for _ in range(args.repeat)]
                entry = {'name':name, 'pages':measurements[0]['pages'], 'input_bytes':len(content),
                    'runs':measurements, 'median':{key:statistics.median(v[key] for v in measurements)
                        for key in ['first_page_ready_ms', 'metadata_ms', 'first_render_ms', 'cache_bytes']}}
                for key in ['lazy_page_sizes_ms', 'eager_page_sizes_ms']:
                    entry['median'][key] = statistics.median(v['comparison'][key] for v in measurements)
                if all('process_peak_kib' in v for v in measurements):
                    entry['median']['process_peak_kib'] = statistics.median(v['process_peak_kib'] for v in measurements)
                report['cases'].append(entry)
                print(json.dumps({'name':name, **entry['median']}), flush=True)
    if args.output:
        args.output.write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')


if __name__ == '__main__':
    main()
