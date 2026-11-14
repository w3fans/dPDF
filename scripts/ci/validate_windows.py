"""Native MSVC regression lane; isolated data, bounded resources, immutable source."""
import argparse
import ctypes
from ctypes import wintypes
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

# The optional embedded Python has an isolated sys.path.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from runtime import prepare
from windows_job import run


def git(source, *args):
    return subprocess.check_output(['git', '-C', str(source), *args], text=True).strip()


def plan(source, binary, pdfium, qpdf, mode, python=sys.executable, cargo='cargo.exe', probe_report=None,
         print_report=None):
    tests = [('build', ['powershell.exe', '-NoProfile', '-NonInteractive', '-ExecutionPolicy',
                        'Bypass', '-File', str(source / 'build.ps1'), '-Release'], 2700),
             ('unit-tests', [cargo, 'test', '--locked', '--release'], 1500),
             ('mark-metadata-contention', [python, str(source / 'scripts/ci/trace_mark_metadata.py'),
                                           '--cargo', cargo, '--cases', '32'], 240),
             ('clippy', [cargo, 'clippy', '--locked', '--', '-D', 'warnings'], 1500),
             ('format', [cargo, 'fmt', '--check'], 120)]
    common = ['--binary', str(binary)]
    for name in ('reader', 'pages', 'marks', 'overlays', 'page-units', 'review', 'aliases'):
        args = common.copy()
        if name != 'marks':
            args += ['--pdfium', str(pdfium)]
        if name not in ('reader', 'marks'):
            args += ['--qpdf', str(qpdf)]
        tests.append((name, [python, str(source / 'scripts' / f'check-{name}.py'), *args], 600))
    tests.append(('geometry', [python, str(source / 'scripts' / 'benchmark-reader.py'),
                              *common, '--pdfium', str(pdfium), '--geometry-only'], 300))
    tests.append(('engine-paths', [python, str(source / 'scripts' / 'check-engine-paths.py'),
                                  *common, '--pdfium', str(pdfium)], 300))
    command = [python, str(source / 'scripts' / 'check-block-rust.py'), *common,
               '--pdfium', str(pdfium), '--qpdf', str(qpdf)]
    if probe_report is not None:
        command += ['--output', str(probe_report)]
    tests.append(('block-probe', command, 1200))
    tests.append(('staged-reader', [python, str(source / 'scripts' / 'check-windows-stage.py'),
                                    *common, '--pdfium', str(pdfium), '--qpdf', str(qpdf)], 180))
    tests.append(('user-registration', [python, str(source / 'scripts' / 'check-windows-registration.py'),
                                        *common, '--pdfium', str(pdfium)], 300))
    if mode == 'desktop':
        for scale in ('1', '1.5', '2'):
            tests.append(('native-file-drop-' + scale.replace('.', '-'),
                          [python, str(source / 'scripts' / 'check-windows-drop.py'), *common,
                           '--pdfium', str(pdfium), '--scale', scale, '--allow-native-input'], 180))
            tests.append(('viewport-' + scale.replace('.', '-'),
                          [python, str(source / 'scripts' / 'check-viewport.py'), *common,
                           '--pdfium', str(pdfium), '--scale', scale], 180))
            tests.append(('physical-reader-' + scale.replace('.', '-'),
                          [python, str(source / 'scripts' / 'check-physical-reader.py'), *common,
                           '--pdfium', str(pdfium), '--qpdf', str(qpdf), '--scale', scale], 300))
            tests.append(('document-tabs-' + scale.replace('.', '-'),
                          [python, str(source / 'scripts' / 'check-tabs.py'), *common,
                           '--pdfium', str(pdfium), '--scale', scale], 180))
            for marks in (False, True):
                arguments = [*common, '--scale', scale, '--allow-native-clipboard']
                arguments += ['--marks'] if marks else ['--placement', '--pdfium', str(pdfium), '--qpdf', str(qpdf)]
                name = ('clipboard-image-' if marks else 'clipboard-placement-') + scale.replace('.', '-')
                tests.append((name, [python, str(source / 'scripts' / 'check-clipboard.py'), *arguments], 180))
        for name in ('about', 'review-ui', 'passwords', 'recovery', 'windows-print'):
            args = common.copy()
            if name != 'about':
                args += ['--pdfium', str(pdfium), '--qpdf', str(qpdf)]
            if name == 'windows-print':
                args += ['--allow-native-dialog', '--stress']
                if print_report is not None:
                    args += ['--benchmark-report', str(print_report)]
            tests.append((name, [python, str(source / 'scripts' / f'check-{name}.py'), *args], 900))
    return tests


