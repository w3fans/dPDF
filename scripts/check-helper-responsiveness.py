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
    with tempfile.TemporaryDirectory(prefix='dpdf-helper-window-') as directory:
        root = Path(directory)
        for mode in ('close', 'retry', 'shutdown', 'recovery', 'protected-print', 'protected-export'):
            folder = root / mode
            folder.mkdir()
            source = folder / 'generated50.pdf'
            source.write_bytes(sample.make_pdf(50))
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
            destination = folder / 'existing-output.pdf'
            destination.write_bytes(b'pre-existing generated destination must survive cancellation')
            destination_before = destination.read_bytes()
            original = hashlib.sha256(source.read_bytes()).hexdigest()
            ready = folder / 'ready.json'
            arm = folder / 'armed'
            driver = folder / 'helper.py'
            # Helpers exec directly without shell children. Metadata modes stall
            # the first inspection; protected modes stall every decryption.
            condition = ('"--decrypt" in sys.argv' if mode.startswith('protected-') else
                         'sys.argv[1:2]==["--json"] and not ready.exists()' +
                         (' and arm.exists()' if mode == 'recovery' else ''))
            driver.write_text(
                'import json,os,time,sys\nfrom pathlib import Path\n'
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
                    env.update(DPDF_SMOKE_PASSWORD_VALUE='generated-only helper test',
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
                    assert 'rendered page 25 while metadata remained stalled' in result.stdout
                    expected = {'close': 'closed stalled document and reopened without a stale completion',
                                'retry': 'retry replaced stalled metadata without accepting its cancelled completion',
                                'shutdown': 'quitting with a live metadata helper',
                                'recovery': 'recovery metadata stayed pending until normal shutdown',
                                'protected-print': 'protected print preparation stayed pending until normal shutdown',
                                'protected-export': 'tab close cancelled protected export without publishing a late result'}[mode]
                    assert expected in result.stdout, expected
                    deadline = time.monotonic() + 1
                    while owned_pids() and time.monotonic() < deadline:
                        time.sleep(.01)
                    assert not owned_pids(), 'Owned helper remained live/unreaped after completion'
                    if mode == 'recovery':
                        assert len(list((Path(env['DPDF_DATA_DIR']) / 'recovery').glob('*.dpdf'))) == 1, 'Cancelled recovery lost its durable checkpoint'
                    if mode.startswith('protected-'):
                        assert len(list(folder.glob('helper-*.json'))) == 1, 'Cancelled queued decryption was started'
                    assert destination.read_bytes() == destination_before, 'Cancelled operation changed its existing destination'
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
