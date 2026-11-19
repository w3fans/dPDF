#!/usr/bin/env python3
"""Interleaved same-machine ordinary-path measurements; flags need investigation, not acceptance."""
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


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--repeat', type=int, default=5)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--case', action='append', choices=['50-office', '300-office', '1000-text', '50-vector', '50-large-image', '300-mixed-geometry'])
    args = parser.parse_args()
    if not 3 <= args.repeat <= 10:
        parser.error('--repeat must be between 3 and 10')
    spec = importlib.util.spec_from_file_location('comparison_fixtures', Path(__file__).with_name('benchmark-reader.py'))
    fixtures = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(fixtures)
    sample = fixtures.sample_module()
    paths = {name: path.resolve() for name, path in [('baseline', args.baseline), ('candidate', args.candidate)]}
    identity = {name: digest(path) for name, path in paths.items()}
    env = dict(os.environ, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()))
    report = dict(schema=1, scope='interleaved_generated_headless_ordinary_path',
                  recorded_at=datetime.now(timezone.utc).isoformat(), platform=platform.platform(),
                  binary_sha256=identity, pdfium_sha256=digest(args.pdfium),
                  gui_latency=False, full_command_queue_depth=False, threshold_percent=10,
                  same_executable=identity['baseline'] == identity['candidate'],
                  requested_cases=args.case, full_generated_corpus=args.case is None, cases=[])
    with tempfile.TemporaryDirectory(prefix='dpdf-reader-comparison-') as directory:
        folder = Path(directory)
        cases = [('50-office', sample.make_pdf(50)), ('300-office', sample.make_pdf(300)),
                 ('1000-text', sample.make_pdf(1000)), ('50-vector', fixtures.vector_fixture(sample)),
                 ('50-large-image', fixtures.image_fixture(sample)),
                 ('300-mixed-geometry', fixtures.mixed_fixture(sample))]
        for name, content in cases:
            if args.case and name not in args.case:
                continue
            fixture = folder / (name + '.pdf')
            fixture.write_bytes(content)
            runs = {'baseline': [], 'candidate': []}
            for iteration in range(args.repeat):
                order = ['baseline', 'candidate'] if iteration % 2 == 0 else ['candidate', 'baseline']
                for role in order:
                    command = [str(paths[role]), '--benchmark-reader', str(fixture), '--compare-eager']
                    usage = folder / 'usage.txt'
                    measured = platform.system() == 'Linux' and Path('/usr/bin/time').is_file()
                    if measured:
                        command = ['/usr/bin/time', '-f', '%M', '-o', str(usage), *command]
                    result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=90)
                    if result.returncode:
                        raise RuntimeError(f'{name}/{role}: {result.stderr}')
                    data = json.loads(result.stdout)
                    assert data['comparison']['geometry_matches_loaded']
                    assert data['cache_bytes'] <= data['cache_budget_bytes']
                    if measured:
                        data['process_peak_kib'] = int(usage.read_text().strip())
                    runs[role].append(data)
                assert fixture.read_bytes() == content, 'Comparison changed its source PDF'
            def metrics(data):
                values = {'first_page_ready_ms': statistics.median(x['first_page_ready_ms'] for x in data),
                          'cache_bytes': statistics.median(x['cache_bytes'] for x in data)}
                for sample_name in ('page_7', 'last_page', 'zoom'):
                    values[sample_name + '_ms'] = statistics.median(
                        next(s['render_ms'] for s in x['samples'] if s['name'] == sample_name) for x in data)
                if all('process_peak_kib' in x for x in data):
                    values['process_peak_kib'] = statistics.median(x['process_peak_kib'] for x in data)
                return values
            medians = {role: metrics(data) for role, data in runs.items()}
            delta = {key: (medians['candidate'][key] / value - 1) * 100
                     for key, value in medians['baseline'].items() if value > 0}
            flagged = [key for key, value in delta.items() if value > 10]
            report['cases'].append(dict(name=name, input_sha256=digest(fixture), runs=runs,
                                        median=medians, change_percent=delta, investigate=flagged))
            args.output.write_text(json.dumps(report, indent=2) + '\n')
            print(json.dumps(dict(case=name, change_percent=delta, investigate=flagged)), flush=True)
    assert all(digest(paths[role]) == sha for role, sha in identity.items()), 'Executable changed during comparison'
    report['status'] = 'INVESTIGATE' if any(case['investigate'] for case in report['cases']) else 'WITHIN_PROPOSED_GUARD'
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(report['status'])


if __name__ == '__main__':
    main()
