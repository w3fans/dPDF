#!/usr/bin/env python3
"""Assemble an SRPM from one committed product tree; never compile on the source host."""
import argparse
import hashlib
import io
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
from file_digest import sha256_file
ROOT = Path(__file__).resolve().parent.parent


def version_from_manifest(manifest):
    version = tomllib.loads(manifest)['package']['version']
    if not re.fullmatch(r'0\.1\.\d{1,2}', version):
        raise ValueError('Package development previews only; stable milestones need acceptance')
    return version


def render_spec(template, version, release='1'):
    if template.count('@VERSION@') != 1:
        raise ValueError('RPM spec must have exactly one version placeholder')
    # The source release controller already assigned this version; never bump it.
    if not re.fullmatch(r'(?:1|0\.\d+\.git[a-f0-9]{12})', release):
        raise ValueError('Invalid RPM release identity')
    return template.replace('@VERSION@', version).replace('@RELEASE@', release)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--outdir', type=Path, required=True)
    parser.add_argument('--ref', default='HEAD', help='Committed source ref; defaults to checked-out HEAD')
    parser.add_argument('--cargo-home', type=Path, help='Optional isolated Cargo download cache')
    parser.add_argument('--pdfium-sources', type=Path, help='Reuse a verified source-only PDFium archive')
    parser.add_argument('--source-cache', type=Path, help='Optional checksum-verified vendor archive cache')
    args = parser.parse_args()
    ref = subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', '--verify',
                                   args.ref + '^{commit}'], text=True).strip()
    manifest = subprocess.check_output(['git', '-C', str(ROOT), 'show', ref + ':Cargo.toml'], text=True)
    version = version_from_manifest(manifest)
    tags = subprocess.check_output(['git', '-C', str(ROOT), 'tag', '--points-at', ref], text=True).splitlines()
    epoch = subprocess.check_output(['git', '-C', str(ROOT), 'show', '-s', '--format=%ct', ref], text=True).strip()
    release = '1' if 'v' + version in tags else '0.' + epoch + '.git' + ref[:12]
    outdir = args.outdir.resolve()
    outdir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='dpdf-srpm-') as directory:
        work = Path(directory)
        rpmroot = work / 'rpmbuild'
        for name in ('SOURCES', 'SPECS', 'SRPMS', 'BUILD', 'BUILDROOT', 'RPMS'):
            (rpmroot / name).mkdir(parents=True)
        sources = rpmroot / 'SOURCES'
        source = sources / f'dpdf-{version}.tar.gz'
        plain = work / 'source.tar'
        subprocess.run(['git', '-C', str(ROOT), 'archive', '--format=tar',
                        '--prefix=dpdf-' + version + '/', '--output=' + str(plain), ref], check=True)
        with tarfile.open(plain, 'a') as archive:
            revision = (ref + '\n').encode()
            item = tarfile.TarInfo('dpdf-' + version + '/SOURCE-REVISION')
            item.size, item.mode = len(revision), 0o644
            archive.addfile(item, io.BytesIO(revision))
        import gzip
        with plain.open('rb') as original, source.open('wb') as raw, \
                gzip.GzipFile(filename='', fileobj=raw, mode='wb', mtime=0) as compressed:
            shutil.copyfileobj(original, compressed)
        with tarfile.open(source) as archive:
            archive.extractall(work, filter='data')
        tree = work / ('dpdf-' + version)
        spec = rpmroot / 'SPECS' / 'dpdf.spec'
        spec.write_text(render_spec((tree / 'packaging/fedora/dpdf.spec').read_text(), version, release))
        import json
        lock_digest = hashlib.sha256((tree / 'Cargo.lock').read_bytes()).hexdigest()
        cache = args.source_cache.resolve() if args.source_cache else None
        cached = cache / (lock_digest + '.tar.gz') if cache else None
        cached_info = cache / (lock_digest + '.json') if cache else None
        use_cache = cached is not None and cached.exists() and cached_info.exists()
        vendor_archive = sources / f'dpdf-{version}-vendor.tar.gz'
        if use_cache:
            info = json.loads(cached_info.read_text())
            if info['lock_sha256'] != lock_digest or info['archive_sha256'] != sha256_file(cached):
                raise ValueError('Vendor source-cache checksum mismatch')
            shutil.copy2(cached, vendor_archive)
        else:
            env = dict(os.environ, CARGO_HOME=str(args.cargo_home.resolve() if args.cargo_home else work / 'cargo-home'))
            subprocess.run(['cargo', 'vendor', '--locked', '--versioned-dirs', str(tree / 'vendor')],
                           cwd=tree, env=env, check=True)
        def anonymous(item):
            item.uid = item.gid = item.mtime = 0
            item.uname = item.gname = ''
            return item
        if not use_cache:
            with vendor_archive.open('wb') as raw, \
                    gzip.GzipFile(filename='', fileobj=raw, mode='wb', mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode='w|') as archive:
                    archive.add(tree / 'vendor', arcname='vendor', filter=anonymous)
            if cache:
                cache.mkdir(parents=True, exist_ok=True)
                shutil.copy2(vendor_archive, cached)
                cached_info.write_text(json.dumps(dict(lock_sha256=lock_digest,
                        archive_sha256=sha256_file(cached))) + '\n')
        pdfium = sources / 'pdfium-8125-source.tar.gz'
        if args.pdfium_sources:
            with tarfile.open(args.pdfium_sources) as archive:
                import json
                recorded = json.load(archive.extractfile('pdfium-source/SOURCE-MANIFEST.json'))
                expected = json.loads((tree / 'packaging/fedora/pdfium-sources.json').read_text())
                for component in recorded['components']:
                    component.pop('archive_sha256', None)
                if recorded != expected:
                    raise ValueError('PDFium source archive differs from the committed source manifest')
            shutil.copy2(args.pdfium_sources, pdfium)
        else:
            subprocess.run(['python3', 'scripts/collect-pdfium-source.py', '--output', str(pdfium),
                            '--cache', str(work / 'pdfium-cache')], cwd=tree, check=True)
        subprocess.run(['rpmbuild', '-bs', '--define', '_topdir ' + str(rpmroot),
                        '--define', 'dist .fc44', '--define', '_buildhost dpdf-source-builder',
                        '--define', 'packager dPDF contributors', '--define', 'vendor dPDF', str(spec)], check=True)
        built = list((rpmroot / 'SRPMS').glob('*.src.rpm'))
        if len(built) != 1:
            raise ValueError('Expected exactly one dPDF source RPM')
        destination = outdir / built[0].name
        shutil.copy2(built[0], destination)
        print(destination)


if __name__ == '__main__':
    main()
