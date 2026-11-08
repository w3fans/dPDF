"""Build once under the existing Windows limits, or reuse its exact artifact."""
import argparse
import json
import os
from pathlib import Path
import hashlib
import re
import shutil
import subprocess
import tempfile

import artifact_cache
import validate_windows


def reuse_hosted(source, commit, report, target, artifacts):
    """Recover exact cached bytes; never compile or infer controlled acceptance."""
    identity = report.get('artifact_identity', {})
    checks = report.get('checks', [])
    required = {'ci-contract', 'powershell-syntax'} | {
        name for name, _, _ in validate_windows.plan(source, 'binary', 'pdfium', 'qpdf', 'service')}
    if report.get('status') != 'PASS' or report.get('lane') != 'hosted' \
            or report.get('source_commit') != commit or identity.get('source_commit') != commit \
            or identity.get('target') != 'x86_64-pc-windows-msvc' or identity.get('profile') != 'release' \
            or not checks or any(c.get('status') != 'PASS' for c in checks) \
            or not required.issubset({c.get('name') for c in checks}) \
            or len({c.get('name') for c in checks}) != len(checks):
        raise ValueError('Exact-source complete hosted validation is required; no rebuild')
    if identity.get('adapter_sha256') != artifact_cache.digest(source / 'build.ps1') \
            or identity.get('compiler_sha256') != hashlib.sha256(report['compiler'].encode()).hexdigest() \
            or identity.get('cargo_sha256') != hashlib.sha256(report['toolchain'].encode()).hexdigest():
        raise ValueError('Build provenance differs from the validated source/toolchain')
    expected = report.get('binary_sha256', '')
    if not re.fullmatch(r'[a-f0-9]{64}', expected):
        raise ValueError('Missing validated binary hash')
    candidates = [target / 'release/dpdf.exe', target / 'x86_64-pc-windows-msvc/release/dpdf.exe']
    matches = [p for p in candidates if p.is_file() and not p.is_symlink()
               and artifact_cache.digest(p) == expected]
    if len(matches) != 1:
        raise ValueError('Validated binary unavailable or changed in cache; no rebuild')
    binary = artifact_cache.store(artifacts, identity, matches[0])
    return binary, dict(identity=identity, binary_sha256=expected, artifact_reused=True,
                        scope='build_identity_only', hosted_validation_run=report.get('run_url'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=Path('.'))
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--acceptance-report', type=Path,
                        help='Private packaging requires the exact passed controlled native report')
    parser.add_argument('--reuse-report', type=Path, help='Passed hosted report for packaging-only recovery')
    parser.add_argument('--reuse-target', type=Path, help='Exact native target cache; never rebuilt in recovery')
    args = parser.parse_args()
    if os.name != 'nt':
        raise RuntimeError('Native Windows is required; no hosted fallback')
    source = args.source.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    commit = validate_windows.git(source, 'rev-parse', 'HEAD')
    if validate_windows.git(source, 'status', '--porcelain', '--untracked-files=all'):
        raise RuntimeError('Build source must be clean')
    if args.reuse_report or args.reuse_target:
        if not (args.reuse_report and args.reuse_target) or args.acceptance_report:
            parser.error('Hosted recovery requires report and cache; it is not controlled native acceptance')
        artifacts = Path(os.environ.get('RUNNER_TEMP') or tempfile.gettempdir()) / 'native-artifacts'
        binary, record = reuse_hosted(source, commit, json.loads(args.reuse_report.read_text()),
                                      args.reuse_target.resolve(), artifacts)
        (output / 'build-provenance.json').write_text(json.dumps(record, indent=2) + '\n')
        (output / 'artifact-path.txt').write_text(str(binary) + '\n')
        if os.environ.get('GITHUB_ENV'):
            with Path(os.environ['GITHUB_ENV']).open('a', encoding='utf-8') as stream:
                stream.write(f'DPDF_VERIFIED_BINARY={binary}\n')
        print(json.dumps(record, indent=2))
        return 0
    cache = Path(os.environ.get('DPDF_CI_ROOT') or os.environ.get('RUNNER_TEMP') or tempfile.gettempdir())
    target = cache / 'dpdf-msvc-target'
    artifacts = Path(os.environ.get('DPDF_NATIVE_ARTIFACT_ROOT') or cache)
    cargo = shutil.which('cargo.exe')
    if not cargo:
        raise RuntimeError('Native Cargo is unavailable')
    validate_windows.admission()
    with tempfile.TemporaryDirectory(prefix='dpdf-native-build-') as directory:
        env = validate_windows.environment(Path(directory), target)
        versions = {}
        for name, command in [('compiler', ['rustc.exe', '-vV']), ('cargo', [cargo, '-vV'])]:
            log = output / (name + '.log')
            if validate_windows.run(command, source, env, log, 120):
                raise RuntimeError('Native toolchain probe failed')
            versions[name] = log.read_text(encoding='utf-8', errors='replace')
            if 'host: x86_64-pc-windows-msvc' not in versions[name]:
                raise RuntimeError('Native x86_64 MSVC is required')
        adapter, adapter_args = artifact_cache.native_adapter(source, Path(__file__).resolve().parents[2])
        identity = artifact_cache.identity(commit, versions['compiler'], versions['cargo'], env,
                                           artifact_cache.digest(adapter))
        binary = artifact_cache.load(artifacts, identity)
        reused = binary is not None
        if binary is None:
            if args.acceptance_report:
                raise RuntimeError('PENDING: the accepted immutable artifact is unavailable; do not rebuild a substitute')
            manifest = Path(directory) / 'cargo-artifacts.jsonl'
            command = ['powershell.exe', '-NoProfile', '-NonInteractive', '-ExecutionPolicy',
                       'Bypass', '-File', str(adapter), '-Release', *adapter_args,
                       '-ArtifactManifest', str(manifest)]
            if validate_windows.run(command, source, env, output / 'build.log', 2700):
                raise RuntimeError('Native build failed')
            if validate_windows.git(source, 'rev-parse', 'HEAD') != commit or validate_windows.git(
                    source, 'status', '--porcelain', '--untracked-files=all'):
                raise RuntimeError('Source changed during build; no receipt recorded')
            if artifact_cache.identity(commit, versions['compiler'], versions['cargo'], env,
                                       artifact_cache.digest(adapter)) != identity:
                raise RuntimeError('Build configuration/adapter changed; no receipt recorded')
            produced = artifact_cache.built_binary(manifest, target)
            if validate_windows.run([os.sys.executable, str(source / 'scripts/release-version.py'),
                                     'verify', '--binary', str(produced)],
                                    source, env, output / 'binary-version.log', 60):
                raise RuntimeError('Built binary version verification failed')
            binary = artifact_cache.store(artifacts, identity, produced)
        if validate_windows.git(source, 'rev-parse', 'HEAD') != commit or validate_windows.git(
                source, 'status', '--porcelain', '--untracked-files=all'):
            raise RuntimeError('Source changed during build')
        record = dict(identity=identity, binary_sha256=artifact_cache.digest(binary),
                      artifact_reused=reused, scope='build_identity_only')
        if args.acceptance_report:
            accepted = json.loads(args.acceptance_report.read_text())
            mode = 'desktop' if accepted.get('lane') == 'desktop' else 'service'
            required = {'ci-contract', 'powershell-syntax'} | {name for name, _, _ in
                validate_windows.plan(source, binary, 'pdfium', 'qpdf', mode)}
            artifact_cache.validate_acceptance(accepted, commit, record['binary_sha256'], required)
            record['native_acceptance'] = dict(status='PASS', source_commit=commit,
                binary_sha256=record['binary_sha256'], checks=len(accepted['checks']), lane=accepted['lane'])
        (output / 'build-provenance.json').write_text(json.dumps(record, indent=2) + '\n')
        # This path is local operator state, deliberately excluded from assets.
        (output / 'artifact-path.txt').write_text(str(binary) + '\n')
        if os.environ.get('GITHUB_ENV'):
            with Path(os.environ['GITHUB_ENV']).open('a', encoding='utf-8') as stream:
                stream.write(f'DPDF_VERIFIED_BINARY={binary}\n')
        print(json.dumps(record, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
