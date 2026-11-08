#!/usr/bin/env python3
"""Compare reader buffer memory using generated padding, not an office-performance claim."""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/dpdf'))
    parser.add_argument('--baseline-binary', type=Path, required=True)
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--repeat', type=int, choices=range(1, 6), default=3)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if platform.system() != 'Linux' or not Path('/usr/bin/time').is_file():
        parser.error('This RSS diagnostic requires Linux and the standard /usr/bin/time tool')
    spec = importlib.util.spec_from_file_location('sample', Path(__file__).with_name('make-sample.py'))
    sample = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(sample)
    env = dict(os.environ, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()))
    report = dict(schema=1, scope='generated_64_MiB_padding_reader_buffer_probe',
                  representative_office_document=False, timing_is_gui_presentation=False,
                  recorded_at=datetime.now(timezone.utc).isoformat(), platform=platform.platform(),
                  pdfium_sha256=hashlib.sha256(args.pdfium.read_bytes()).hexdigest())
    binaries = {'baseline': args.baseline_binary.resolve(), 'candidate': args.binary.resolve()}
    for name, binary in binaries.items():
        report[name] = dict(binary_version=subprocess.check_output([str(binary), '--version'], env=env,
                              text=True, timeout=20).strip(), binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), runs=[])
    with tempfile.TemporaryDirectory(prefix='dpdf-reader-buffer-') as directory:
        root = Path(directory)
        fixture = root / 'generated-buffer-probe.pdf'
        content = b'BT /F1 16 Tf 40 240 Td (Generated reader buffer memory probe) Tj ET\n'
        fixture.write_bytes(sample.write_pdf([
            b'<< /Type /Catalog /Pages 2 0 R /DPDFMemoryProbe 6 0 R >>',
            b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
            b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 300] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>',
            b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
            f'<< /Length {len(content)} >>\nstream\n'.encode() + content + b'endstream',
            f'<< /Length {64*1024*1024} >>\nstream\n'.encode() + b'X'*(64*1024*1024) + b'\nendstream']))
        report['input_bytes'] = fixture.stat().st_size
        report['input_sha256'] = digest = hashlib.sha256(fixture.read_bytes()).hexdigest()
        # Interleave fresh processes to reduce ordering effects from warm OS caches.
        for _ in range(args.repeat):
            for name, binary in binaries.items():
                usage = root / 'usage.txt'
                result = subprocess.run(['/usr/bin/time', '-f', '%M', '-o', str(usage), str(binary),
                                         '--benchmark-reader', str(fixture)], env=env, check=True,
                                        capture_output=True, text=True, timeout=60)
                data = json.loads(result.stdout)
                assert data['pages'] == 1 and data['input_bytes'] == report['input_bytes']
                assert data['cache_bytes'] <= data['cache_budget_bytes']
                assert hashlib.sha256(fixture.read_bytes()).hexdigest() == digest
                metrics = {key: data[key] for key in ('read_ms', 'load_ms', 'metadata_ms', 'first_page_ready_ms')}
                metrics['process_peak_kib'] = int(usage.read_text().strip())
                report[name]['runs'].append(metrics)
    for name in binaries:
        runs = report[name]['runs']
        report[name]['median'] = {key: statistics.median(run[key] for run in runs) for key in runs[0]}
    before, after = [report[name]['median']['process_peak_kib'] for name in binaries]
    report['median_peak_reduction_kib'] = before - after
    report['median_peak_reduction_percent'] = (before - after) * 100 / before
    args.output.write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
    print(json.dumps({name: report[name]['median'] for name in binaries}))


if __name__ == '__main__':
    main()
