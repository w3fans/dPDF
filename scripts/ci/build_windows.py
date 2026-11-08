"""Build once under the existing Windows limits, or reuse its exact artifact."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

import artifact_cache
import validate_windows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=Path('.'))
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--acceptance-report', type=Path,
                        help='Private packaging requires the exact passed controlled native report')
    args = parser.parse_args()
    if os.name != 'nt':
        raise RuntimeError('Native Windows is required; no hosted fallback')
    source = args.source.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    commit = validate_windows.git(source, 'rev-parse', 'HEAD')
    if validate_windows.git(source, 'status', '--porcelain', '--untracked-files=all'):
        raise RuntimeError('Build source must be clean')
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
