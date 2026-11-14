"""Check declared minimum Rust on an approved native build runner.

This checks source compatibility, including test code; it does not build a
replacement release artifact or qualify native desktop/package behavior.
"""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import tempfile
import tomllib


def minimum_toolchain(source):
    value = tomllib.loads((source / 'Cargo.toml').read_text(encoding='utf-8'))['package']['rust-version']
    if not isinstance(value, str) or not re.fullmatch(r'\d+\.\d+(?:\.\d+)?', value):
        raise ValueError('Require a numeric declared minimum Rust version')
    return value if value.count('.') == 2 else value + '.0'


def native_run(command, source, env, log, timeout):
    log.parent.mkdir(parents=True, exist_ok=True)
    if os.name == 'nt':
        from windows_job import run
        return run(command, source, env, log, timeout)
    # A timed-out Cargo must not leave a compiler child behind. This is the
    # existing public Linux runner, not the source workstation's Cargo wrapper.
    with log.open('wb') as stream:
        child = subprocess.Popen(command, cwd=source, env=env, stdout=stream,
                                 stderr=subprocess.STDOUT, start_new_session=True)
        try:
            return child.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            return 'timeout'
        finally:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait()


def git(source, *args):
    return subprocess.check_output(['git', '-C', str(source), *args], text=True).strip()


def native_admission():
    if os.name == 'nt':
        from validate_windows import admission
        admission()


def check(source, commit, output, runner=native_run):
    source, output = source.resolve(), output.resolve()
    report = dict(schema=1, scope='native minimum Rust source compatibility', status='FAIL',
                  source_commit=commit, platform=os.name, checks=[],
                  started_at=datetime.now(timezone.utc).isoformat(),
                  unqualified=['release binary built with minimum Rust',
                               'runtime/PDF/desktop/package acceptance on minimum Rust'])
    output.mkdir(parents=True, exist_ok=True)
    try:
        if os.name not in ('nt', 'posix') or (os.name == 'posix' and not hasattr(os, 'sched_getaffinity')):
            raise ValueError('Use the approved native Linux or Windows build runner')
        if git(source, 'rev-parse', 'HEAD') != commit or git(source, 'status', '--porcelain', '--untracked-files=all'):
            raise ValueError('Require the requested clean source commit')
        toolchain = minimum_toolchain(source)
        report['minimum_toolchain'] = toolchain
        native_admission()
        # Keep metadata and generated UI outputs out of the qualified runtime's
        # target/cache. Do not change the runner's default stable toolchain.
        with tempfile.TemporaryDirectory(prefix='dpdf-minimum-rust-') as scratch:
            env = dict(os.environ, CARGO_TARGET_DIR=str(Path(scratch) / 'target'),
                       CARGO_BUILD_JOBS='1', CARGO_INCREMENTAL='0', RAYON_NUM_THREADS='1',
                       RUSTUP_TOOLCHAIN=toolchain)
            for key in ('RUSTFLAGS', 'RUSTDOCFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_ENCODED_RUSTDOCFLAGS'):
                env.pop(key, None)
            rustup, rustc, cargo = (shutil.which(name) for name in ('rustup', 'rustc', 'cargo'))
            if not all((rustup, rustc, cargo)):
                raise ValueError('Rustup and native Rust shims must be installed on the runner')
            # Cargo must use the same shim/version that the probe checks, even
            # when a runner profile contains a different compiler or wrapper.
            env.update(RUSTC=rustc, RUSTC_WRAPPER='', RUSTC_WORKSPACE_WRAPPER='',
                       CARGO_BUILD_RUSTC=rustc, CARGO_BUILD_RUSTC_WRAPPER='',
                       CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='')
            commands = [('install', [rustup, 'toolchain', 'install', toolchain, '--profile', 'minimal',
                                     '--no-self-update'], 600),
                        ('compiler', [rustc, '+' + toolchain, '-vV'], 120),
                        ('check-all-targets', [cargo, '+' + toolchain, 'check', '--locked', '--all-targets'], 1500)]
            if os.name != 'nt':
                commands.append(('check-windows-print-metadata',
                                 [cargo, '+' + toolchain, 'check', '--locked', '--all-targets',
                                  '--features', 'check-windows-print'], 1500))
            for name, command, timeout in commands:
                log = output / (name + '.log')
                code = runner(command, source, env, log, timeout)
                report['checks'].append(dict(name=name, status='PASS' if code == 0 else 'FAIL',
                                             exit_code=code, command=command))
                if code != 0:
                    raise RuntimeError('Minimum Rust ' + name + ' failed; see retained log')
                if name == 'compiler':
                    text = log.read_text(encoding='utf-8', errors='replace')
                    host = 'x86_64-pc-windows-msvc' if os.name == 'nt' else 'x86_64-unknown-linux-gnu'
                    if not text.startswith('rustc ' + toolchain + ' ') or 'host: ' + host not in text:
                        report['checks'][-1]['status'] = 'FAIL'
                        raise ValueError('Actual compiler version/native target differs from the requested minimum')
                    report['compiler'] = text
            if git(source, 'rev-parse', 'HEAD') != commit or git(source, 'status', '--porcelain', '--untracked-files=all'):
                raise ValueError('Minimum Rust checks changed the source checkout')
        report['status'] = 'PASS'
    except Exception as error:
        report['error'] = str(error)
    finally:
        report['finished_at'] = datetime.now(timezone.utc).isoformat()
        (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
        (output / 'README.md').write_text(
            '# Minimum Rust source checks\n\n' + report['status'] + '\n\n' +
            'Source: ' + commit + '\n\n' + report.get('error', '') + '\n\n' +
            'Source compatibility only; this does not replace the qualified stable runtime.\n', encoding='utf-8')
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    report = check(args.source, args.commit, args.output)
    print(json.dumps(report, indent=2))
    return 0 if report['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
