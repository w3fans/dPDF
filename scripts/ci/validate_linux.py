#!/usr/bin/env python3
"""Linux regression lane: PDF corpora and private-Xvfb window suites for one exact commit.

Runs against an already-built release executable with pinned PDFium and a qpdf helper.
Each suite writes its own log; report.json/README.md record exact identities and
results. Nothing here qualifies physical printing, Wayland/compositor input,
real file managers, installers or real-user acceptance.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

UNQUALIFIED = ['physical printer output', 'Wayland compositor input and clipboard',
               'real file-manager drops and default-app integration', 'installer packages',
               'manual pointer interaction and real-user acceptance',
               'complete dependency/toolchain/native-library license audit']


def plan(source, binary, pdfium, qpdf, xvfb, desktop=True, print_report=None):
    py = [sys.executable]
    script = lambda name: str(source / 'scripts' / name)
    engines = ['--binary', str(binary), '--pdfium', str(pdfium)]
    both = [*engines, '--qpdf', str(qpdf)]
    tests = [('reader', [*py, script('check-reader.py'), *engines], 600),
             ('geometry', [*py, script('benchmark-reader.py'), *engines, '--geometry-only'], 300),
             ('engine-paths', [*py, script('check-engine-paths.py'), *engines], 300),
             ('pages', [*py, script('check-pages.py'), *both], 900),
             ('marks', [*py, script('check-marks.py'), '--binary', str(binary)], 600),
             ('overlays', [*py, script('check-overlays.py'), *both], 1200),
             ('page-units', [*py, script('check-page-units.py'), *both], 600),
             ('review', [*py, script('check-review.py'), *both], 600),
             ('aliases', [*py, script('check-aliases.py'), *both], 600),
             ('block-probe', [*py, script('check-block-rust.py'), *both, '--poppler'], 1500),
             ('poppler', [*py, script('check-poppler.py'), *both], 900),
             ('desktop-entry', [*py, script('check-desktop.py')], 180)]
    if not desktop:
        return tests
    display = ['--xvfb', xvfb]
    tests += [('file-drop', [*py, script('check-file-drop.py'), *engines, *display], 300),
              ('about', [*py, script('check-about.py'), '--binary', str(binary), *display], 300)]
    for scale in ('1', '1.5', '2'):
        suffix = '-' + scale.replace('.', '-')
        scaled = [*display, '--scale', scale]
        tests += [('viewport' + suffix, [*py, script('check-viewport.py'), *engines, *scaled], 300),
                  ('physical-reader' + suffix, [*py, script('check-physical-reader.py'), *both, *scaled], 300),
                  ('tabs' + suffix, [*py, script('check-tabs.py'), *engines, *scaled], 300),
                  ('clipboard-placement' + suffix,
                   [*py, script('check-clipboard.py'), *both, *scaled, '--placement'], 300),
                  ('clipboard-image' + suffix,
                   [*py, script('check-clipboard.py'), '--binary', str(binary), *scaled, '--marks'], 300)]
    print_args = ['--stress']
    if print_report is not None:
        print_args += ['--benchmark-report', str(print_report)]
    tests += [('helper-responsiveness', [*py, script('check-helper-responsiveness.py'), *both, *display], 180),
              ('review-ui', [*py, script('check-review-ui.py'), *both, *display], 900),
              ('passwords', [*py, script('check-passwords.py'), *both, *display], 900),
              ('recovery', [*py, script('check-recovery.py'), *both, *display], 900),
              ('native-print', [*py, script('check-native-print.py'), *both, *display, *print_args], 900)]
    return tests


def environment(root):
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(('DPDF_', 'SLINT_', 'DISPLAY', 'WAYLAND_', 'XAUTHORITY'))}
    for name in ('data', 'config', 'temp'):
        (root / name).mkdir()
    env.update(DPDF_DATA_DIR=str(root / 'data'), DPDF_CONFIG_DIR=str(root / 'config'),
               TMPDIR=str(root / 'temp'), PYTHONDONTWRITEBYTECODE='1', PYTHONUTF8='1',
               PYTHONOPTIMIZE='0')
    return env


def git(source, *args):
    return subprocess.check_output(['git', '-C', str(source), *args], text=True).strip()


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write_report(report, output):
    output.mkdir(parents=True, exist_ok=True)
    (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    lines = ['# Linux validation', '', f"Result: **{report['status']}**", '',
             f"Source: `{report['source_ref']}` / `{report['source_commit']}`", '',
             f"Executable SHA-256: `{report.get('binary_sha256', 'unavailable')}`", '',
             f"Run: {report['run_url']}", '', '| Check | Result | Seconds |', '| --- | --- | ---: |']
    lines += [f"| {item['name']} | {item['status']} | {item.get('seconds', 0):.1f} |" for item in report['checks']]
    if 'error' in report:
        lines += ['', 'Preflight/finalization error: ' + report['error']]
    lines += ['', 'Unqualified by this lane: ' + '; '.join(report['unqualified']) + '.', '']
    (output / 'README.md').write_text('\n'.join(lines), encoding='utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=Path('.'))
    parser.add_argument('--commit', required=True)
    parser.add_argument('--ref', required=True)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    parser.add_argument('--xvfb', default='Xvfb')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--no-desktop', action='store_true', help='Skip private-Xvfb window suites')
    parser.add_argument('--only', action='append', help='Run only the named checks (repeatable)')
    args = parser.parse_args()
    source, output = args.source.resolve(), args.output.resolve()
    report = dict(schema=1, lane='linux-hosted', source_ref=args.ref, source_commit=args.commit,
                  status='FAIL', checks=[], started_at=datetime.now(timezone.utc).isoformat(),
                  runner_name=os.environ.get('RUNNER_NAME', 'local'),
                  run_url=f"https://github.com/{os.environ.get('GITHUB_REPOSITORY', 'w3fans/dPDF')}"
                          f"/actions/runs/{os.environ.get('GITHUB_RUN_ID', 'local')}",
                  unqualified=list(UNQUALIFIED))
    root = None
    try:
        if sys.platform != 'linux':
            raise RuntimeError('This validator requires Linux')
        if git(source, 'rev-parse', 'HEAD') != args.commit:
            raise RuntimeError('Checkout identity differs from the requested commit')
        before = git(source, 'status', '--porcelain', '--untracked-files=all')
        binary, pdfium, qpdf = args.binary.resolve(), args.pdfium.resolve(), args.qpdf.resolve()
        report.update(binary_sha256=sha256(binary), pdfium_sha256=sha256(pdfium),
                      qpdf_version=subprocess.check_output([str(qpdf), '--version'], text=True).splitlines()[0],
                      binary_version=subprocess.check_output([str(binary), '--version'], text=True).strip())
        if args.xvfb and not args.no_desktop and not shutil.which(args.xvfb):
            raise RuntimeError(f'{args.xvfb} is unavailable; pass --no-desktop to skip window suites')
        root = Path(tempfile.mkdtemp(prefix='dpdf-linux-validation-')).resolve()
        env = environment(root)
        tests = plan(source, binary, pdfium, qpdf, args.xvfb, desktop=not args.no_desktop,
                     print_report=output / 'print-benchmark.json')
        if args.only:
            unknown = set(args.only) - {name for name, _, _ in tests}
            if unknown:
                raise RuntimeError('Unknown checks: ' + ', '.join(sorted(unknown)))
            tests = [test for test in tests if test[0] in args.only]
        logs = output / 'logs'
        logs.mkdir(parents=True, exist_ok=True)
        for name, command, timeout in tests:
            print(f'::group::{name}' if os.environ.get('GITHUB_ACTIONS') else f'== {name}', flush=True)
            started = time.monotonic()
            log = logs / (name + '.log')
            with log.open('wb') as stream:
                try:
                    code = subprocess.run(command, cwd=source, env=env, stdout=stream, stderr=subprocess.STDOUT,
                                          timeout=timeout).returncode
                except subprocess.TimeoutExpired:
                    code = 'timeout'
            text = log.read_text(encoding='utf-8', errors='replace')
            print(text[-4000:], flush=True)
            if os.environ.get('GITHUB_ACTIONS'):
                print('::endgroup::', flush=True)
            item = dict(name=name, command=list(map(str, command)), status='PASS' if code == 0 else 'FAIL',
                        exit_code=code, seconds=round(time.monotonic() - started, 3))
            skipped = [line.strip() for line in text.splitlines() if line.startswith('SKIP:')]
            if skipped:
                item['optional_skips'] = skipped
                report['unqualified'].extend(skipped)
            report['checks'].append(item)
            write_report(report, output)
        if git(source, 'status', '--porcelain', '--untracked-files=all') != before:
            raise RuntimeError('Validation changed source files')
        if report['checks'] and all(check['status'] == 'PASS' for check in report['checks']):
            report['status'] = 'PASS'
    except Exception as error:
        report['error'] = str(error)
        print(f'Validation failed: {error}', flush=True)
    finally:
        report['finished_at'] = datetime.now(timezone.utc).isoformat()
        write_report(report, output)
        if root:
            shutil.rmtree(root, ignore_errors=True)
    failed = [check['name'] for check in report['checks'] if check['status'] != 'PASS']
    if failed:
        print('Failed checks: ' + ', '.join(failed), flush=True)
    return 0 if report['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