BLOCK_REFUSALS = ['crossing-path', 'crossing-text', 'marked', 'text-clip', 'path-clip',
                  'annotation', 'form', 'empty', 'catalog-stream', 'catalog-attachment', 'page-tree-stream',
                  'untouched-page-pieceinfo', 'untouched-page-resources', 'shared-contents']


def load_block_report(path, binary_sha256):
    with path.open('rb') as stream:
        raw = stream.read(262145)
    if len(raw) > 262144:
        raise ValueError('Block probe report exceeds its metadata budget')
    report = json.loads(raw)
    if not isinstance(report, dict):
        raise ValueError('Block probe report must be an object')
    if (report.get('scope') != 'generated_rust_block_probe_only' or
            report.get('binary_sha256') != binary_sha256 or
            report.get('qualified_for_secure_export') is not False or
            report.get('dPDF_secure_export_enabled') is not False or
            report.get('requested_cases') != list(range(32))):
        raise ValueError('Block probe identity, coverage or qualification mismatch')
    if report.get('status') == 'PASS':
        cases = report.get('cases', [])
        if (not isinstance(cases, list) or not all(isinstance(case, dict) for case in cases) or
                [case.get('case') for case in cases] != list(range(32)) or
                not isinstance(report.get('refusals'), list) or
                not all(isinstance(item, dict) for item in report['refusals']) or
                [item.get('case') for item in report['refusals']] != BLOCK_REFUSALS or
                not isinstance(report.get('graph_cases'), list) or
                not all(isinstance(item, dict) for item in report['graph_cases']) or
                [item.get('case') for item in report['graph_cases']] != ['indirect-resources']):
            raise ValueError('Complete block probe report lacks required cases/refusals')
    elif report.get('status') != 'INCOMPLETE':
        raise ValueError('Unexpected block probe status')
    return report


def environment(root, target):
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(('DPDF_', 'SLINT_', 'CARGO_', 'RUSTFLAGS', 'RUSTDOCFLAGS'))}
    # Keep installation homes selected by the runner installer, never use dGK's target/cache.
    for key in ('CARGO_HOME', 'RUSTUP_HOME', 'RUSTUP_TOOLCHAIN'):
        if key in os.environ:
            env[key] = os.environ[key]
    for name in ('data', 'config', 'temp'):
        (root / name).mkdir()
    env.update(DPDF_DATA_DIR=str(root / 'data'), DPDF_CONFIG_DIR=str(root / 'config'),
               TEMP=str(root / 'temp'), TMP=str(root / 'temp'),
               CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS='1', RUST_TEST_THREADS='1',
               RAYON_NUM_THREADS='1', PYTHONDONTWRITEBYTECODE='1', PYTHONUTF8='1', PYTHONOPTIMIZE='0',
               RUSTUP_TOOLCHAIN='stable-x86_64-pc-windows-msvc')
    return env


