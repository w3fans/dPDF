"""Immutable native build artifacts; identity reuse never implies test acceptance."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import tempfile


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def identity(commit, compiler, cargo, env, adapter_sha256=None):
    if not re.fullmatch(r'[0-9a-f]{40}', commit):
        raise ValueError('An exact source commit is required')
    flags = {key: value for key, value in env.items()
             if key.startswith('CARGO_PROFILE_RELEASE_') or
             (key.startswith('CARGO_TARGET_') and key != 'CARGO_TARGET_DIR') or key in
             ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC', 'RUSTC_WRAPPER',
              'RUSTC_WORKSPACE_WRAPPER', 'CARGO_BUILD_TARGET')}
    home = Path(env.get('CARGO_HOME') or
                Path(env.get('USERPROFILE') or env.get('HOME') or Path.home()) / '.cargo')
    flags['global_config_digests'] = {name: digest(home / name) if (home / name).is_file() else None
                                     for name in ('config', 'config.toml')}
    return dict(schema=1, source_commit=commit, target='x86_64-pc-windows-msvc', adapter_sha256=adapter_sha256,
                profile='release', compiler_sha256=hashlib.sha256(compiler.encode()).hexdigest(),
                cargo_sha256=hashlib.sha256(cargo.encode()).hexdigest(),
                flags_sha256=hashlib.sha256(json.dumps(flags, sort_keys=True).encode()).hexdigest())


def folder(cache, expected):
    fingerprint = hashlib.sha256(json.dumps(expected, sort_keys=True).encode()).hexdigest()
    root = Path(cache).resolve() / 'native-artifacts' / expected['source_commit'] / fingerprint
    if root.resolve() != root:
        raise ValueError('Invalid immutable artifact path')
    return root


def built_binary(manifest, target):
    """Use Cargo's actual executable output, never a stale guessed target path."""
    path = Path(manifest)
    with path.open('rb') as stream:
        encoding = 'utf-16' if stream.read(2) in (b'\xff\xfe', b'\xfe\xff') else 'utf-8-sig'
    found = set()
    with path.open(encoding=encoding) as stream:
        for line in stream:
            if not line.lstrip().startswith('{'):
                continue
            event = json.loads(line)
            if event.get('reason') != 'compiler-artifact' or not event.get('executable'):
                continue
            if event.get('target', {}).get('name') == 'dpdf' and 'bin' in event['target'].get('kind', []) and not event.get('profile', {}).get('test'):
                found.add(Path(event['executable']).resolve())
    if len(found) != 1:
        raise ValueError('Cargo must identify exactly one dPDF executable')
    binary = found.pop()
    target = Path(target).resolve()
    if binary.parent not in (target / 'release', target / 'x86_64-pc-windows-msvc/release') or binary.suffix != '.exe' or not binary.is_file():
        raise ValueError('Cargo output is not the expected native release artifact')
    return binary


def native_adapter(source, harness):
    source = Path(source).resolve()
    wrapper = source / 'build.ps1'
    if 'ArtifactManifest' in wrapper.read_text(encoding='utf-8'):
        return wrapper, []
    # Older immutable tags lack manifest capture; use the maintained thin Cargo
    # adapter against that checkout without modifying it or guessing an output.
    return Path(harness).resolve() / 'build.ps1', ['-SourceDirectory', str(source)]


def validate_acceptance(report, commit, binary_sha256, required):
    if report.get('status') != 'PASS' or report.get('source_commit') != commit or report.get('binary_sha256') != binary_sha256:
        raise ValueError('Native acceptance identity/result mismatch')
    if report.get('lane') not in ('self-hosted', 'desktop'):
        raise ValueError('Controlled native acceptance is required')
    checks = report.get('checks', [])
    if not checks or any(check.get('status') != 'PASS' for check in checks) or not required.issubset({check.get('name') for check in checks}):
        raise ValueError('Native acceptance is incomplete')


def load(cache, expected):
    root = folder(cache, expected)
    receipt = root / 'artifact.json'
    if not receipt.exists():
        return None
    if receipt.is_symlink():
        raise ValueError('Artifact receipt must not be a symlink')
    record = json.loads(receipt.read_text())
    if record.get('identity') != expected or not re.fullmatch(r'[0-9a-f]{64}', record.get('binary_sha256', '')):
        raise ValueError('Artifact identity mismatch')
    binary = root / record['binary_sha256'] / 'dpdf.exe'
    if root.is_symlink() or binary.parent.is_symlink() or binary.is_symlink() or not binary.is_file():
        raise ValueError('Invalid immutable artifact path')
    if digest(binary) != record['binary_sha256']:
        raise ValueError('Cached binary hash mismatch; no artifact reused')
    return binary


def store(cache, expected, binary):
    root = folder(cache, expected)
    checksum = digest(binary)
    destination = root / checksum / 'dpdf.exe'
    destination.parent.mkdir(parents=True, exist_ok=True)
    if root.is_symlink() or destination.parent.is_symlink() or destination.is_symlink():
        raise ValueError('Invalid immutable artifact path')
    if destination.exists():
        if digest(destination) != checksum:
            raise ValueError('Existing immutable artifact differs')
    else:
        shutil.copyfile(binary, destination)
    if digest(destination) != checksum or digest(binary) != checksum:
        raise ValueError('Binary changed while recording provenance')
    record = dict(identity=expected, binary_sha256=checksum, scope='build_identity_only')
    with tempfile.NamedTemporaryFile(mode='w', dir=root, delete=False) as temporary:
        json.dump(record, temporary, indent=2)
        temporary.write('\n')
    os.replace(temporary.name, root / 'artifact.json')
    return destination
