#!/usr/bin/env python3
"""Collect immutable upstream Git source snapshots, without binary toolchain hooks."""
import argparse
import gzip
import hashlib
from file_digest import sha256_file
import json
from pathlib import Path, PurePosixPath
import re
import shutil
import tarfile
import tempfile
import time
import urllib.request
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parent.parent


def snapshot_url(component):
    url, revision = component['url'], component['revision']
    if urlsplit(url).hostname not in ('pdfium.googlesource.com', 'chromium.googlesource.com', 'gn.googlesource.com') \
            or not re.fullmatch('[a-f0-9]{40}', revision):
        raise ValueError('Require an immutable upstream source revision')
    path = PurePosixPath(component['path'])
    if path.is_absolute() or '..' in path.parts:
        raise ValueError('Source component escapes the source tree')
    return url + '/+archive/' + revision + '.tar.gz'


def validate_manifest(manifest):
    components = manifest.get('components')
    if (manifest.get('chromium_branch') != 8125 or manifest.get('binary_downloads') is not False
            or manifest.get('checkout_configuration') != 'minimal'
            or not isinstance(components, list) or not components
            or components[0] != {
                'path': '.', 'url': 'https://pdfium.googlesource.com/pdfium',
                'revision': '43d55e429cf72c21e9a1e60475b50feefa16fda9'}):
        raise ValueError('Require the reviewed source-only PDFium 8125 manifest and root revision')
    # Reject malformed immutable component identities before any download.
    for component in components:
        snapshot_url(component)
    exclusions = manifest.get('excluded_precompiled_test_fixtures', [])
    if not isinstance(exclusions, list):
        raise ValueError('Require a list of excluded test fixtures')
    for name in exclusions:
        if not isinstance(name, str):
            raise ValueError('Require relative test fixture paths')
        path = PurePosixPath(name)
        if (path.is_absolute() or '..' in path.parts
                or path.as_posix() in ('', '.') or '\\' in name):
            raise ValueError('Excluded test fixture escapes the source tree')


def collect(manifest_path, output, cache):
    manifest = json.loads(manifest_path.read_text())
    validate_manifest(manifest)
    cache.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='pdfium-sources-', dir=cache) as directory:
        tree = Path(directory) / 'pdfium-source'
        tree.mkdir()
        for component in manifest['components']:
            url = snapshot_url(component)
            snapshot = cache / (hashlib.sha256(url.encode()).hexdigest() + '.tar.gz')
            if not snapshot.exists():
                partial = snapshot.with_suffix('.partial')
                for attempt in range(3):
                    try:
                        with urllib.request.urlopen(url, timeout=180) as response, partial.open('wb') as target:
                            shutil.copyfileobj(response, target)
                        with tarfile.open(partial) as archive:
                            archive.getmembers()
                        partial.replace(snapshot)
                        break
                    except Exception:
                        partial.unlink(missing_ok=True)
                        if attempt == 2:
                            raise
                        time.sleep(3)
            component['archive_sha256'] = sha256_file(snapshot)
            destination = tree / component['path']
            destination.mkdir(parents=True, exist_ok=True)
            with tarfile.open(snapshot) as archive:
                archive.extractall(destination, filter='data')
            print('SOURCE:', component['path'], component['revision'], flush=True)
        excluded = []
        for name in manifest.get('excluded_precompiled_test_fixtures', []):
            path = tree / name
            if not path.resolve().is_relative_to(tree.resolve()):
                raise ValueError('Excluded test fixture escapes source tree')
            if path.is_file():
                excluded.append({'path': name, 'sha256': sha256_file(path), 'bytes': path.stat().st_size})
            path.unlink(missing_ok=True)
        manifest['excluded_test_fixture_receipt'] = excluded
        (tree / 'SOURCE-MANIFEST.json').write_text(json.dumps(manifest, indent=2) + '\n')
        # Retain every upstream notice with its component-relative location.
        notices = tree / 'dpdf-source-notices'
        for path in list(tree.rglob('*')):
            if path.is_file() and (path.name.lower().startswith(('license', 'copying', 'notice'))
                                   or path.name == 'README.chromium'):
                target = notices / path.relative_to(tree)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, target)
        output.parent.mkdir(parents=True, exist_ok=True)

        def normalize(item):
            item.uid = item.gid = item.mtime = 0
            item.uname = item.gname = ''
            return item

        with output.open('wb') as raw, gzip.GzipFile(filename='', fileobj=raw, mode='wb', mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode='w|') as archive:
                archive.add(tree, arcname='pdfium-source', filter=normalize)
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--cache', type=Path, required=True)
    args = parser.parse_args()
    print(collect(ROOT / 'packaging/fedora/pdfium-sources.json', args.output.resolve(), args.cache.resolve()))


if __name__ == '__main__':
    main()
