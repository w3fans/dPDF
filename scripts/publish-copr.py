#!/usr/bin/env python3
"""Build an exact release in COPR; attach verified Fedora 44/45 RPMs."""
import argparse
from collections.abc import Mapping
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile
import time
import urllib.request
from urllib.parse import urlsplit
from file_digest import sha256_file

OWNER, PROJECT = 'zvegelj', 'dpdf'
CHROOTS = ('fedora-44-x86_64', 'fedora-45-x86_64')
REPOSITORY = 'https://github.com/w3fans/dPDF.git'
ROOT = Path(__file__).resolve().parent.parent


def save_receipt(path, value):
    with tempfile.NamedTemporaryFile(mode='w', dir=path.parent, delete=False) as stream:
        json.dump(value, stream, indent=2)
        stream.write('\n')
        stream.flush()
        os.fsync(stream.fileno())
    try:
        os.replace(stream.name, path)
    finally:
        Path(stream.name).unlink(missing_ok=True)


def begin_submission(output, tag, commit, source, chroots):
    path = output / 'submission-intent.json'
    record = dict(tag=tag, source_commit=commit, srpm_sha256=sha256_file(source), chroots=chroots)
    try:
        with path.open('x') as stream:
            json.dump(record, stream, indent=2)
            stream.write('\n')
            stream.flush()
            os.fsync(stream.fileno())
    except FileExistsError as error:
        raise ValueError('Previous submission has no durable build receipt; identify its original COPR build and resume with --build-id, never blindly submit again') from error


def release_identity(tag):
    if not re.fullmatch(r'v0\.1\.\d{1,2}', tag):
        raise ValueError('Publish 0.1.x preview tags only; stable milestones need acceptance')
    commit = subprocess.check_output(['git', 'rev-parse', '--verify', tag + '^{commit}'],
                                     cwd=ROOT, text=True).strip()
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if head != commit:
        raise ValueError('Check out the exact release tag before publishing')
    subprocess.run(['python3', 'scripts/release-version.py', 'verify', '--tag', tag], cwd=ROOT, check=True)
    return commit


def check_rpm_identity(identity, version, chroot=CHROOTS[0]):
    if chroot not in CHROOTS:
        raise ValueError('Unsupported Fedora release target')
    parts = identity.strip().split('|')
    if len(parts) != 4 or parts[0] != 'dpdf' or parts[1] != version \
            or not re.fullmatch(r'(?:\d+|0\.\d+\.git[a-f0-9]{12})\.fc' + chroot.split('-')[1], parts[2]) or parts[3] not in ('x86_64', 'src'):
        raise ValueError('Unexpected dPDF RPM identity for ' + chroot)
    return parts[3]


def source_revision(path, version):
    """Read the single regular revision marker without extracting source paths."""
    expected = f'dpdf-{version}/SOURCE-REVISION'
    revisions = []
    with tarfile.open(path, 'r:gz') as archive:
        for member in archive:
            if member.name != expected:
                continue
            if not member.isfile() or member.size != 41:
                raise ValueError('Invalid source archive revision marker')
            value = archive.extractfile(member).read(42)
            if not re.fullmatch(rb'[a-f0-9]{40}\n', value):
                raise ValueError('Invalid source archive revision marker')
            revisions.append(value.decode('ascii').strip())
    if len(revisions) != 1:
        raise ValueError('Expected one source archive revision marker')
    return revisions[0]