def desktop_context():
    # Read-only checks: never activate/switch/unlock the user's desktop.
    user = ctypes.WinDLL('user32', use_last_error=True)
    user.GetProcessWindowStation.argtypes = []
    user.GetProcessWindowStation.restype = wintypes.HANDLE
    user.GetUserObjectInformationW.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p,
                                              wintypes.DWORD, ctypes.POINTER(wintypes.DWORD)]
    user.GetUserObjectInformationW.restype = wintypes.BOOL
    user.OpenInputDesktop.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    user.OpenInputDesktop.restype = wintypes.HANDLE
    user.CloseDesktop.argtypes = [wintypes.HANDLE]
    user.CloseDesktop.restype = wintypes.BOOL

    class FLAGS(ctypes.Structure):
        _fields_ = [('inherit', wintypes.BOOL), ('reserved', wintypes.BOOL), ('flags', wintypes.DWORD)]
    session, size, flags = wintypes.DWORD(), wintypes.DWORD(), FLAGS()
    if not ctypes.windll.kernel32.ProcessIdToSessionId(os.getpid(), ctypes.byref(session)):
        raise ctypes.WinError()
    visible = bool(user.GetUserObjectInformationW(user.GetProcessWindowStation(), 1,
                   ctypes.byref(flags), ctypes.sizeof(flags), ctypes.byref(size)) and flags.flags & 1)
    desktop = user.OpenInputDesktop(0, False, 1)  # DESKTOP_READOBJECTS only
    accessible = bool(desktop)
    if desktop:
        user.CloseDesktop(desktop)
    return dict(session_id=session.value, station_visible=visible, input_desktop_accessible=accessible)


def admission(seconds=600):
    """Don't compete with an existing native build or low-memory desktop."""
    class MEMORY(ctypes.Structure):
        _fields_ = [('length', wintypes.DWORD), ('load', wintypes.DWORD)] + [
            (name, ctypes.c_ulonglong) for name in
            ('total', 'available', 'page_total', 'page_available', 'virtual_total',
             'virtual_available', 'extended_available')]
    deadline = time.monotonic() + seconds
    while True:
        memory = MEMORY()
        memory.length = ctypes.sizeof(memory)
        if not ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(memory)):
            raise ctypes.WinError()
        import csv
        rows = csv.reader(subprocess.check_output(['tasklist.exe', '/FO', 'CSV', '/NH'],
                                                  text=True, errors='replace').splitlines())
        busy = any(row and row[0].lower() in ('cargo.exe', 'rustc.exe', 'link.exe', 'cl.exe')
                   for row in rows)
        if not busy and memory.available >= 3 * 1024 ** 3:
            return
        if time.monotonic() >= deadline:
            raise RuntimeError('Admission timed out: another build is active or less than 3 GiB RAM is free')
        time.sleep(10)


