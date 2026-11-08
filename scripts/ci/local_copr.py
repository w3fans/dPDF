"""Publish reviewed immutable tags locally using the existing COPR publisher.

No version allocation, commits, tag creation, Git push, token export or PR execution.
The operator's private configuration supplies all host/cache/state paths.
"""
import argparse
import fcntl
import importlib.util
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()


def tag_number(tag):
    if not re.fullmatch(r'v0\.1\.\d{1,2}', tag):
        raise ValueError('Only existing development preview tags are accepted')
    return int(tag.rsplit('.', 1)[1])


def source_checkout(args, tag):
    tag_number(tag)
    commit = git(args.repository, 'rev-parse', tag + '^{commit}')
    subprocess.run(['git', '-C', str(args.repository), 'merge-base', '--is-ancestor',
                    commit, 'origin/main'], check=True)
    target = args.worktrees / tag
    if not target.exists():
        subprocess.run(['git', '-C', str(args.repository), 'worktree', 'add', '--detach',
                        str(target), tag], check=True)
    if target.is_symlink() or git(target, 'rev-parse', 'HEAD') != commit or git(
            target, 'status', '--porcelain=v1', '--untracked-files=all'):
        raise ValueError('Publication checkout identity/cleanliness mismatch')
    return target, commit


def publisher_module(source):
    sys.path.insert(0, str(source / 'scripts'))
    # Each immutable tag supplies its own matching checksum helper.
    sys.modules.pop('file_digest', None)
    spec = importlib.util.spec_from_file_location('tag_publisher', source / 'scripts/publish-copr.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def receipt_stamp(output):
    return {name: hashlib.sha256((output / name).read_bytes()).hexdigest()
            for name in ('copr-release.json', 'SHA256SUMS-fedora.txt')}


def verify_completed(source, tag, commit, output):
    receipt_path = output / 'copr-release.json'
    if not receipt_path.exists():
        return False
    receipt = json.loads(receipt_path.read_text())
    if receipt['tag'] != tag or receipt['source_commit'] != commit or receipt['project'] != 'zvegelj/dpdf':
        raise ValueError('Existing receipt belongs to a different release')
    if receipt['status'] != 'succeeded':
        return False
    from copr.v3 import Client
    client = Client.create_from_config_file()
    build = client.build_proxy.get(receipt['build_id'])
    if build.ownername != 'zvegelj' or build.projectname != 'dpdf' or build.state != 'succeeded':
        raise ValueError('COPR readback does not confirm the existing release')
    module = publisher_module(source)
    for target in module.CHROOTS:
        if target not in build.chroots or client.build_chroot_proxy.get(build.id, target).state != 'succeeded':
            raise ValueError('A required Fedora target is not complete')
    module.release_identity(tag)
    packages = module.verified_packages(output / 'packages', tag[1:], commit, list(module.CHROOTS))
    actual = ''.join(module.sha256_file(p) + '  ' + p.name + '\n' for p in packages)
    if (output / 'SHA256SUMS-fedora.txt').read_text() != actual:
        raise ValueError('Existing release package checksums differ')
    print(json.dumps(dict(tag=tag, source_commit=commit, build_id=build.id,
                         status='VERIFIED_EXISTING', new_build_submitted=False)), flush=True)
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('repository', 'worktrees', 'outputs', 'privacy-patterns',
                 'pdfium-sources', 'source-cache', 'cargo-home'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--baseline', required=True, help='Existing qualified tag; older history is never submitted')
    parser.add_argument('--verify-tag', help='Verify an existing publication without submitting anything')
    parser.add_argument('--dry-run', action='store_true', help='Verify baseline and list queued tags without submitting')
    args = parser.parse_args()
    baseline = tag_number(args.baseline)
    args.repository = args.repository.resolve()
    args.worktrees = args.worktrees.resolve()
    args.outputs = args.outputs.resolve()
    if git(args.repository, 'remote', 'get-url', 'origin') != 'git@github-w3f:w3fans/dPDF.git':
        raise ValueError('Unexpected repository authority')
    args.worktrees.mkdir(parents=True, exist_ok=True)
    args.outputs.mkdir(parents=True, exist_ok=True)
    with (args.outputs / '.copr-publication.lock').open('a+') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        subprocess.run(['git', '-C', str(args.repository), 'fetch', '--quiet', 'origin',
                        'refs/heads/main:refs/remotes/origin/main',
                        'refs/tags/v*:refs/tags/v*'], check=True)
        if args.verify_tag:
            source, commit = source_checkout(args, args.verify_tag)
            if not verify_completed(source, args.verify_tag, commit, args.outputs / args.verify_tag / 'copr'):
                raise ValueError('No completed publication exists; nothing submitted')
            return 0
        state_path = args.outputs / '.copr-publications.json'
        state = json.loads(state_path.read_text()) if state_path.exists() else {
            'schema': 1, 'baseline': args.baseline, 'published': {}}
        if state['schema'] != 1 or state['baseline'] != args.baseline:
            raise ValueError('Publication state baseline differs')

        def completed(source, tag, commit, output):
            previous = state['published'].get(tag)
            if previous is not None:
                if previous['source_commit'] != commit:
                    raise ValueError('A previously published tag moved')
                if previous['receipt_stamp'] == receipt_stamp(output):
                    print(json.dumps(dict(tag=tag, status='ALREADY_PUBLISHED', new_build_submitted=False)), flush=True)
                    return True
            if not verify_completed(source, tag, commit, output):
                return False
            state['published'][tag] = dict(source_commit=commit, receipt_stamp=receipt_stamp(output))
            incoming = state_path.with_suffix('.tmp')
            incoming.write_text(json.dumps(state, indent=2) + '\n')
            incoming.chmod(0o600)
            incoming.replace(state_path)
            return True

        source, commit = source_checkout(args, args.baseline)
        if not completed(source, args.baseline, commit, args.outputs / args.baseline / 'copr'):
            raise ValueError('Qualified baseline receipt is required before automatic publication')
        tags = [tag for tag in git(args.repository, 'tag', '--list', 'v0.1.*').splitlines()
                if re.fullmatch(r'v0\.1\.\d{1,2}', tag) and tag_number(tag) > baseline]
        for tag in sorted(tags, key=tag_number):
            source, commit = source_checkout(args, tag)
            output = args.outputs / tag / 'copr'
            if completed(source, tag, commit, output):
                continue
            policy_path = source / 'ops/build-routing.json'
            policy = json.loads(policy_path.read_text()) if policy_path.is_file() else {}
            if policy.get('copr_publication') != 'local-operator':
                print(json.dumps(dict(tag=tag, source_commit=commit, status='PENDING_OPERATOR_REVIEW',
                                      reason='Tag does not opt into this local publisher', new_build_submitted=False)), flush=True)
                continue
            if args.dry_run:
                print(json.dumps(dict(tag=tag, source_commit=commit, status='QUEUED', new_build_submitted=False)), flush=True)
                continue
            subprocess.run([sys.executable, str(source / 'scripts/check-public-export.py'),
                            '--patterns-file', str(args.privacy_patterns), '--ref', tag,
                            '--report', str(output.parent / 'public-export-check.json')], cwd=source, check=True)
            command = [sys.executable, str(source / 'scripts/publish-copr.py'), '--tag', tag,
                       '--output', str(output), '--pdfium-sources', str(args.pdfium_sources),
                       '--source-cache', str(args.source_cache), '--cargo-home', str(args.cargo_home)]
            subprocess.run(command, cwd=source, check=True)
            if not completed(source, tag, commit, output):
                raise ValueError('Publication did not produce verified artifacts')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
