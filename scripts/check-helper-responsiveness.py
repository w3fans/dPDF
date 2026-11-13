#!/usr/bin/env python3
"""Generated Linux helper stalls: reader navigation, retry, tab close and shutdown."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shlex
import signal
import subprocess
import sys
import tempfile
import time


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    parser.add_argument('--xvfb', default='Xvfb')
    args = parser.parse_args()
    if sys.platform != 'linux':
        raise RuntimeError('This helper/process-identity fixture requires Linux; native Windows remains separate')
    sample = module('helper_sample', 'make-sample.py')
    private = module('helper_display', 'private-display.py')
    pages_fixture = module('helper_page_fixtures', 'check-pages.py')
    with tempfile.TemporaryDirectory(prefix='dpdf-helper-window-') as directory:
        root = Path(directory)
        for mode in ('close', 'retry', 'shutdown', 'recovery', 'protected-print', 'protected-export',
                     'overlay-close', 'overlay-save', 'overlay-units', 'overlay-review',
                     'image-preflight', 'image-unit-write', 'image-save', 'pages-save', 'pages-retry',
                     'print-review-close', 'print-review-shutdown'):
            folder = root / mode
            folder.mkdir()
            source = folder / 'generated50.pdf'
            source.write_bytes(sample.make_pdf(50, user_unit=2 if mode in ('overlay-units', 'image-preflight', 'image-unit-write', 'image-save') else 1))
            if mode.startswith('protected-'):
                encrypted = folder / 'protected50.pdf'
                job = folder / 'generated-encryption.json'
                job.write_text(json.dumps({'inputFile': str(source), 'outputFile': str(encrypted),
                    'encrypt': {'userPassword': 'generated-only helper test',
                                'ownerPassword': 'generated-owner', '256bit': {}}}))
                job.chmod(0o600)
                try:
                    subprocess.run([str(args.qpdf.resolve()), '--job-json-file=' + str(job)],
                                   check=True, capture_output=True, timeout=10)
                finally:
                    job.unlink()
                source = encrypted
            inputs = []
            if mode.startswith('image-') or mode == 'pages-retry':
                inputs = [Path(__file__).with_name('fixtures').joinpath('page-photo.jpg').resolve()]
            if mode == 'pages-retry':
                png = folder / 'generated-transparent.png'
                png.write_bytes(pages_fixture.png(32, 16))
                secondary = folder / 'generated3.pdf'
                secondary.write_bytes(sample.make_pdf(3))
                inputs += [png, secondary]
            input_hashes = {path: hashlib.sha256(path.read_bytes()).hexdigest() for path in inputs}
            destination = folder / 'existing-output.pdf'
            destination.write_bytes(b'pre-existing generated destination must survive cancellation')
            destination_before = destination.read_bytes()
            original = hashlib.sha256(source.read_bytes()).hexdigest()
            ready = folder / 'ready.json'
            arm = folder / 'armed'
            driver = folder / 'helper.py'
            # Helpers exec directly without shell children. Metadata modes stall
            # the first inspection; protected modes stall every decryption.
            if mode in ('image-save', 'pages-save', 'pages-retry'):
                condition = 'arm.exists() and not ready.exists() and any(arg.startswith("--job-json-file=") for arg in sys.argv[1:])'
            elif mode == 'image-preflight':
                condition = 'arm.exists() and not ready.exists() and sys.argv[1:2]==["--json"]'
            elif mode == 'image-unit-write':
                condition = ('arm.exists() and not ready.exists() and '
                    'any(arg.startswith("--update-from-json=") and Path(arg.split("=",1)[1]).name=="units.json" for arg in sys.argv[1:])')
            elif mode == 'overlay-save':
                condition = 'arm.exists() and any(arg.startswith("--job-json-file=") for arg in sys.argv[1:])'
            elif mode == 'overlay-review' or mode.startswith('print-review-'):
                condition = 'arm.exists() and sys.argv[1:2]==["--json"] and Path(sys.argv[-1]).name=="source.pdf"'
            elif mode == 'overlay-units':
                condition = ('arm.exists() and sys.argv[1:2]==["--json"] and '
                             f'subprocess.check_output([{str(args.qpdf.resolve())!r}, "--show-npages", sys.argv[-1]]).strip()==b"1"')
            elif mode == 'overlay-close':
                condition = 'arm.exists() and sys.argv[1:2]==["--json"]'
            else:
                condition = ('"--decrypt" in sys.argv' if mode.startswith('protected-') else
                             'sys.argv[1:2]==["--json"] and not ready.exists()' +
                             (' and arm.exists()' if mode == 'recovery' else ''))
            driver.write_text(
                'import json,os,time,sys,subprocess\nfrom pathlib import Path\n'
                f'ready=Path({str(ready)!r});arm=Path({str(arm)!r})\n'
                f'if {condition}:\n'
                ' identity=json.dumps({"pid":os.getpid(),"start":Path(f"/proc/{os.getpid()}/stat").read_text().rsplit(")",1)[1].split()[19]})\n'
                ' (ready.parent/f"helper-{os.getpid()}.json").write_text(identity)\n'
                ' ready.write_text(identity)\n'
                ' deadline=time.monotonic()+30\n'
                ' while time.monotonic()<deadline:time.sleep(.02)\n'
                f'os.execv({str(args.qpdf.resolve())!r}, [{str(args.qpdf.resolve())!r}, *sys.argv[1:]])\n',
                encoding='utf-8')
            helper = folder / 'qpdf-fixture'
            helper.write_text('#!/bin/sh\nexec ' + shlex.quote(sys.executable) + ' ' +
                              shlex.quote(str(driver)) + ' "$@"\n', encoding='utf-8')
            helper.chmod(0o700)

            def owned_pids():
                result = []
                for record in folder.glob('helper-*.json'):
                    identity = json.loads(record.read_text())
                    pid = identity['pid']
                    try:
                        fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
                        data = Path(f'/proc/{pid}/cmdline').read_bytes()
                    except FileNotFoundError:
                        continue
                    if fields[19] == identity['start'] and (
                            str(driver).encode() in data.split(b'\0') or
                            (not data and fields[0] == 'Z')):
                        result.append(pid)
                return result

            try:
                with private.display(folder, args.xvfb) as env:
                    if mode == 'recovery':
                        prepared_env = dict(env, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()),
                            DPDF_QPDF_BINARY=str(args.qpdf.resolve()), DPDF_SMOKE_RECOVERY='write',
                            DPDF_SMOKE_PLACEMENT_IMAGE=str(Path(__file__).with_name('fixtures').joinpath('page-photo.jpg').resolve()),
                            SLINT_SCALE_FACTOR='1')
                        prepared = subprocess.run([str(args.binary.resolve()), '--smoke-ui', str(source)],
                            env=prepared_env, capture_output=True, text=True, timeout=45)
                        if prepared.returncode:
                            raise RuntimeError(prepared.stdout + prepared.stderr)
                        assert 'durable checkpoint written' in prepared.stdout
                        assert len(list((Path(env['DPDF_DATA_DIR']) / 'recovery').glob('*.dpdf'))) == 1
                        env['DPDF_SMOKE_RECOVERY'] = 'read'
                    env.update(DPDF_SMOKE_HELPER_PAGE_INPUTS=json.dumps([str(path) for path in inputs]),
                               DPDF_SMOKE_PLACEMENT_IMAGE=str(Path(__file__).with_name('fixtures').joinpath('page-photo.jpg').resolve()),
                               DPDF_SMOKE_PASSWORD_VALUE='generated-only helper test',
                               DPDF_SMOKE_HELPER_DESTINATION=str(destination),
                               GTK_PRINT_BACKENDS='file', GTK_USE_PORTAL='0',
                               DPDF_SMOKE_HELPER=mode, DPDF_SMOKE_HELPER_READY=str(ready),
                               DPDF_SMOKE_HELPER_ARM=str(arm),
                               DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()), DPDF_QPDF_BINARY=str(helper),
                               SLINT_SCALE_FACTOR='1')
                    started = time.monotonic()
                    result = subprocess.run([str(args.binary.resolve()), '--smoke-ui', str(source)],
                                            env=env, capture_output=True, text=True, timeout=15)
                    print(result.stdout, end='', flush=True)
                    if result.returncode:
                        print(result.stderr, end='', flush=True)
                    result.check_returncode()
                    assert ready.is_file(), 'Generated helper never started'
                    rendered = ('rendered page 25 while reviewed print helper remained stalled' if mode.startswith('print-review-') else
                                'rendered page 25 while page export helper remained stalled' if mode.startswith(('image-', 'pages-')) else
                                'rendered page 25 while export helper remained stalled' if mode.startswith('overlay-') else
                                'rendered page 25 while metadata remained stalled')
                    assert rendered in result.stdout
                    expected = {'print-review-close': 'tab close cancelled reviewed print before native handoff',
                                'print-review-shutdown': 'owned reviewed print preparation stayed pending until normal shutdown',
                                'close': 'closed stalled document and reopened without a stale completion',
                                'retry': 'retry replaced stalled metadata without accepting its cancelled completion',
                                'shutdown': 'quitting with a live metadata helper',
                                'recovery': 'recovery metadata stayed pending until normal shutdown',
                                'protected-print': 'protected print preparation stayed pending until normal shutdown',
                                'protected-export': 'tab close cancelled protected export without publishing a late result',
                                'overlay-close': 'tab close cancelled overlay export without publishing a late result',
                                'overlay-units': 'tab close cancelled overlay export without publishing a late result',
                                'overlay-save': 'owned overlay save stayed pending until normal shutdown',
                                'overlay-review': 'owned overlay save stayed pending until normal shutdown',
                                'image-preflight': 'tab close cancelled page export without publishing a late result',
                                'image-unit-write': 'tab close cancelled page export without publishing a late result',
                                'image-save': 'owned page export stayed pending until normal shutdown',
                                'pages-save': 'tab close cancelled page export without publishing a late result',
                                'pages-retry': 'page export retry published 55 pages after cancelling its prior generation'}[mode]
                    assert expected in result.stdout, expected
                    deadline = time.monotonic() + 1
                    while owned_pids() and time.monotonic() < deadline:
                        time.sleep(.01)
                    assert not owned_pids(), 'Owned helper remained live/unreaped after completion'
                    if mode == 'recovery':
                        assert len(list((Path(env['DPDF_DATA_DIR']) / 'recovery').glob('*.dpdf'))) == 1, 'Cancelled recovery lost its durable checkpoint'
                    if mode.startswith('protected-'):
                        assert len(list(folder.glob('helper-*.json'))) == 1, 'Cancelled queued decryption was started'
                    if mode.startswith(('overlay-', 'image-', 'pages-')):
                        assert not [p for p in folder.iterdir() if p.name.startswith(('.tmp', '.dpdf'))], 'Cancelled export left destination staging files'
                    if mode == 'pages-retry':
                        subprocess.run([str(args.qpdf.resolve()), '--check', str(destination)], check=True, capture_output=True, timeout=10)
                        assert subprocess.check_output([str(args.qpdf.resolve()), '--show-npages', str(destination)], timeout=10).strip() == b'55'
                        assert destination.read_bytes() != destination_before
                    else:
                        assert destination.read_bytes() == destination_before, 'Cancelled operation changed its existing destination'
                    for path, before in input_hashes.items():
                        assert hashlib.sha256(path.read_bytes()).hexdigest() == before, 'Page input changed during cancellation/retry'
                    assert hashlib.sha256(source.read_bytes()).hexdigest() == original
                    print(f'PASS: helper {mode}, reader responsive, child reaped, source unchanged '
                          f'({time.monotonic() - started:.2f}s)', flush=True)
            except subprocess.TimeoutExpired as error:
                print(error.stdout or '', error.stderr or '', flush=True)
                raise
            finally:
                for pid in owned_pids():
                    # Failure cleanup only for this generated fixture identity.
                    try:
                        if os.getpgid(pid) == pid:
                            os.killpg(pid, signal.SIGKILL)
                        else:
                            os.kill(pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass


if __name__ == '__main__':
    main()
