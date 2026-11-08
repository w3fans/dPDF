#!/usr/bin/env python3
"""Inventory cached Cargo license declarations/texts; not a distribution clearance.

Run beside the registry sources referenced by cargo metadata. Does not invoke Cargo,
fetch dependencies, select SPDX alternatives, or follow files outside package roots.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import tarfile
import tomllib
from urllib.parse import urlsplit, urlunsplit

PREFIXES = ('license', 'licence', 'copying', 'copyright', 'notice', 'unlicense', 'authors')


def public_url(value):
    if not value:
        return None
    parts = urlsplit(value)
    if parts.scheme not in ('https', 'http') or parts.username or parts.password:
        return None
    return urlunsplit((parts.scheme, parts.netloc, parts.path, '', ''))


def notice_files(root, declared):
    root = root.resolve()
    candidates = []
    if declared:
        candidates.append(root / declared)
    for path in root.iterdir():
        if path.name.lower().startswith(PREFIXES):
            candidates += list(path.rglob('*')) if path.is_dir() else [path]
    found = {}
    for path in candidates:
        if not path.is_file():
            continue
        if not path.resolve().is_relative_to(root):
            raise ValueError('License file escapes package root')
        if path.stat().st_size > 2 * 1024 ** 2:
            raise ValueError('License file exceeds 2 MiB')
        found[path.relative_to(root).as_posix()] = path
    return sorted(found.items())


def lock_fingerprint(lock, workspace_names):
    entries = [{key: package.get(key) for key in ('name', 'version', 'source', 'checksum', 'dependencies')}
               for package in lock['package']
               if package.get('source') is not None or package['name'] not in workspace_names]
    entries.sort(key=lambda item: (item['name'], item['version'], item['source'] or ''))
    return hashlib.sha256(json.dumps(entries, sort_keys=True).encode()).hexdigest()


def config_fingerprint(config):
    relevant = {key: config.get(key) for key in ('dependencies', 'build-dependencies', 'target', 'features')}
    return hashlib.sha256(json.dumps(relevant, sort_keys=True).encode()).hexdigest()


def collect(metadata, lock, target, output, selected_tree=None, overrides_path=None, config=None):
    if metadata.get('version') != 1 or not metadata.get('resolve'):
        raise ValueError('Require Cargo metadata --format-version 1 with dependencies')
    if not re.fullmatch('[a-zA-Z0-9_-]+', target):
        raise ValueError('Invalid target triple')
    output.mkdir(parents=True, exist_ok=False)
    texts = output / 'texts'
    texts.mkdir()
    nodes = {node['id']: node for node in metadata['resolve']['nodes']}
    own = set(metadata['workspace_members'])
    packages = [package for package in metadata['packages'] if package['id'] in nodes and package['id'] not in own]
    if selected_tree is not None:
        selected = set()
        for line in selected_tree.splitlines():
            if not line.strip() or line.startswith('[build-dependencies]'):
                continue
            match = re.match(r'^([A-Za-z0-9_-]+) v(\S+)(?:\s|$)', line)
            if not match:
                raise ValueError('Unexpected cargo tree output')
            selected.add((match[1], match[2]))
        if not selected:
            raise ValueError('Cargo tree selected no packages')
        known = {(package['name'], package['version']) for package in metadata['packages']}
        if selected - known:
            raise ValueError('Cargo tree and metadata package versions differ')
        packages = [package for package in packages if (package['name'], package['version']) in selected]
        identities = [(package['name'], package['version']) for package in packages]
        if len(set(identities)) != len(identities):
            raise ValueError('Ambiguous package source; review package identities manually')
    checksums = {(p['name'], p['version'], p.get('source')): p.get('checksum') for p in lock['package']}
    overrides = json.loads(overrides_path.read_text(encoding='utf-8'))['packages'] if overrides_path else {}
    components, missing, total = [], [], 0
    fingerprints = []
    for package in sorted(packages, key=lambda p: (p['name'], p['version'], p.get('source') or '')):
        root = Path(package['manifest_path']).parent.resolve()
        records = []
        for name, path in notice_files(root, package.get('license_file')):
            data = path.read_bytes()
            digest = hashlib.sha256(data).hexdigest()
            destination = texts / (digest + '.txt')
            if not destination.exists():
                total += len(data)
                if total > 32 * 1024 ** 2:
                    raise ValueError('Notice bundle exceeds 32 MiB')
                destination.write_bytes(data)
            records.append(dict(path=name, sha256=digest, text='texts/' + digest + '.txt', origin='crate-archive'))
        checksum = checksums.get((package['name'], package['version'], package.get('source')))
        override = overrides.get(package['name'] + '@' + package['version'])
        if override:
            if not checksum or override['registry_checksum'] != checksum:
                raise ValueError('Reviewed notice override crate checksum mismatch')
            vcs = root / '.cargo_vcs_info.json'
            if not vcs.resolve().is_relative_to(root):
                raise ValueError('VCS metadata escapes package root')
            revision = json.loads(vcs.read_text(encoding='utf-8'))['git']['sha1']
            if not re.fullmatch('[0-9a-f]{40}', revision) or override['source_revision'] != revision:
                raise ValueError('Reviewed notice override source revision mismatch')
            for notice in override['notices']:
                path = overrides_path.parent / notice['text']
                if not path.resolve().is_relative_to(overrides_path.parent.resolve()):
                    raise ValueError('Reviewed notice text escapes override directory')
                if path.stat().st_size > 2 * 1024 ** 2:
                    raise ValueError('Reviewed notice text exceeds 2 MiB')
                data = path.read_bytes()
                digest = hashlib.sha256(data).hexdigest()
                if notice['sha256'] != digest:
                    raise ValueError('Reviewed notice text checksum mismatch')
                url = public_url(notice['url'])
                if not url or revision not in url:
                    raise ValueError('Reviewed notice URL is not pinned to the source revision')
                destination = texts / (digest + '.txt')
                if not destination.exists():
                    total += len(data)
                    if total > 32 * 1024 ** 2:
                        raise ValueError('Notice bundle exceeds 32 MiB')
                    destination.write_bytes(data)
                records.append(dict(path=notice['path'], sha256=digest, text='texts/' + digest + '.txt',
                                    origin='upstream-exact-revision', url=url, source_revision=revision))
        item = dict(name=package['name'], version=package['version'],
                    license_declaration=package.get('license'),
                    repository=public_url(package.get('repository')), registry_checksum=checksum,
                    resolved_features=sorted(nodes[package['id']]['features']), notices=records)
        # Public open-source distribution selects Slint's GPL alternative.
        if 'LicenseRef-Slint-Royalty-free-2.0' in (package.get('license') or ''):
            item['application_license_choice'] = 'GPL-3.0-only'
        components.append(item)
        fingerprints.append({key: item[key] for key in ('name', 'version', 'registry_checksum', 'resolved_features')})
        if not records:
            missing.append(package['name'] + '@' + package['version'])
    fingerprint = hashlib.sha256(json.dumps(fingerprints, sort_keys=True).encode()).hexdigest()
    own_names = {p['name'] for p in metadata['packages'] if p['id'] in own}
    inventory = dict(schema=1, target=target, config_fingerprint=config_fingerprint(config) if config else None,
                     lock_fingerprint=lock_fingerprint(lock, own_names), scope=('Cargo target tree: normal/build/proc-macro dependencies' if selected_tree is not None
                            else 'Conservative Cargo resolved graph; may include unbuilt optional dependencies'),
                     audit_status='inventory_only', qualified_for_binary_distribution=False,
                     dependency_fingerprint=fingerprint, components=components,
                     missing_license_text=missing,
                     exclusions=['Rust standard library and native toolchain runtimes',
                                 'PDFium and its bundled dependencies', 'qpdf and system libraries',
                                 'vendored notices not captured by package license-file/root notice names',
                                 'manual license selection and distribution-compliance review'])
    (output / 'inventory.json').write_text(json.dumps(inventory, indent=2) + '\n', encoding='utf-8')
    lines = ['# Cargo dependency notices — ' + target, '',
             'Inventory only. This is not distribution clearance or a complete license audit.', '',
             'Declarations are retained exactly as supplied by Cargo. Except the existing',
             'Slint desktop license choice, alternative-license selection is not automated.',
             'All discovered notice texts are retained, including unselected alternatives.', '',
             '| Package | Version | Declaration | Notice files |', '| --- | --- | --- | ---: |']
    lines += [f"| {p['name']} | {p['version']} | {(p['license_declaration'] or 'UNDECLARED').replace('|', chr(92)+'|')} | {len(p['notices'])} |" for p in components]
    lines += ['', 'Missing standalone notice text: ' + (', '.join(missing) or 'none'), '',
              'See inventory.json for text hashes, original filenames and explicit scope limits.', '']
    (output / 'README.md').write_text('\n'.join(lines), encoding='utf-8')
    return inventory


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--metadata', type=Path, required=True)
    parser.add_argument('--manifest', type=Path, help='Defaults to Cargo.toml beside the lockfile')
    parser.add_argument('--lock', type=Path, required=True)
    parser.add_argument('--target', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--archive', type=Path)
    parser.add_argument('--overrides', type=Path, help='Reviewed exact-revision notice overrides and hash-verified local texts')
    parser.add_argument('--tree', type=Path, help='cargo tree --edges normal,build --prefix none --format {p} for the same target')
    args = parser.parse_args()
    result = collect(json.loads(args.metadata.read_text(encoding='utf-8')),
                     tomllib.loads(args.lock.read_text(encoding='utf-8')), args.target, args.output,
                     args.tree.read_text(encoding='utf-8') if args.tree else None, args.overrides,
                     tomllib.loads((args.manifest or args.lock.with_name('Cargo.toml')).read_text(encoding='utf-8')))
    if args.archive:
        with tarfile.open(args.archive, 'w:gz') as archive:
            for file in sorted(args.output.rglob('*')):
                if file.is_file():
                    archive.add(file, arcname=file.relative_to(args.output).as_posix())
    print(json.dumps(dict(target=result['target'], packages=len(result['components']),
                         missing_license_text=result['missing_license_text'],
                         qualified_for_binary_distribution=False)))


if __name__ == '__main__':
    main()
