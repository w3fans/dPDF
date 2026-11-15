#!/usr/bin/env python3
"""Generated Linux/X11 GUI timing upper bounds using visible per-page fiducials.

Includes process/input/capture overhead; not compositor-independent latency or Windows acceptance.
Requires the existing private-display tools, xdotool and ImageMagick import.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import tempfile
import time


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def markers(sample, count):
    objects = [b'<< /Type /Catalog /Pages 2 0 R >>', b'']
    pages = []
    for page in range(count):
        index = len(objects) + 1
        pages.append(index)
        color = '0 1 1' if page == 6 else ('1 0.5 0' if page == count - 1 else '1 0 1')
        stream = f'q {color} rg 40 700 40 40 re f Q\n'.encode()
        objects.extend([
            f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << >> /Contents {index+1} 0 R >>'.encode(),
            f'<< /Length {len(stream)} >>\nstream\n'.encode() + stream + b'endstream'])
    objects[1] = f'<< /Type /Pages /Count {count} /Kids [{" ".join(str(n)+" 0 R" for n in pages)}] >>'.encode()
    return sample.write_pdf(objects)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    parser.add_argument('--xvfb', default='Xvfb')
    parser.add_argument('--scale', type=float, choices=[1, 1.5, 2], default=1)
    parser.add_argument('--repeat', type=int, choices=range(1, 6), default=3)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.name != 'posix' or not Path('/proc/self/status').is_file():
        parser.error('Use Linux and the isolated authenticated X11 display')
    bench = load('gui_benchmark_fixtures', 'benchmark-reader.py')
    private = load('gui_benchmark_display', 'private-display.py')
    sample = bench.sample_module()
    binary, pdfium, qpdf = (path.resolve() for path in (args.binary, args.pdfium, args.qpdf))
    digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    identity = dict(binary_sha256=digest(binary), pdfium_sha256=digest(pdfium), qpdf_sha256=digest(qpdf))
    report = dict(schema=1, scope='generated_private_x11_gui_timing_upper_bounds',
                  recorded_at=datetime.now(timezone.utc).isoformat(), scale=args.scale,
                  includes_input_and_capture_overhead=True, first_phase_includes_fit_page_request=True,
                  input_is_generated_fiducial_overlay=True, windows_qualified=False,
                  complete_command_queue_depth_measured=False, **identity, cases=[])
    with tempfile.TemporaryDirectory(prefix='dpdf-gui-benchmark-') as directory:
        root = Path(directory)
        cases = [('50-office', sample.make_pdf(50), 50), ('300-office', sample.make_pdf(300), 300),
                 ('1000-text', sample.make_pdf(1000), 1000), ('50-vector', bench.vector_fixture(sample), 50),
                 ('50-large-image', bench.image_fixture(sample), 50),
                 ('300-mixed-geometry', bench.mixed_fixture(sample), 300)]
        for name, content, count in cases:
            original = root / (name + '-original.pdf')
            original.write_bytes(content)
            overlay = root / 'markers.pdf'
            overlay.write_bytes(markers(sample, count))
            fixture = root / (name + '.pdf')
            subprocess.run([str(qpdf), str(original), '--deterministic-id', '--overlay', str(overlay), '--', str(fixture)],
                           check=True, capture_output=True, timeout=60)
            fixture_sha = digest(fixture)
            case = dict(name=name, pages=count, input_sha256=fixture_sha, original_input_sha256=digest(original), runs=[])
            report['cases'].append(case)
            for iteration in range(args.repeat):
                folder = root / (name + '-' + str(iteration))
                folder.mkdir()
                with private.display(folder, args.xvfb, screen='2560x1800x24') as env:
                    env.update(DPDF_PDFIUM_LIBRARY=str(pdfium), DPDF_QPDF_BINARY=str(qpdf),
                               SLINT_SCALE_FACTOR=str(args.scale),
                               DBUS_SESSION_BUS_ADDRESS='unix:path=' + str(folder / 'no-session-bus'))
                    with (folder / 'stdout').open('w') as out, (folder / 'stderr').open('w') as err:
                        started = time.monotonic()
                        window = None
                        app = subprocess.Popen([str(binary), str(fixture)], env=env, stdout=out, stderr=err)
                        try:
                            deadline = started + 15
                            while time.monotonic() < deadline:
                                found = subprocess.run(['xdotool', 'search', '--onlyvisible', '--pid', str(app.pid)],
                                                       env=env, capture_output=True, text=True, timeout=2)
                                if not found.returncode:
                                    window = found.stdout.splitlines()[-1]
                                    break
                                if app.poll() is not None:
                                    raise RuntimeError('Reader exited before showing its generated window')
                                time.sleep(0.02)
                            else:
                                raise RuntimeError('Reader window did not appear')
                            def x(*items):
                                subprocess.run(['xdotool', *items], env=env, check=True, capture_output=True, timeout=3)
                            def click(px, py):
                                x('mousemove', '--window', window, str(round(px * args.scale)),
                                  str(round(py * args.scale)), 'click', '1')
                            x('windowfocus', window)
                            def fit():
                                click(347, 62)
                            width, height = round(978 * args.scale), round(650 * args.scale)
                            crop = f'{width}x{height}+{round(200*args.scale)}+{round(118*args.scale)}'
                            captures = []
                            def visible(color, min_width=0):
                                deadline = time.monotonic() + 15
                                while time.monotonic() < deadline:
                                    capture_start = time.monotonic()
                                    frame = subprocess.check_output(['import', '-window', window, '-crop', crop,
                                                                     '-depth', '8', 'RGB:-'], env=env, timeout=5)
                                    captures.append((time.monotonic() - capture_start) * 1000)
                                    if len(frame) != width * height * 3:
                                        raise RuntimeError('Unexpected private-window capture dimensions')
                                    positions = [match.start() // 3 for match in re.finditer(re.escape(color), frame)
                                                 if match.start() % 3 == 0]
                                    if len(positions) >= 256:
                                        extent = max(p % width for p in positions) - min(p % width for p in positions) + 1
                                        if extent > min_width:
                                            return extent
                                    if app.poll() is not None:
                                        raise RuntimeError('Reader exited before drawing requested fiducial')
                                    time.sleep(0.02)
                                raise RuntimeError('Requested page/zoom fiducial was not visibly drawn')
                            fit()
                            visible(b'\xff\x00\xff')
                            initial_ms = (time.monotonic() - started) * 1000
                            def page(number):
                                click(705, 22)
                                x('key', 'ctrl+a')
                                x('type', '--clearmodifiers', str(number))
                                x('key', 'Return')
                            began = time.monotonic()
                            page(7)
                            old_width = visible(b'\x00\xff\xff')
                            page_ms = (time.monotonic() - began) * 1000
                            began = time.monotonic()
                            x('key', 'ctrl+plus')
                            visible(b'\x00\xff\xff', min_width=old_width + 2)
                            zoom_ms = (time.monotonic() - began) * 1000
                            began = time.monotonic()
                            page(count)
                            fit()
                            visible(b'\xff\x80\x00')
                            last_ms = (time.monotonic() - began) * 1000
                            status = Path(f'/proc/{app.pid}/status').read_text()
                            rss = int(re.search(r'^VmRSS:\s+(\d+) kB$', status, re.M)[1])
                            case['runs'].append(dict(launch_fit_page_ready_ms=initial_ms,
                                                      page7_ready_ms=page_ms, zoom_ready_ms=zoom_ms,
                                                      last_page_fit_ready_ms=last_ms, final_rss_kib=rss,
                                                      process_peak_rss_kib=int(re.search(r'^VmHWM:\s+(\d+) kB$', status, re.M)[1]),
                                                      capture_ms=captures))
                        except Exception as error:
                            report['status'] = 'FAIL'
                            case['error'] = str(error)
                            failure = args.output.with_name(args.output.stem + '-' + name + '-failed.png')
                            case['failure_capture'] = None
                            if window is not None:
                                try:
                                    capture = subprocess.run(['import', '-window', window, str(failure)],
                                                             env=env, capture_output=True, timeout=5)
                                    if capture.returncode == 0:
                                        case['failure_capture'] = str(failure)
                                except (OSError, subprocess.SubprocessError) as capture_error:
                                    case['failure_capture_error'] = str(capture_error)
                            case['stderr_tail'] = (folder / 'stderr').read_text(errors='replace')[-4000:]
                            args.output.write_text(json.dumps(report, indent=2) + '\n')
                            raise
                        finally:
                            try:
                                if app.poll() is None:
                                    if window is None:
                                        app.terminate()
                                    else:
                                        subprocess.run(['xdotool', 'windowclose', window], env=env, check=True, capture_output=True, timeout=3)
                                app.wait(timeout=5)
                            except (subprocess.SubprocessError, UnboundLocalError):
                                app.terminate()
                                try:
                                    app.wait(timeout=5)
                                except subprocess.TimeoutExpired:
                                    app.kill()
                                    app.wait()
                if digest(fixture) != fixture_sha or original.read_bytes() != content:
                    raise RuntimeError('GUI benchmark changed a generated input')
                args.output.write_text(json.dumps(report, indent=2) + '\n')
            keys = ('launch_fit_page_ready_ms', 'page7_ready_ms', 'zoom_ready_ms', 'last_page_fit_ready_ms', 'final_rss_kib', 'process_peak_rss_kib')
            case['median'] = {key: statistics.median(row[key] for row in case['runs']) for key in keys}
            print(json.dumps(dict(name=name, median=case['median'])), flush=True)
    if identity != dict(binary_sha256=digest(binary), pdfium_sha256=digest(pdfium), qpdf_sha256=digest(qpdf)):
        raise RuntimeError('Benchmark runtime changed during measurement')
    report['status'] = 'MEASURED'
    args.output.write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