def check_rpm(path, version, commit, chroot=CHROOTS[0]):
    if chroot not in CHROOTS:
        raise ValueError('Unsupported Fedora release target')
    identity = subprocess.check_output(['rpm', '-qp', '--queryformat',
                  '%{NAME}|%{VERSION}|%{RELEASE}|%{ARCH}', str(path)], text=True)
    source_package = subprocess.check_output(['rpm', '-qp', '--queryformat', '%{SOURCEPACKAGE}',
                                               str(path)], text=True).strip()
    # The uploaded SRPM is shared; target downloads can also include COPR's
    # rebuilt fc44/fc45 source RPMs. Verify their source marker independently.
    source_chroot = chroot
    if source_package == '1':
        parts = identity.strip().split('|')
        distribution = parts[2].rsplit('.', 1)[-1] if len(parts) == 4 else ''
        source_chroot = {'fc44': CHROOTS[0], 'fc45': CHROOTS[1]}.get(distribution)
        if source_chroot is None:
            raise ValueError('Unexpected source RPM distribution')
    arch = check_rpm_identity(identity, version, source_chroot)
    if source_package == '1':
        if not path.name.endswith('.src.rpm'):
            raise ValueError('Source package has an unexpected filename')
        with tempfile.TemporaryDirectory(prefix='dpdf-srpm-identity-') as folder:
            name = f'dpdf-{version}.tar.gz'
            unpack = subprocess.Popen(['rpm2cpio', str(path)], stdout=subprocess.PIPE)
            try:
                subprocess.run(['cpio', '-id', '--quiet', '--no-absolute-filenames',
                                name, './' + name],
                               cwd=folder, stdin=unpack.stdout, check=True)
            finally:
                unpack.stdout.close()
                code = unpack.wait()
            if code or source_revision(Path(folder) / name, version) != commit:
                raise ValueError('Source RPM was built from a different source commit')
        return 'src'
    if arch != 'x86_64':
        raise ValueError('Source package is missing its source-package marker')
    if arch == 'x86_64':
        with tempfile.TemporaryDirectory(prefix='dpdf-rpm-identity-') as folder:
            # Extract one fixed, relative product file; never install the RPM or
            # execute its scripts to inspect source provenance.
            unpack = subprocess.Popen(['rpm2cpio', str(path)], stdout=subprocess.PIPE)
            try:
                subprocess.run(['cpio', '-id', '--quiet', '--no-absolute-filenames',
                                './usr/share/dpdf/SOURCE-REVISION'],
                               cwd=folder, stdin=unpack.stdout, check=True)
            finally:
                unpack.stdout.close()
                code = unpack.wait()
            if code or (Path(folder) / 'usr/share/dpdf/SOURCE-REVISION').read_text().strip() != commit:
                raise ValueError('RPM was built from a different source commit')
    return arch


def source_package_url(build):
    # The installed COPR SDK leaves nested source_package metadata as a dict,
    # even though the outer build supports attribute access.
    package = build.get('source_package') if isinstance(build, Mapping) else None
    source_url = package.get('url') if isinstance(package, Mapping) else None
    if not isinstance(source_url, str):
        raise ValueError('COPR build is missing its source-package URL')
    location = urlsplit(source_url)
    if location.scheme != 'https' or location.hostname != 'download.copr.fedorainfracloud.org':
        raise ValueError('Unexpected COPR source-package download location')
    if not location.path.endswith('.src.rpm'):
        raise ValueError('COPR source-package URL must identify a source RPM')
    return source_url


def verified_packages(root, version, commit, chroots):
    if not chroots or len(set(chroots)) != len(chroots) or any(chroot not in CHROOTS for chroot in chroots):
        raise ValueError('Choose distinct supported Fedora release targets')
    packages = sorted(root.rglob('*.rpm'))
    sources = [path for path in packages if path.name.endswith('.src.rpm')]
    if not sources:
        raise ValueError('Expected corresponding dPDF source RPMs')
    source_scopes = set()
    for path in sources:
        matches = [chroot for chroot in chroots if chroot in path.relative_to(root).parts[:-1]]
        if len(matches) > 1 or (not matches and path.parent != root):
            raise ValueError('Unexpected source RPM target directory')
        scope = matches[0] if matches else 'shared'
        if scope in source_scopes or (scope == 'shared' and len(sources) != 1):
            raise ValueError('Ambiguous duplicate corresponding source RPMs')
        source_scopes.add(scope)
        if check_rpm(path, version, commit, matches[0] if matches else CHROOTS[0]) != 'src':
            raise ValueError('Invalid corresponding source RPM')
    counts = dict.fromkeys(chroots, 0)
    for path in packages:
        if path in sources:
            continue
        # copr-cli keeps each target in its own directory. Validate the RPM
        # against that requested target, not against its own claimed platform.
        matches = [chroot for chroot in chroots if chroot in path.relative_to(root).parts[:-1]]
        if len(matches) != 1 or check_rpm(path, version, commit, matches[0]) != 'x86_64':
            raise ValueError('Unexpected binary RPM or target directory')
        counts[matches[0]] += 1
    if any(count != 1 for count in counts.values()):
        raise ValueError('Expected exactly one dPDF binary RPM for each requested target')
    return packages


