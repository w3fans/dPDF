#!/usr/bin/env python3
"""Native Windows print-dialog / Microsoft Print to PDF checks; never physical jobs."""
import argparse
import ctypes
from ctypes import wintypes
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
from print_benchmark import Benchmark


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


def owned_by_fixture(window, fixture_pid, window_pid, owner):
    """Follow only a bounded owner chain; no title/foreground/Explorer guesses."""
    seen = set()
    for _ in range(16):
        if not window or window in seen:
            return False
        seen.add(window)
        if window_pid(window) == fixture_pid:
            return True
        window = owner(window)
    return False


def dialog_action(process, cancel):
    """Address a dialog proven to belong to the exact fixture or its owner chain.

    WM_COMMAND invokes its standard OK/Cancel button without keyboard injection
    into another application, clipboard changes, or a dependency on English labels.
    Native mouse/keyboard and physical printer qualification remain separate.
    """
    user = ctypes.WinDLL('user32', use_last_error=True)
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
    user.EnumWindows.restype = wintypes.BOOL
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user.GetWindow.argtypes = [wintypes.HWND, wintypes.UINT]
    user.GetWindow.restype = wintypes.HWND
    user.IsWindowVisible.argtypes = [wintypes.HWND]
    user.GetDlgItem.argtypes = [wintypes.HWND, ctypes.c_int]
    user.GetDlgItem.restype = wintypes.HWND
    user.GetClassNameW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
    button_id = 2 if cancel else 1
    deadline = time.monotonic() + 25
    last_diagnostic = ''

    def window_pid(window):
        pid = wintypes.DWORD()
        user.GetWindowThreadProcessId(window, ctypes.byref(pid))
        return pid.value

    def belongs(window):
        return owned_by_fixture(window, process.pid, window_pid, lambda hwnd: user.GetWindow(hwnd, 4))
    while time.monotonic() < deadline and process.poll() is None:
        matches = []

        @callback_type
        def visit(window, _):
            if belongs(window) and user.IsWindowVisible(window):
                name = ctypes.create_unicode_buffer(128)
                user.GetClassNameW(window, name, len(name))
                button = user.GetDlgItem(window, button_id)
                if name.value == '#32770' and button:
                    matches.append((window, button))
                else:
                    matches.append((window, None))
            return True

        user.EnumWindows(visit, 0)
        if len(matches) != 1:
            last_diagnostic = f'Fixture-owned visible candidates: {len(matches)}. '
        if len(matches) == 1:
            window, button = matches[0]
            time.sleep(0.3)
            if not belongs(window) or process.poll() is not None:
                continue
            if button:
                if not user.PostMessageW(window, 0x0111, button_id, button):
                    raise ctypes.WinError(ctypes.get_last_error())
                return
            result = subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-MTA',
                '-ExecutionPolicy', 'Bypass', '-File', str(Path(__file__).with_name('windows-print-dialog.ps1')),
                '-WindowHandle', str(window), '-FixtureProcessId', str(process.pid),
                '-Action', 'cancel' if cancel else 'print'], capture_output=True, text=True,
                encoding='utf-8', errors='replace', timeout=15)
            if result.returncode == 0:
                return
            last_diagnostic = (result.stderr + result.stdout)[-4000:]
            time.sleep(.3)
        time.sleep(0.05)
    stdout, stderr = process.communicate(timeout=5) if process.poll() is not None else ('', '')
    raise AssertionError('No unambiguous native fixture print dialog: ' + last_diagnostic + stderr + stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/dpdf.exe'))
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    parser.add_argument('--allow-native-dialog', action='store_true', help='Allow fixture print windows on the current Windows desktop')
    parser.add_argument('--stress', action='store_true', help='Also print 50 generated pages and page 7')
    parser.add_argument('--benchmark-report', type=Path, help='Save job sizes/timings as JSON; implies --stress')
    args = parser.parse_args()
    if os.name != 'nt':
        parser.error('This suite requires native Windows; Linux type-checks do not run a Windows printer')
    if not args.allow_native_dialog:
        parser.error('Pass --allow-native-dialog to open fixture dialogs; the child permits only Microsoft Print to PDF')
    binary, qpdf = str(args.binary.resolve()), str(args.qpdf.resolve())
    sample = module('sample', 'make-sample.py')
    display = module('display', 'private-display.py')
    with tempfile.TemporaryDirectory(prefix='dpdf-windows-print-') as directory:
        folder = Path(directory)
        source = folder / 'Native print Žig.pdf'
        source.write_bytes(sample.make_pdf(3, print_marker=True))
        digest = hashlib.sha256(source.read_bytes()).digest()
        env = display.fixture_environment(folder)
        env.update(DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()), DPDF_QPDF_BINARY=qpdf, DPDF_TEST_NATIVE_PRINT='1')

        def run(command):
            result = subprocess.run(command, env=env, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=30)
            assert result.returncode == 0, result.stderr
            return result

        def command(output, pages='', fit='fit', input_pdf=None):
            # Current is zero-based, so '2' below tests the Current Page radio
            # button as well as the same one-page range accepted by other backends.
            return [binary, '--test-native-print', str(input_pdf or source), str(output), '1', pages, fit]

        def native(output, pages='', fit='fit', cancel=False, input_pdf=None):
            process = subprocess.Popen(command(output, pages, fit, input_pdf), env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8", errors="replace")
            try:
                dialog_action(process, cancel)
                stdout, stderr = process.communicate(timeout=300 if args.stress or args.benchmark_report else 90)
                assert process.returncode == 0, stderr
                return json.loads(stdout)
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=10)

        def check_pdf(output, expected, result):
            assert result['status'] == 'submitted' and result['pages_drawn'] == expected, result
            assert result['printer'].lower() == 'microsoft print to pdf', result
            # Spool completion is distinct from submission. Wait for a complete,
            # valid PDF before asserting its page count or reading its pixels.
            deadline = time.monotonic() + 60
            while time.monotonic() < deadline:
                if output.is_file():
                    check = subprocess.run([qpdf, '--check', str(output)], env=env, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=10)
                    if check.returncode == 0:
                        break
                time.sleep(0.1)
            else:
                raise AssertionError('Print spooler did not finish a valid PDF')
            assert int(run([qpdf, '--show-npages', str(output)]).stdout) == len(expected)
            assert hashlib.sha256(source.read_bytes()).digest() == digest
            for metric in result['metrics']:
                assert 0 < metric['raster_width'] <= 4096 and 0 < metric['raster_height'] <= 8192
                assert metric['raster_width'] * metric['raster_height'] <= 16_000_000

        def marker(output, index, channel):
            ppm = folder / 'frame.ppm'
            run([binary, '--render-page', str(output), str(index), str(ppm)])
            _, size, _, pixels = ppm.read_bytes().split(b'\n', 3)
            width, height = map(int, size.split())
            xs, ys = [], []
            for y in range(height):
                for x in range(width):
                    rgb = pixels[(y * width + x) * 3:(y * width + x) * 3 + 3]
                    if rgb[channel] > 150 and all(rgb[c] < 70 for c in range(3) if c != channel):
                        xs.append(x); ys.append(y)
            assert xs, 'Printed page color/order or marker disappeared'
            return (min(xs), min(ys), max(xs) - min(xs) + 1, max(ys) - min(ys) + 1, width, height)

        for selection, expected in [('', [0, 1, 2]), ('2', [1]), ('1,3', [0, 2]), ('1-2,2-3', [0, 1, 2])]:
            output = folder / ('pages-' + (selection or 'all') + '.pdf')
            result = native(output, selection)
            check_pdf(output, expected, result)
            for i, source_index in enumerate(expected, 1):
                marker(output, i, source_index % 3)
        print('PASS: native Windows all/current/disjoint/overlapping pages, page order/color and source preservation', flush=True)

        reference_bounds = {}
        for fit in ['fit', 'actual']:
            output = folder / f'{fit}.pdf'
            result = native(output, '1', fit)
            check_pdf(output, [0], result)
            x, y, w, h, image_w, image_h = marker(output, 1, 0)
            reference_bounds[fit] = (x,y,w,h,image_w,image_h)
            m = result['metrics'][0]
            expected = [
                (m['offset_x'] + m['x'] + 72 / 612 * m['width']) * image_w / m['physical_width'],
                (m['offset_y'] + m['y'] + (792 - 108) / 792 * m['height']) * image_h / m['physical_height'],
                72 / 612 * m['width'] * image_w / m['physical_width'],
                36 / 792 * m['height'] * image_h / m['physical_height'],
            ]
            assert all(abs(a - b) <= 3 for a, b in zip([x, y, w, h], expected)), (fit, expected, [x,y,w,h], m)
        print('PASS: native printed fit/actual-size physical geometry, upright BGR colors and bounded raster', flush=True)

        for unit in (2, 0.5):
            scaled = folder / f'physical-unit-{unit}.pdf'
            scaled.write_bytes(sample.make_physical_unit_pdf(unit))
            scaled_digest = hashlib.sha256(scaled.read_bytes()).digest()
            for fit in ('fit', 'actual'):
                output = folder / f'physical-unit-{unit}-{fit}.pdf'
                result = native(output, '1', fit, input_pdf=scaled)
                check_pdf(output, [0], result)
                bounds = marker(output, 1, 0)
                expected = reference_bounds[fit]
                assert bounds[4:] == expected[4:] and all(abs(a-b) <= 3 for a,b in zip(bounds[:4],expected[:4])), ('Physical-unit marker mismatch',unit,fit,bounds,expected)
                assert hashlib.sha256(scaled.read_bytes()).digest() == scaled_digest
        print('PASS: UserUnit half/double preserve native fit/actual-size physical marker geometry', flush=True)

        fixtures = module('mark_pages', 'check-pages.py')
        blue = folder / 'physical-blue-mark.png'
        blue.write_bytes(fixtures.png(32,16, b''.join(b'\0'+bytes((20,40,180,255))*32 for _ in range(16))))
        recipe = folder / 'physical-mark.json'
        recipe.write_text(json.dumps([dict(page=1, image=blue.name, geometry=dict(x_mm=25,y_mm=40,width_mm=40,height_mm=20))]))
        mark_bounds = {}
        for unit in (1, 2, .5):
            original_page = folder / f'mark-source-unit-{unit}.pdf'
            original_page.write_bytes(sample.make_physical_unit_pdf(unit, count=3))
            original_hash = hashlib.sha256(original_page.read_bytes()).digest()
            marked = folder / f'marked-unit-{unit}.pdf'
            run([binary, '--overlay', str(original_page), str(marked), str(recipe)])
            marked_hash = hashlib.sha256(marked.read_bytes()).digest()
            for scaling in ('fit', 'actual'):
                output = folder / f'mark-unit-{unit}-{scaling}.pdf'
                result = native(output, '1', scaling, input_pdf=marked)
                check_pdf(output, [0], result)
                bounds = marker(output, 1, 2)
                if unit == 1:
                    m = result['metrics'][0]
                    w,h = bounds[4:]
                    expected = [(m['offset_x']+m['x']+25*72/25.4/612*m['width'])*w/m['physical_width'],
                                (m['offset_y']+m['y']+40*72/25.4/792*m['height'])*h/m['physical_height'],
                                40*72/25.4/612*m['width']*w/m['physical_width'],20*72/25.4/792*m['height']*h/m['physical_height']]
                    assert all(abs(a-b)<=3 for a,b in zip(bounds[:4],expected)), (scaling,bounds,expected)
                    mark_bounds[scaling] = bounds
                else:
                    expected = mark_bounds[scaling]
                    assert bounds[4:] == expected[4:] and all(abs(a-b)<=3 for a,b in zip(bounds[:4],expected[:4])), ('Printed mark physical size mismatch',unit,scaling,bounds,expected)
            assert hashlib.sha256(original_page.read_bytes()).digest() == original_hash
            assert hashlib.sha256(marked.read_bytes()).digest() == marked_hash
        print('PASS: exported 40x20-mm mark retains native fit/actual-size print geometry on units 1/2/0.5', flush=True)

        output = folder / 'cancelled.pdf'
        result = native(output, '2', cancel=True)
        assert result['status'] == 'cancelled' and not output.exists(), result
        alias = folder / 'source alias.pdf'
        os.link(source, alias)
        rejected = subprocess.run(command(alias), env=env, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=15)
        assert rejected.returncode != 0 and hashlib.sha256(source.read_bytes()).digest() == digest
        no_opt_in = dict(env); no_opt_in.pop('DPDF_TEST_NATIVE_PRINT')
        rejected = subprocess.run(command(folder / 'refused.pdf'), env=no_opt_in, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=15)
        assert rejected.returncode != 0 and not (folder / 'refused.pdf').exists()
        print('PASS: native cancellation, alias refusal before dialog and diagnostic opt-in enforcement', flush=True)

        if args.stress or args.benchmark_report:
            benchmark = Benchmark('microsoft-print-to-pdf', binary)
            long_source = folder / '50-page office fixture.pdf'
            long_source.write_bytes(sample.make_pdf(50, print_marker=True))
            for name, selection, expected in [('all-50-fit', '', list(range(50))),
                                               ('page-7-fit', '7', [6])]:
                output = folder / (name + '.pdf')
                benchmark.measure(name, long_source, output, expected, 'fit',
                                  lambda: native(output, selection, input_pdf=long_source),
                                  lambda result: check_pdf(output, expected, result))
            if args.benchmark_report:
                benchmark.write(args.benchmark_report)


if __name__ == '__main__':
    main()
