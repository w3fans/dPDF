"""UI client for the disposable loopback-only print server; generated PDF only."""
import argparse, importlib.util, json, os, subprocess, time
from pathlib import Path
p = argparse.ArgumentParser()
p.add_argument('--binary', type=Path, required=True)
p.add_argument('--source', type=Path, required=True)
p.add_argument('--outdir', type=Path, required=True)
p.add_argument('--range', default='7')
p.add_argument('--cancel', action='store_true')
p.add_argument('--mutate-source', action='store_true')
p.add_argument('--expect-failure', action='store_true')
p.add_argument('--expect-raster', action='store_true')
a = p.parse_args()
assert os.getuid() != 0, 'Non-root disposable fixture user required'
scripts = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('display', scripts / 'private-display.py')
display = importlib.util.module_from_spec(spec)
spec.loader.exec_module(display)
report = dict(status='FAIL', physical_printer=False, range=a.range, cancel=a.cancel, mutate_source=a.mutate_source, expect_failure=a.expect_failure)
process = None
try:
    with display.display(a.outdir, 'Xvfb') as env:
        env.update(GTK_PRINT_BACKENDS='cups', GTK_USE_PORTAL='0', LC_ALL='C.UTF-8', GSETTINGS_BACKEND='memory', DPDF_PDFIUM_LIBRARY=os.environ['DPDF_PDFIUM_LIBRARY'])
        assert env['CUPS_SERVER'].startswith('127.0.0.1:')
        data = a.source.read_bytes()
        started = time.monotonic()
        if a.expect_failure:
            assert env.get('DPDF_PRIVATE_PROXY_REFUSES_PRINT_JOB') == '1', 'A controlled refusal proxy is required'
        process = subprocess.Popen([str(a.binary), '--native-print', str(a.source), '6', a.range, 'actual', 'dPDF-controlled'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
        process.stdin.write(data)
        process.stdin.close()
        process.stdin = None
        deadline = time.monotonic() + 20
        window = None
        while time.monotonic() < deadline and process.poll() is None:
            r = subprocess.run(['xdotool', 'search', '--all', '--onlyvisible', '--pid', str(process.pid), '--name', '^Print$'], env=env, capture_output=True, text=True)
            if r.returncode == 0 and r.stdout.strip():
                window = r.stdout.splitlines()[-1]
                break
            time.sleep(0.1)
        assert window, 'Native print dialog did not appear'
        if a.mutate_source:
            fixture_spec = importlib.util.spec_from_file_location('sample', scripts / 'make-sample.py')
            sample = importlib.util.module_from_spec(fixture_spec)
            fixture_spec.loader.exec_module(sample)
            a.source.write_bytes(sample.make_pdf(3))
        subprocess.run(['xdotool', 'windowfocus', '--sync', window], env=env, check=True, capture_output=True)
        time.sleep(1)
        subprocess.run(['xdotool', 'key', 'Escape' if a.cancel else 'Return'], env=env, check=True, capture_output=True)
        if a.expect_raster and (not a.cancel):
            deadline = time.monotonic() + 20
            confirmation = None
            while time.monotonic() < deadline and process.poll() is None:
                r = subprocess.run(['xdotool', 'search', '--all', '--onlyvisible', '--pid', str(process.pid), '--name', '^Print$'], env=env, capture_output=True, text=True)
                for item in r.stdout.splitlines():
                    if item != window:
                        confirmation = item
                        break
                if confirmation:
                    break
                time.sleep(0.1)
            assert confirmation, 'Raster fallback confirmation did not appear'
            subprocess.run(['xdotool', 'windowfocus', '--sync', confirmation], env=env, check=True, capture_output=True)
            time.sleep(1)
            subprocess.run(['xdotool', 'key', 'Return'], env=env, check=True, capture_output=True)
        stdout, stderr = process.communicate(timeout=60)
        (a.outdir / 'stdout.json').write_bytes(stdout)
        (a.outdir / 'stderr.log').write_bytes(stderr)
        if a.expect_failure:
            assert process.returncode != 0 and b'CUPS PDF submission failed' in stderr, stderr.decode(errors='replace')
            report.update(status='PASS', submission_refused_once=True)
        else:
            assert process.returncode == 0, 'Native print exited ' + str(process.returncode) + ': ' + stderr.decode(errors='replace')[-2000:]
            receipt = json.loads(stdout)
            assert receipt['status'] == ('cancelled' if a.cancel else 'submitted'), receipt
            if not a.cancel:
                if a.expect_raster:
                    selected = set(range(50)) if not a.range else set()
                    for part in a.range.split(',') if a.range else []:
                        first, last = part.split('-') if '-' in part else (part, part)
                        selected.update(range(int(first) - 1, int(last)))
                    assert receipt['pages_drawn'] == sorted(selected), receipt
                else:
                    assert receipt.get('transport') == 'cups-pdf', receipt
            report.update(status='PASS', seconds=round(time.monotonic() - started, 3), source_bytes=len(data), receipt=receipt)
except Exception as e:
    report['error'] = str(e)
finally:
    if process and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
    (a.outdir / 'client-report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2), flush=True)
raise SystemExit(0 if report['status'] == 'PASS' else 1)