def github_api(endpoint):
    result = subprocess.run(['gh', 'api', '--hostname', 'github.com', endpoint],
                            capture_output=True, text=True, timeout=60)
    if result.returncode:
        raise RuntimeError('GitHub Release API access unavailable; preserve artifacts and retry')
    return json.loads(result.stdout)


def github_upload(tag, path):
    # Never clobber: another publisher may have won after the preflight read.
    result = subprocess.run(['gh', 'release', 'upload', tag, '--repo', 'w3fans/dPDF', str(path)],
                            capture_output=True, text=True, timeout=600)
    if result.returncode:
        raise RuntimeError('GitHub asset upload did not confirm success; read back before retry')


def remote_release_commit(api, tag):
    item = api(f'repos/w3fans/dPDF/git/ref/tags/{tag}')['object']
    for _ in range(8):
        if item['type'] == 'commit' and re.fullmatch(r'[a-f0-9]{40}', item['sha']):
            return item['sha']
        if item['type'] != 'tag' or not re.fullmatch(r'[a-f0-9]{40}', item['sha']):
            break
        item = api('repos/w3fans/dPDF/git/tags/' + item['sha'])['object']
    raise ValueError('Remote release tag does not resolve to a commit')


def attach_verified_assets(tag, commit, paths, api=None, upload=None):
    """Upload once, preserve conflicts, and require server hash/size readback."""
    api = api or github_api
    upload = upload or github_upload
    if not re.fullmatch(r'v0\.1\.\d{1,2}', tag) or not re.fullmatch(r'[a-f0-9]{40}', commit):
        raise ValueError('Exact preview tag/source required for attachment')
    expected = {}
    for path in map(Path, paths):
        if path.is_symlink() or not path.is_file() or path.name in expected:
            raise ValueError('Assets must be unique regular files')
        expected[path.name] = dict(path=path, size=path.stat().st_size,
                                   digest='sha256:' + sha256_file(path))
    if not expected:
        raise ValueError('No verified assets supplied')
    if remote_release_commit(api, tag) != commit:
        raise ValueError('Remote release tag differs from verified source')
    release = api(f'repos/w3fans/dPDF/releases/tags/{tag}')
    if release.get('tag_name') != tag or release.get('draft') is not False \
            or release.get('prerelease') is not True \
            or release.get('html_url') != 'https://github.com/w3fans/dPDF/releases/tag/' + tag \
            or type(release.get('id')) is not int or release['id'] <= 0:
        raise ValueError('A published development preview page is required')

    def assets():
        found = {}
        for page in range(1, 101):
            rows = api(f"repos/w3fans/dPDF/releases/{release['id']}/assets?per_page=100&page={page}")
            for asset in rows:
                if asset['name'] in found:
                    raise ValueError('Duplicate remote asset names require review')
                found[asset['name']] = asset
            if len(rows) < 100:
                return found
        raise ValueError('Remote asset inventory exceeds bounded review limit')

    def matches(asset, local):
        return asset.get('state') == 'uploaded' and asset.get('size') == local['size'] \
            and asset.get('digest') == local['digest']

    remote = assets()
    # Review every collision before making any write, including later filenames.
    for name, local in expected.items():
        if name in remote and not matches(remote[name], local):
            raise ValueError('Existing asset ' + name + ' differs or lacks verified digest; nothing overwritten')
    for name, local in expected.items():
        if name in remote:
            continue
        if sha256_file(local['path']) != local['digest'][7:] \
                or local['path'].stat().st_size != local['size']:
            raise ValueError('Local asset changed after preflight')
        try:
            upload(tag, local['path'])
        except RuntimeError:
            # A failed response can follow a successful upload. Read the server;
            # never overwrite or blindly upload a duplicate to resolve ambiguity.
            remote = assets()
            if name not in remote or not matches(remote[name], local):
                raise
        remote = assets()
        if name not in remote or not matches(remote[name], local):
            raise ValueError('Uploaded asset hash/size readback differs')
    if remote_release_commit(api, tag) != commit:
        raise ValueError('Remote release tag changed during attachment')
    remote = assets()
    if any(name not in remote or not matches(remote[name], local) for name, local in expected.items()):
        raise ValueError('Final asset verification failed')
    return dict(tag=tag, source_commit=commit, status='VERIFIED', release_url=release['html_url'],
                assets=[dict(name=name, size=local['size'], sha256=local['digest'][7:],
                             id=remote[name]['id'], url=remote[name]['browser_download_url'])
                        for name, local in expected.items()])


