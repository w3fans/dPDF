#!/usr/bin/env python3
"""Verify Fedora's real GTK printing to isolated PDF files; no physical queues."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time
from print_benchmark import Benchmark


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/dpdf'))
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    parser.add_argument('--xvfb', default='Xvfb')
    parser.add_argument('--stress', action='store_true', help='Also print a generated 50-page document and page 7')
    parser.add_argument('--benchmark-report', type=Path, help='Save job sizes/timings as JSON; implies --stress')
    args = parser.parse_args()
    args.stress = args.stress or args.benchmark_report is not None
    binary, qpdf = str(args.binary.resolve()), str(args.qpdf.resolve())
    sample, server = module('sample', 'make-sample.py'), module('display', 'private-display.py')
    with tempfile.TemporaryDirectory(prefix='dpdf-native-print-') as directory:
        folder = Path(directory)
        source = folder / 'Native print Žig.pdf'
        source.write_bytes(sample.make_pdf(3, print_marker=True))
        original = hashlib.sha256(source.read_bytes()).digest()
        with server.display(folder, args.xvfb) as env:
            env.update(DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()), DPDF_QPDF_BINARY=qpdf, GTK_PRINT_BACKENDS='file', GTK_USE_PORTAL='0', LC_ALL='C', GSETTINGS_BACKEND='memory', SLINT_BACKEND='winit-software', DPDF_CONFIG_DIR=str(folder / 'config'), XDG_CONFIG_HOME=str(folder / 'xdg-config'), XDG_DATA_HOME=str(folder / 'xdg-data'), XDG_CACHE_HOME=str(folder / 'cache'))

            def run(command, timeout=45):
                result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=timeout)
                if result.returncode:
                    raise AssertionError(result.stderr)
                return result

            def command(output, pages='', fit='fit', input_pdf=None):
                return [binary, '--test-native-print', str(input_pdf or source), str(output), '1', pages, fit]

            def native(output, pages='', fit='fit', cancel=False, input_pdf=None):
                process = subprocess.Popen(command(output, pages, fit, input_pdf), env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
                try:
                    deadline = time.monotonic()+15
                    window = None
                    while time.monotonic() < deadline and process.poll() is None:
                        found = subprocess.run(['xdotool', 'search', '--onlyvisible', '--name', 'Print'], env=env, capture_output=True, text=True)
                        if found.returncode == 0:
                            window = found.stdout.splitlines()[-1]
                            break
                        time.sleep(0.05)
                    if not window:
                        process.terminate()
                        stdout, stderr = process.communicate(timeout=5)
                        raise AssertionError('Native print dialog did not appear: ' + stderr + stdout)
                    run(['xdotool', 'windowfocus', window])
                    time.sleep(0.3)
                    if os.environ.get('DPDF_NATIVE_PRINT_SNAPSHOT') and not cancel:
                        subprocess.run([sys.executable,'-c','from PIL import ImageGrab; import sys; ImageGrab.grab(xdisplay=sys.argv[1]).save(sys.argv[2])',env['DISPLAY'],os.environ['DPDF_NATIVE_PRINT_SNAPSHOT']],env=env,check=True,timeout=10)
                    run(['xdotool', 'key', 'Escape' if cancel else 'Return'])
                    stdout, stderr = process.communicate(timeout=120 if args.stress else 30)
                    assert process.returncode == 0, stderr
                    return json.loads(stdout)
                finally:
                    if process.poll() is None:
                        process.terminate(); process.wait(timeout=5)

            def check_output(output, drawn, result):
                assert result['status'] == 'submitted' and result['pages_drawn'] == drawn, result
                run([qpdf, '--check', str(output)])
                assert int(run([qpdf, '--show-npages', str(output)]).stdout) == len(drawn)
                assert hashlib.sha256(source.read_bytes()).digest() == original

            for selection, expected in (('', [0, 1, 2]), ('2', [1]), ('1,3', [0, 2])):
                output = folder / ('pages-' + (selection or 'all') + '.pdf')
                result = native(output, selection)
                check_output(output, expected, result)
            print('PASS: native GTK all/current/disjoint page output counts and source preservation', flush=True)

            def marker_bounds(path, page=1, channel=0):
                ppm = folder / 'frame.ppm'
                run([binary, '--render-page', str(path), str(page), str(ppm)])
                _, size, _, pixels = ppm.read_bytes().split(b'\n', 3)
                w, h = map(int, size.split())
                xs, ys = [], []
                for y in range(h):
                    for x in range(w):
                        offset = (y*w+x)*3
                        rgb = pixels[offset:offset+3]
                        if rgb[channel] > 150 and all(rgb[c] < 70 for c in range(3) if c != channel):
                            xs.append(x); ys.append(y)
                assert xs, 'Printed marker disappeared'
                return min(xs), min(ys), max(xs)+1, max(ys)+1, w, h

            for selection, channels in (('all', [0,1,2]), ('2', [1]), ('1,3', [0,2])):
                for page,channel in enumerate(channels,1):
                    marker_bounds(folder / f'pages-{selection}.pdf',page,channel)
            print('PASS: every output page retains the selected source page color and order',flush=True)

            reference_bounds = {}
            for scaling in ('fit', 'actual'):
                output = folder / f'scaling-{scaling}.pdf'
                result = native(output, '1', scaling)
                check_output(output, [0], result)
                x0, y0, x1, y1, w, h = marker_bounds(output)
                metric = result['metrics'][0]
                pw, ph = metric['source_width'], metric['source_height']
                dw, dh = metric['draw_width'], metric['draw_height']
                cw, ch = metric['printable_width'], metric['printable_height']
                paper_w, paper_h = cw+72, ch+72  # Diagnostic page setup uses 36pt margins.
                expected = [(36+(cw-dw)/2+72*dw/pw)*w/paper_w,
                            (36+(ch-dh)/2+(ph-108)*dh/ph)*h/paper_h,
                            72*dw/pw*w/paper_w, 36*dh/ph*h/paper_h]
                actual = [x0, y0, x1-x0, y1-y0]
                reference_bounds[scaling] = (x0, y0, x1, y1, w, h)
                assert all(abs(a-b) <= 3 for a, b in zip(actual, expected)), (scaling, actual, expected, result)
                assert metric['raster_width'] <= 4096 and metric['raster_height'] <= 8192
            print('PASS: real printed color, fit/actual-size physical geometry and bounded print resolution', flush=True)

            for unit in (2, 0.5):
                scaled = folder / f'physical-unit-{unit}.pdf'
                scaled.write_bytes(sample.make_physical_unit_pdf(unit))
                digest = hashlib.sha256(scaled.read_bytes()).digest()
                for scaling in ('fit', 'actual'):
                    output = folder / f'physical-unit-{unit}-{scaling}.pdf'
                    result = native(output, '1', scaling, input_pdf=scaled)
                    check_output(output, [0], result)
                    if scaling == 'actual':
                        metric = result['metrics'][0]
                        assert abs(metric['source_width']-612) < .01 and abs(metric['source_height']-792) < .01, ('Incorrect physical page dimensions', unit, metric)
                    bounds = marker_bounds(output)
                    expected = reference_bounds[scaling]
                    assert bounds[4:] == expected[4:] and all(abs(a-b) <= 3 for a,b in zip(bounds[:4],expected[:4])), ('Physical-unit marker mismatch', unit, scaling, bounds, expected)
                    assert hashlib.sha256(scaled.read_bytes()).digest() == digest
            print('PASS: UserUnit half/double retain the same physical marker in native fit/actual-size output', flush=True)

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
                    check_output(output, [0], result)
                    bounds = marker_bounds(output, channel=2)
                    if unit == 1:
                        m = result['metrics'][0]
                        pw,ph = m['source_width'],m['source_height']
                        dw,dh,cw,ch = m['draw_width'],m['draw_height'],m['printable_width'],m['printable_height']
                        w,h = bounds[4:]
                        expected = [(36+(cw-dw)/2+25*72/25.4*dw/pw)*w/(cw+72),
                                    (36+(ch-dh)/2+40*72/25.4*dh/ph)*h/(ch+72),
                                    40*72/25.4*dw/pw*w/(cw+72),20*72/25.4*dh/ph*h/(ch+72)]
                        actual = [bounds[0],bounds[1],bounds[2]-bounds[0],bounds[3]-bounds[1]]
                        assert all(abs(a-b)<=3 for a,b in zip(actual,expected)), (scaling,actual,expected)
                        mark_bounds[scaling] = bounds
                    else:
                        expected = mark_bounds[scaling]
                        assert bounds[4:] == expected[4:] and all(abs(a-b)<=3 for a,b in zip(bounds[:4],expected[:4])), ('Printed mark physical size mismatch',unit,scaling,bounds,expected)
                assert hashlib.sha256(original_page.read_bytes()).digest() == original_hash
                assert hashlib.sha256(marked.read_bytes()).digest() == marked_hash
            print('PASS: exported 40x20-mm mark retains native fit/actual-size print geometry on units 1/2/0.5', flush=True)

            output = folder / 'cancelled.pdf'
            result=native(output,'2',cancel=True)
            assert result['status']=='cancelled' and not output.exists(),result
            print('PASS: native GTK dialog cancellation and actual Print-to-File acceptance', flush=True)

            alias = folder / 'source alias.pdf'
            os.link(source, alias)
            result = subprocess.run(command(alias, '1'), env=env, capture_output=True, text=True, timeout=30)
            assert result.returncode != 0 and hashlib.sha256(source.read_bytes()).digest() == original
            print('PASS: native print-to-file refuses a protected input alias', flush=True)

            if args.stress:
                benchmark = Benchmark('gtk-print-to-file', binary)
                long_source=folder / '50-page office fixture.pdf'
                long_source.write_bytes(sample.make_pdf(50,print_marker=True))
                digest=hashlib.sha256(long_source.read_bytes()).digest()
                output=folder / '50-page printed.pdf'
                benchmark.measure('all-50-fit', long_source, output, list(range(50)), 'fit',
                                  lambda: native(output, input_pdf=long_source),
                                  lambda result: check_output(output, list(range(50)), result))
                selected=folder / 'printed page 7.pdf'
                benchmark.measure('page-7-fit', long_source, selected, [6], 'fit',
                                  lambda: native(selected, '7', input_pdf=long_source),
                                  lambda result: check_output(selected, [6], result))
                assert hashlib.sha256(long_source.read_bytes()).digest()==digest
                print('PASS: page 7 of the 50-page fixture prints alone; original remains unchanged',flush=True)
                if args.benchmark_report:
                    benchmark.write(args.benchmark_report)

            # Launch the actual reader, then use OS key events to launch and
            # cancel printing through the snapshot worker/broker path.
            second = folder / 'second-reader.pdf'
            second.write_bytes(sample.make_pdf(2))
            parent = subprocess.Popen([binary, str(source), str(second)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
            try:
                deadline = time.monotonic()+20
                main_window=None
                while time.monotonic()<deadline:
                    found=subprocess.run(['xdotool','search','--onlyvisible','--name','second-reader.pdf'],env=env,capture_output=True,text=True)
                    if found.returncode==0:
                        main_window=found.stdout.splitlines()[-1];break
                    time.sleep(0.05)
                assert main_window,'Reader window did not open both documents'
                run(['xdotool','windowfocus',main_window]);run(['xdotool','key','ctrl+p'])
                deadline=time.monotonic()+15;print_window=None
                while time.monotonic()<deadline:
                    found=subprocess.run(['xdotool','search','--onlyvisible','--name','Print'],env=env,capture_output=True,text=True)
                    if found.returncode==0:
                        candidates=[window for window in found.stdout.splitlines() if window!=main_window]
                        if candidates:print_window=candidates[-1];break
                    time.sleep(0.05)
                assert print_window,'Ctrl+P did not open the native print process'
                run(['xdotool','windowfocus',main_window]);run(['xdotool','key','ctrl+Tab'])
                deadline=time.monotonic()+5
                while time.monotonic()<deadline:
                    title=run(['xdotool','getwindowname',main_window]).stdout
                    if source.name in title:break
                    time.sleep(0.05)
                assert source.name in title,'Reader did not respond while native print dialog was open'
                run(['xdotool','windowfocus',print_window]);run(['xdotool','key','Escape'])
                deadline=time.monotonic()+5
                while time.monotonic()<deadline:
                    status=subprocess.run(['xdotool','getwindowname',print_window],env=env,capture_output=True)
                    if status.returncode:break
                    time.sleep(0.05)
                assert status.returncode,'Native print cancellation did not close the dialog'
                print('PASS: real reader Ctrl+P transfers its snapshot, navigation remains responsive, and cancellation closes the print process',flush=True)
            finally:
                if parent.poll() is None:
                    parent.terminate();parent.wait(timeout=5)



if __name__ == '__main__':
    main()