def write_report(report, output):
    output.mkdir(parents=True, exist_ok=True)
    (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    lines = [f"# Windows validation: {report['lane']}", '',
             f"Result: **{report['status']}**", '',
             f"Source: `{report['source_ref']}` / `{report['source_commit']}`", '',
             f"Run: {report['run_url']}", '',
             '| Check | Result | Seconds |', '| --- | --- | ---: |']
    lines += [f"| {item['name']} | {item['status']} | {item.get('seconds', 0):.1f} |"
              for item in report['checks']]
    if 'error' in report:
        lines += ['', 'Preflight/finalization error: ' + report['error']]
    if 'notice_inventory_status' in report:
        lines += ['', 'Dependency notice collection: ' + report['notice_inventory_status'] + ' (inventory only).']
    lines += ['', 'Limits: one build job, one logical CPU equivalent, 2 GiB aggregate child memory.',
              '', 'Unqualified by this lane: ' + '; '.join(report['unqualified']) + '.', '']
    (output / 'README.md').write_text('\n'.join(lines), encoding='utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--ref', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--lane', choices=['hosted', 'self-hosted', 'desktop'], required=True)
    args = parser.parse_args()
    source, output = args.source.resolve(), args.output.resolve()
    report = dict(schema=1, lane=args.lane, runner_name=os.environ.get('RUNNER_NAME', 'local'),
                  runner_environment=os.environ.get('RUNNER_ENVIRONMENT', 'local'), source_ref=args.ref, source_commit=args.commit,
                  status='FAIL', checks=[], run_attempt=os.environ.get('GITHUB_RUN_ATTEMPT', 'local'), started_at=datetime.now(timezone.utc).isoformat(),
                  run_url=f"https://github.com/{os.environ.get('GITHUB_REPOSITORY', 'w3fans/dPDF')}/actions/runs/{os.environ.get('GITHUB_RUN_ID', 'local')}",
                  unqualified=['physical printer output', 'manual pointer/clipboard interaction',
                               'installer and default-app integration'])
    if args.lane != 'desktop':
        report['unqualified'].append('interactive windows, password/recovery UI and native print dialog')
    root = None
    try:
        if os.name != 'nt' or ctypes.sizeof(ctypes.c_void_p) != 8:
            raise RuntimeError('This validator requires native 64-bit Windows')
        if git(source, 'rev-parse', 'HEAD') != args.commit:
            raise RuntimeError('Checkout identity differs from resolved commit')
        if git(source, 'status', '--porcelain', '--untracked-files=all'):
            raise RuntimeError('Source checkout is not clean')
        context = desktop_context()
        report['desktop_context'] = context
        if args.lane == 'desktop' and not (context['session_id'] and context['station_visible']
                                           and context['input_desktop_accessible']):
            raise RuntimeError('Desktop tests require a dedicated logged-in, visible, accessible input desktop; never Session 0')
        admission()
        import tempfile
        root = Path(tempfile.mkdtemp(prefix='dpdf-validation-')).resolve()
        # Job-limited Cargo uses its own cache outside the source checkout.
        cache = Path(os.environ.get('DPDF_CI_ROOT', os.environ.get('RUNNER_TEMP', str(root))))
        target = cache / 'dpdf-msvc-target'
        env = environment(root, target)
        if args.lane == 'desktop':
            env['DPDF_TEST_NATIVE_DISPLAY'] = '1'
        runtimes = prepare(root / 'runtime')
        report['runtimes'] = runtimes
        env.update(DPDF_PDFIUM_LIBRARY=runtimes['pdfium'], DPDF_QPDF_BINARY=runtimes['qpdf'])
        binary = target / 'release' / 'dpdf.exe'
        cargo = shutil.which('cargo.exe')
        if not cargo:
            raise RuntimeError('Native cargo.exe is unavailable to this runner account')
        # Toolchain commands are also under the same Job Object limits.
        contract_log = output / 'logs' / 'ci-contract.log'
        if run([sys.executable, str(Path(__file__).with_name('test_ci.py'))], source, env, contract_log, 60):
            raise RuntimeError('Native CI contract/process-tree checks failed; see log')
        report['checks'].append(dict(name='ci-contract', status='PASS'))
        parser_log = output / 'logs' / 'powershell-syntax.log'
        parser_script = Path(__file__).with_name('check-powershell.ps1')
        if run(['powershell.exe', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
                '-File', str(parser_script), '-Source', str(source)], source, env, parser_log, 60):
            raise RuntimeError('PowerShell source parsing failed; see log')
        report['checks'].append(dict(name='powershell-syntax', status='PASS'))
        for name, command in [('toolchain', [cargo, '-vV']), ('compiler', ['rustc.exe', '-vV'])]:
            log = output / 'logs' / (name + '.log')
            if run(command, source, env, log, 120):
                raise RuntimeError(f'{name} probe failed; see log')
            text = log.read_text(encoding='utf-8', errors='replace')
            report[name] = text
            if 'host: x86_64-pc-windows-msvc' not in text:
                raise RuntimeError('A native x86_64 MSVC toolchain is required')
        import artifact_cache
        adapter, adapter_args = artifact_cache.native_adapter(source, Path(__file__).resolve().parents[2])
        artifact_identity = artifact_cache.identity(args.commit, report['compiler'], report['toolchain'], env,
                                                    artifact_cache.digest(adapter))
        report['artifact_identity'] = artifact_identity
        artifact_root = Path(os.environ.get('DPDF_NATIVE_ARTIFACT_ROOT') or cache)
        cached_binary = artifact_cache.load(artifact_root, artifact_identity)
        if cached_binary is not None:
            binary = cached_binary
        build_ok = False
        probe_report = output / 'block-probe.json'
        tests = plan(source, binary, runtimes['pdfium'], runtimes['qpdf'],
                     'desktop' if args.lane == 'desktop' else 'service', cargo=cargo,
                     probe_report=probe_report, print_report=output / 'print-benchmark.json')
        for name, command, timeout in tests:
            if name not in ('build', 'format', 'unit-tests', 'clippy') and not build_ok:
                report['checks'].append(dict(name=name, status='SKIP', reason='build failed'))
                continue
            report['active_check'] = name
            write_report(report, output)
            print(f'Running {name}', flush=True)
            started = time.monotonic()
            if name == 'build' and cached_binary is None:
                command[command.index('-File') + 1] = str(adapter)
                command += adapter_args
                command += ['-ArtifactManifest', str(root / 'cargo-artifacts.jsonl')]
            if name == 'build' and cached_binary is not None:
                (output / 'logs' / 'build.log').write_text('Reused hash-verified build for the exact commit/toolchain/flags. Native acceptance still runs.\n')
                code = 0
                report['artifact_reused'] = True
            else:
                code = run(command, source, env, output / 'logs' / (name + '.log'), timeout)
            if name == 'build' and code == 0 and cached_binary is None:
                if git(source, 'rev-parse', 'HEAD') != args.commit or git(
                        source, 'status', '--porcelain', '--untracked-files=all'):
                    raise RuntimeError('Source changed during build; no artifact recorded')
                if artifact_cache.identity(args.commit, report['compiler'], report['toolchain'], env,
                                           artifact_cache.digest(adapter)) != artifact_identity:
                    raise RuntimeError('Build configuration/adapter changed; no artifact recorded')
                version_log = output / 'logs' / 'binary-version.log'
                produced = artifact_cache.built_binary(root / 'cargo-artifacts.jsonl', target)
                if run([sys.executable, str(source / 'scripts/release-version.py'), 'verify',
                        '--binary', str(produced)], source, env, version_log, 60):
                    raise RuntimeError('Built binary version verification failed')
                immutable = artifact_cache.store(artifact_root, artifact_identity, produced)
                for _, future, _ in tests:
                    future[:] = [str(immutable) if str(arg) == str(binary) else arg for arg in future]
                binary = immutable
                report['artifact_reused'] = False
            if name == 'block-probe':
                try:
                    report['block_probe'] = load_block_report(probe_report, report['binary_sha256'])
                    if code == 0 and report['block_probe']['status'] != 'PASS':
                        raise ValueError('Successful block command reported incomplete coverage')
                except (OSError, ValueError, KeyError, TypeError) as error:
                    code = 1
                    with (output / 'logs' / (name + '.log')).open('a', encoding='utf-8') as stream:
                        stream.write('\nBlock report validation: ' + str(error) + '\n')
            report['checks'].append(dict(name=name, command=list(map(str, command)),
                                         status='PASS' if code == 0 else 'FAIL', exit_code=code,
                                         seconds=round(time.monotonic() - started, 3)))
            report.pop('active_check', None)
            text = (output / 'logs' / (name + '.log')).read_text(encoding='utf-8', errors='replace')
            skipped = [line.strip() for line in text.splitlines() if line.startswith('SKIP:')]
            if skipped:
                report['checks'][-1]['optional_skips'] = skipped
                report['unqualified'].extend(skipped)
            if name == 'build':
                build_ok = code == 0
                if build_ok:
                    report['binary_sha256'] = hashlib.sha256(binary.read_bytes()).hexdigest()
            write_report(report, output)
        notice_command = [sys.executable, str(Path(__file__).with_name('collect_notices.py')),
                          '--source', str(source), '--cargo', cargo, '--output', str(output / 'notices')]
        notice_code = run(notice_command, source, env, output / 'logs' / 'dependency-notices.log', 300)
        report['notice_inventory_status'] = 'inventory_only' if notice_code == 0 else 'unavailable'
        report['unqualified'].append('complete dependency/toolchain/native-library license audit')
        if git(source, 'status', '--porcelain', '--untracked-files=all'):
            raise RuntimeError('Validation changed source files; no changes will be committed')
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
    return 0 if report['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