def main():
    global ROOT
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--build-id', type=int, help='Resume an existing COPR build')
    parser.add_argument('--pdfium-sources', type=Path, help='Verified source-only engine archive cache')
    parser.add_argument('--source-cache', type=Path, help='Verified Cargo vendor archive cache')
    parser.add_argument('--cargo-home', type=Path, help='Operator Cargo download cache')
    parser.add_argument('--chroot', action='append', choices=CHROOTS,
                        help='Select targets explicitly; default builds both Fedora 44 and 45')
    parser.add_argument('--attach', action='store_true', help='Attach assets to the existing GitHub preview')
    parser.add_argument('--attach-only', action='store_true',
                        help='Verify and attach an existing successful build; never build or download')
    parser.add_argument('--source', type=Path,
                        help='Exact immutable tag checkout for attachment-only recovery')
    parser.add_argument('--dry-run', action='store_true',
                        help='Verify attachment-only inputs and write a plan without GitHub access')
    args = parser.parse_args()
    if args.dry_run and not args.attach_only:
        parser.error('--dry-run requires --attach-only')
    if args.source:
        if not args.attach_only:
            parser.error('--source is only supported for --attach-only recovery')
        ROOT = args.source.resolve()
    chroots = list(dict.fromkeys(args.chroot or CHROOTS))
    commit = release_identity(args.tag)
    from copr.v3 import Client
    login, token = os.environ.get('COPR_LOGIN'), os.environ.get('COPR_TOKEN')
    if login or token:
        if not login or not token:
            raise ValueError('Both COPR_LOGIN and COPR_TOKEN are required')
        client = Client(dict(copr_url='https://copr.fedorainfracloud.org',
                             username=OWNER, login=login, token=token))
    else:
        client = Client.create_from_config_file()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    receipt_path = output / 'copr-release.json'
    if args.attach_only:
        if any((args.pdfium_sources, args.source_cache, args.cargo_home)):
            raise ValueError('Attachment-only recovery never assembles source or builds')
        if subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=all'],
                                   cwd=ROOT, text=True).strip():
            raise ValueError('Exact attachment source checkout must be clean')
        previous = json.loads(receipt_path.read_text())
        if previous.get('tag') != args.tag or previous.get('source_commit') != commit \
                or previous.get('project') != f'{OWNER}/{PROJECT}' \
                or previous.get('status') != 'succeeded' \
                or sorted(previous.get('verified_chroots', [])) != sorted(chroots) \
                or (args.build_id is not None and args.build_id != previous.get('build_id')):
            raise ValueError('A verified existing build receipt for the exact source/targets is required')
        build = client.build_proxy.get(previous['build_id'])
        if build.state != 'succeeded' or build.ownername != OWNER or build.projectname != PROJECT:
            raise ValueError('Existing COPR build identity/state differs')
        for chroot in chroots:
            if client.build_chroot_proxy.get(build.id, chroot).state != 'succeeded':
                raise ValueError('Existing target did not succeed')
        packages = verified_packages(output / 'packages', args.tag[1:], commit, chroots)
        checksums = output / 'SHA256SUMS-fedora.txt'
        expected = ''.join(sha256_file(p) + '  ' + p.name + '\n' for p in packages)
        if checksums.read_text() != expected:
            raise ValueError('Existing artifact checksums differ; no assets attached')
        if args.dry_run:
            record = dict(tag=args.tag, source_commit=commit, status='READY_NO_UPLOAD',
                          build_id=previous['build_id'],
                          assets=[dict(name=p.name, bytes=p.stat().st_size, sha256=sha256_file(p))
                                  for p in [*packages, checksums, receipt_path]])
            save_receipt(output / 'github-assets.plan.json', record)
            print(json.dumps(record), flush=True)
            return
        record = attach_verified_assets(args.tag, commit, [*packages, checksums, receipt_path])
        record['input_stamp'] = dict(copr_receipt=sha256_file(receipt_path), checksums=sha256_file(checksums))
        save_receipt(output / 'github-assets.json', record)
        print(json.dumps(record), flush=True)
        return
    receipt = dict(tag=args.tag, source_commit=commit, project=f'{OWNER}/{PROJECT}',
                   chroots=chroots, status='pending')
    build_id = args.build_id
    if receipt_path.exists():
        previous = json.loads(receipt_path.read_text())
        previous_chroots = previous.get('chroots') or [previous.get('chroot')]
        if any(previous[key] != receipt[key] for key in ('tag', 'source_commit', 'project')) \
                or sorted(previous_chroots) != sorted(chroots):
            raise ValueError('Output directory belongs to a different release')
        if build_id is not None and build_id != previous['build_id']:
            raise ValueError('Output directory belongs to a different build')
        build_id = previous['build_id']
    if build_id is None:
        if (output / 'submission-intent.json').exists():
            raise ValueError('Previous submission has no durable build receipt; resume the original build with --build-id before assembling another SRPM')
        # The repository may remain private. Upload only the committed, reviewed
        # source export; never give COPR a GitHub token or the private Git history.
        srpms = output / 'srpms'
        command = ['python3', 'scripts/build-srpm.py', '--outdir', str(srpms)]
        for flag, value in [('--pdfium-sources', args.pdfium_sources),
                            ('--source-cache', args.source_cache), ('--cargo-home', args.cargo_home)]:
            if value is not None:
                command += [flag, str(value.resolve())]
        subprocess.run(command, cwd=ROOT, check=True)
        sources = list(srpms.glob('*.src.rpm'))
        if len(sources) != 1:
            raise ValueError('Expected one exact source RPM')
        begin_submission(output, args.tag, commit, sources[0], chroots)
        build = client.build_proxy.create_from_file(OWNER, PROJECT, str(sources[0]),
                    buildopts=dict(chroots=chroots, enable_net=False))
        build_id = build.id
    receipt.update(build_id=build_id,
                   build_url=f'https://copr.fedorainfracloud.org/coprs/{OWNER}/{PROJECT}/build/{build_id}/')
    save_receipt(receipt_path, receipt)
    print(receipt['build_url'], flush=True)
    deadline = time.monotonic() + 170 * 60
    while time.monotonic() < deadline:
        build = client.build_proxy.get(build_id)
        if build.ownername != OWNER or build.projectname != PROJECT or not all(chroot in build.chroots for chroot in chroots):
            raise ValueError('COPR build belongs to a different project or platform')
        receipt['status'] = build.state
        save_receipt(receipt_path, receipt)
        if build.state == 'succeeded':
            break
        if build.state in ('failed', 'canceled', 'cancelled', 'skipped'):
            raise RuntimeError('COPR did not build the release: ' + build.state)
        time.sleep(30)
    else:
        raise TimeoutError('COPR remains pending; resume with its recorded build_id')
    for chroot in chroots:
        if client.build_chroot_proxy.get(build_id, chroot).state != 'succeeded':
            raise ValueError(chroot + ' did not succeed')
        subprocess.run(['copr-cli', 'download-build', str(build_id), '--chroot', chroot,
                        '--dest', str(output / 'packages'), '--rpms'], check=True)
    source_url = source_package_url(build)
    source_name = Path(urlsplit(source_url).path).name
    if not any((output / 'packages').rglob('*.src.rpm')):
        source_path = output / 'packages' / source_name
        with urllib.request.urlopen(source_url, timeout=120) as response, source_path.open('wb') as target:
            import shutil
            shutil.copyfileobj(response, target)
    packages = verified_packages(output / 'packages', args.tag[1:], commit, chroots)
    receipt['verified_chroots'] = chroots
    save_receipt(receipt_path, receipt)
    checksums = output / 'SHA256SUMS-fedora.txt'
    checksums.write_text(''.join(sha256_file(p) + '  ' + p.name + '\n'
                                 for p in packages))
    if args.attach:
        record = attach_verified_assets(args.tag, commit, [*packages, checksums, receipt_path])
        record['input_stamp'] = dict(copr_receipt=sha256_file(receipt_path), checksums=sha256_file(checksums))
        save_receipt(output / 'github-assets.json', record)


if __name__ == '__main__':
    main()
