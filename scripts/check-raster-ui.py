#!/usr/bin/env python3
"""Generated high-zoom window checks; bounded slots/bitmaps, pan, rotations and tabs."""
import argparse
import os
import hashlib
import json
import time
from pathlib import Path
import re
import subprocess
import tempfile

# Shared fixture/display loaders do not start a display at import time.
import importlib.util


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--xvfb', default='Xvfb')
    parser.add_argument('--report', type=Path)
    parser.add_argument('--scale', type=float, choices=[1, 1.5, 2], default=1)
    args = parser.parse_args()
    bench = module('raster_fixtures', 'benchmark-reader.py')
    private = module('raster_display', 'private-display.py')
    with tempfile.TemporaryDirectory(prefix='dpdf-raster-ui-') as directory:
        folder = Path(directory)
        fixture = folder / 'Generated raster fixture.pdf'
        original = bench.vector_fixture(bench.sample_module(), 50)
        fixture.write_bytes(original)
        with private.display(folder, args.xvfb, screen='2560x1800x24') as env:
            env.update(DPDF_SMOKE_RASTER_TILES='1', SLINT_SCALE_FACTOR=str(args.scale),
                       DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()))
            if os.name != 'nt':
                env['DBUS_SESSION_BUS_ADDRESS'] = 'unix:path=' + str(folder / 'no-session-bus')
            try:
                command = [str(args.binary.resolve()), '--smoke-ui', str(fixture)]
                started = time.monotonic()
                rss = []
                with subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                      text=True, encoding='utf-8', errors='replace') as child:
                    while True:
                        status = Path(f'/proc/{child.pid}/status')
                        if os.name == 'posix' and status.is_file():
                            try:
                                match = re.search(r'^VmRSS:\s+(\d+) kB$', status.read_text(), re.M)
                                if match:
                                    rss.append(dict(seconds=time.monotonic() - started, kib=int(match[1])))
                            except FileNotFoundError:
                                pass  # Exited between status lookup and read.
                        remaining = 60 - (time.monotonic() - started)
                        try:
                            stdout, stderr = child.communicate(timeout=min(0.05, max(0.001, remaining)))
                            break
                        except subprocess.TimeoutExpired:
                            if remaining <= 0:
                                child.kill()
                                stdout, stderr = child.communicate()
                                raise subprocess.TimeoutExpired(command, 60, stdout, stderr)
                    result = subprocess.CompletedProcess(command, child.returncode, stdout, stderr)
                observations = dict(schema=1, scope='generated_native_raster_window', scale=args.scale,
                                    binary_sha256=hashlib.sha256(args.binary.read_bytes()).hexdigest(),
                                    pdfium_sha256=hashlib.sha256(args.pdfium.read_bytes()).hexdigest(),
                                    input_sha256=hashlib.sha256(original).hexdigest(),
                                    timing_origin='first diagnostic tick for preview; view mutation for detail/pan snapshots',
                                    rss_samples=rss, rss_sampling_seconds=0.05,
                                    peak_rss_kib=max((item['kib'] for item in rss), default=None),
                                    stdout=stdout, stderr=stderr, exit_code=child.returncode)
                if args.report:
                    args.report.write_text(json.dumps(observations, indent=2) + '\n')
                print(result.stdout, end='')
                if result.returncode:
                    print(result.stderr, end='')
                if rss:
                    print(f"raster: sampled_peak_rss_kib={observations['peak_rss_kib']}")
                    if observations['peak_rss_kib'] > 512 * 1024:
                        raise RuntimeError('Generated window exceeded proposed 512 MiB RSS guard; investigate report')
            except subprocess.TimeoutExpired as error:
                for output in (error.stdout, error.stderr):
                    if output:
                        print(output.decode('utf-8', errors='replace') if isinstance(output, bytes) else output)
                raise
            result.check_returncode()
            summary = re.search(r'raster: summary peak_render_slots=(\d+) peak_detail_bytes=(\d+) cases=12 tab_switch_close=true', result.stdout)
            if result.stdout.count('raster: case=') != 12 or not summary:
                raise RuntimeError('Raster window diagnostic did not complete all twelve cases and tab lifecycle')
            slots, size = map(int, summary.groups())
            if not 0 < slots <= 8 or not 0 < size <= 32 * 1024 ** 2:
                raise RuntimeError('Observed raster metrics exceeded their budgets or were empty')
            if fixture.read_bytes() != original:
                raise RuntimeError('Raster diagnostic changed its source PDF')
        print(f'PASS: generated high-zoom window diagnostics, scale={args.scale}; render slots include queued, active and unconsumed replies')


if __name__ == '__main__':
    main()
