"""Recover GitHub preview pages for existing tags; never build, tag or change versions."""
import argparse
import fcntl
import hashlib
import json
import shlex
from pathlib import Path
import re
import shutil
import subprocess
import tomllib

from check_pr_policy import redaction_only
import delivery_checkpoint

REPOSITORY = 'w3fans/dPDF'
ENDPOINT = 'repos/' + REPOSITORY


def git(repository, *args):
    return subprocess.check_output(['git', '-C', str(repository), *args], text=True).strip()


def preview_number(tag):
    if not re.fullmatch(r'v0\.1\.\d{1,2}', tag):
        raise ValueError('Only existing development preview tags are accepted')
    return int(tag.rsplit('.', 1)[1])


def plan(repository, tag, patterns):
    preview_number(tag)
    commit = git(repository, 'rev-parse', '--verify', tag + '^{commit}')
    subprocess.run(['git', '-C', str(repository), 'merge-base', '--is-ancestor',
                    commit, 'origin/main'], check=True, capture_output=True)
    path = 'docs/releases/' + tag + '.md'
    manifest = json.loads(git(repository, 'show', tag + ':release.json'))
    cargo = tomllib.loads(git(repository, 'show', tag + ':Cargo.toml'))
    lock = tomllib.loads(git(repository, 'show', tag + ':Cargo.lock'))
    if manifest.get('tag') != tag or manifest.get('version') != tag[1:] \
            or manifest.get('channel') != 'development-preview' or manifest.get('notes') != path \
            or cargo['package']['version'] != tag[1:] \
            or [entry['version'] for entry in lock['package'] if entry['name'] == 'dpdf'] != [tag[1:]]:
        raise ValueError('Existing release contract/version does not match the tag')
    original = git(repository, 'show', tag + ':' + path) + '\n'
    notes = git(repository, 'show', 'origin/main:' + path) + '\n'
    if notes != original and not redaction_only(original, notes):
        raise ValueError('Historical release notes changed beyond explicit privacy redactions')
    if not notes.startswith('# dPDF ' + tag + ' '):
        raise ValueError('Release note title does not identify the exact preview')
    if any(pattern.search(notes) for pattern in patterns):
        raise ValueError('Release notes fail the private-reference check')
    return dict(tag=tag, source_commit=commit, notes=notes,
                notes_sha256=hashlib.sha256(notes.encode()).hexdigest())


class GitHub:
    def __call__(self, endpoint, payload=None, missing=False):
        command = ['gh', 'api', '--hostname', 'github.com', endpoint]
        if payload is not None:
            command += ['--method', 'POST', '--input', '-']
        result = subprocess.run(command, input=json.dumps(payload) if payload is not None else None,
                                text=True, capture_output=True)
        if result.returncode:
            if missing and payload is None and re.search(r'\(HTTP 404\)', result.stderr):
                return None
            raise RuntimeError('GitHub Release API request failed; no publication is claimed')
        return json.loads(result.stdout)


def remote_commit(api, tag):
    item = api(ENDPOINT + '/git/ref/tags/' + tag)['object']
    for _ in range(8):
        if item['type'] == 'commit' and re.fullmatch(r'[a-f0-9]{40}', item['sha']):
            return item['sha']
        if item['type'] != 'tag' or not re.fullmatch(r'[a-f0-9]{40}', item['sha']):
            break
        item = api(ENDPOINT + '/git/tags/' + item['sha'])['object']
    raise ValueError('Remote tag does not resolve to a commit')


def publish(api, item):
    repository = api(ENDPOINT)
    if repository.get('full_name') != REPOSITORY or not repository.get('permissions', {}).get('push'):
        raise ValueError('Authorized API write access to the intended repository is required')
    tag = item['tag']
    preview_number(tag)
    if remote_commit(api, tag) != item['source_commit']:
        raise ValueError('Remote tag differs from reviewed source; no tag is created or moved')
    endpoint = ENDPOINT + '/releases/tags/' + tag
    title = 'dPDF ' + tag + ' — development preview'
    existing = api(endpoint, missing=True)
    created = existing is None
    if created:
        try:
            api(ENDPOINT + '/releases', dict(tag_name=tag, target_commitish=item['source_commit'],
                name=title, body=item['notes'], draft=False, prerelease=True, make_latest='false'))
        except RuntimeError:
            # The POST may have succeeded, or another publisher won the race.
            # Read back the unique tag page before deciding whether it failed.
            if api(endpoint, missing=True) is None:
                raise
    release = api(endpoint)
    if release.get('tag_name') != tag or release.get('name') != title \
            or release.get('draft') is not False or release.get('prerelease') is not True \
            or release.get('body') != item['notes'] or type(release.get('id')) is not int \
            or release['id'] <= 0 or release.get('html_url') != 'https://github.com/' + REPOSITORY + '/releases/tag/' + tag:
        raise ValueError('Release-page readback differs; existing pages are never overwritten')
    if remote_commit(api, tag) != item['source_commit']:
        raise ValueError('Tag changed during publication; stop for review')
    return dict(tag=tag, source_commit=item['source_commit'], notes_sha256=item['notes_sha256'],
                id=release['id'], url=release['html_url'],
                status='CREATED_VERIFIED' if created else 'EXISTING_VERIFIED')


def reconcile(item, output, api=None, apply=False, recheck=False):
    """Never replace a verified publication receipt with an offline/pending plan."""
    path = output / (item['tag'] + '.json')
    previous = json.loads(path.read_text()) if path.exists() else None
    if previous is not None:
        if any(previous.get(key) != item[key] for key in ('tag', 'source_commit', 'notes_sha256')):
            raise ValueError('Existing receipt identity differs; preserve it for review')
        if previous.get('status') not in ('CREATED_VERIFIED', 'EXISTING_VERIFIED'):
            raise ValueError('Existing publication receipt is not verified')
    if not apply or api is None:
        result = {key: value for key, value in item.items() if key != 'notes'}
        result.update(status='PENDING_API_AUTH' if apply else 'DRY_RUN',
                      previous_verified_receipt=previous is not None)
        destination = output / (item['tag'] + '.plan.json')
    elif previous is not None and not recheck:
        return dict(previous, status='VERIFIED_RECEIPT_RETAINED')
    else:
        result = publish(api, item)
        destination = path
    incoming = destination.with_suffix('.incoming')
    incoming.write_text(json.dumps(result, indent=2) + '\n')
    incoming.chmod(0o600)
    incoming.replace(destination)
    return result


def recover_copr_assets(item, publication_root, worktrees, publisher, recheck=False):
    """Resume existing verified packages only; missing/in-progress builds stay pending."""
    preview_number(item['tag'])
    output = publication_root / item['tag'] / 'copr'
    path = output / 'copr-release.json'
    if not path.is_file():
        return 'PENDING_VERIFIED_COPR_ARTIFACTS'
    copr = json.loads(path.read_text())
    if copr.get('tag') != item['tag'] or copr.get('source_commit') != item['source_commit'] \
            or copr.get('project') != 'zvegelj/dpdf':
        raise ValueError('COPR receipt differs from reviewed release source')
    if copr.get('status') != 'succeeded' \
            or set(copr.get('verified_chroots', [])) != {'fedora-44-x86_64', 'fedora-45-x86_64'}:
        return 'PENDING_VERIFIED_COPR_ARTIFACTS'
    stamps = dict(copr_receipt=hashlib.sha256(path.read_bytes()).hexdigest(),
                  checksums=hashlib.sha256((output / 'SHA256SUMS-fedora.txt').read_bytes()).hexdigest())
    verified = output / 'github-assets.json'
    if verified.exists():
        previous = json.loads(verified.read_text())
        if previous.get('tag') != item['tag'] or previous.get('source_commit') != item['source_commit']:
            raise ValueError('Existing asset receipt belongs to different source')
        if previous.get('status') == 'VERIFIED' and previous.get('input_stamp') == stamps and not recheck:
            return 'VERIFIED_RECEIPT_RETAINED'
    source = worktrees / item['tag']
    if not source.is_dir():
        return 'PENDING_EXACT_TAG_CHECKOUT'
    if git(source, 'rev-parse', 'HEAD') != item['source_commit'] \
            or git(source, 'status', '--porcelain', '--untracked-files=all'):
        raise ValueError('Attachment checkout must be clean at the exact reviewed source')
    with (output / 'github-assets-recovery.log').open('w') as log:
        subprocess.run(['python3', str(publisher), '--tag', item['tag'], '--source', str(source),
                        '--output', str(output), '--build-id', str(copr['build_id']), '--attach-only'],
                       stdout=log, stderr=subprocess.STDOUT, check=True, timeout=900)
    current = json.loads(verified.read_text())
    if current.get('tag') != item['tag'] or current.get('source_commit') != item['source_commit'] \
            or current.get('status') != 'VERIFIED' or current.get('input_stamp') != stamps:
        raise ValueError('Asset publisher did not produce an exact verified receipt')
    return 'VERIFIED'


def operator_config():
    """Read only configured publication paths; never evaluate shell or export credentials."""
    names = {'DPDF_RELEASE_PAGE_REPOSITORY', 'DPDF_PUBLICATION_OUTPUTS',
             'DPDF_PRIVACY_PATTERNS'}
    path = Path.home() / '.config/dpdf/publication.env'
    values = {}
    for line in path.read_text().splitlines():
        key, sep, value = line.partition('=')
        if sep and key in names:
            words = shlex.split(value, comments=True)
            if len(words) != 1 or not Path(words[0]).is_absolute():
                raise ValueError('Publication configuration requires absolute paths')
            values[key] = Path(words[0])
    if values.keys() != names:
        raise ValueError('Existing local publication configuration is incomplete')
    return values


def operator_status(repository, outputs, tag=None):
    tags = sorted((value for value in git(repository, 'tag', '--list', 'v0.1.*').splitlines()
                   if re.fullmatch(r'v0\.1\.\d{1,2}', value)), key=preview_number)
    tag = tag or tags[-1]
    preview_number(tag)
    commit = git(repository, 'rev-parse', tag + '^{commit}')
    page = delivery_checkpoint.page_receipt(outputs / 'github-release-pages', tag, commit)
    copr = delivery_checkpoint.read(outputs / tag / 'copr/copr-release.json')
    assets = delivery_checkpoint.read(outputs / tag / 'copr/github-assets.json')
    qualified = bool(copr and copr.get('tag') == tag and copr.get('source_commit') == commit
                     and copr.get('status') == 'succeeded'
                     and set(copr.get('verified_chroots', [])) == {'fedora-44-x86_64', 'fedora-45-x86_64'})
    attached = bool(qualified and assets and assets.get('tag') == tag
                    and assets.get('source_commit') == commit and assets.get('status') == 'VERIFIED')
    auth = shutil.which('gh') and subprocess.run(['gh', 'auth', 'status', '--hostname', 'github.com'],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
    blockers = []
    if not auth:
        blockers.append('PENDING_API_AUTH')
    if not page:
        blockers.append('PENDING_GITHUB_RELEASE')
    if not qualified:
        blockers.append('PENDING_COPR')
    if not attached:
        blockers.append('PENDING_FEDORA_GITHUB_ASSETS')
    # Consume only an existing exact-source verified publication receipt referenced
    # by the release journal. A queue/native-build receipt alone is insufficient.
    checkpoint = delivery_checkpoint.read(outputs / 'RELEASE-DELIVERY-CHECKPOINT.json') or {}
    target = checkpoint.get('target', {})
    windows = checkpoint.get('milestones', {}).get('WINDOWS_ASSETS_PUBLISHED', {})
    windows_ready = False
    if target.get('tag') == tag and target.get('source_commit') == commit \
            and windows.get('status') == 'PASS' and windows.get('source_commit') == commit \
            and windows.get('receipt'):
        published = delivery_checkpoint.read(Path(windows['receipt'])) or {}
        files = published.get('assets', [])
        windows_ready = (published.get('status') == 'VERIFIED'
                         and published.get('tag') == tag and published.get('source_commit') == commit
                         and {entry.get('name') for entry in files}
                             >= {'dPDF-' + tag[1:] + '-windows-x64.msi',
                                 'dPDF-' + tag[1:] + '-windows-x64-portable.zip'}
                         and all(re.fullmatch(r'[a-f0-9]{64}', entry.get('sha256', ''))
                                 and entry.get('size', 0) > 0 for entry in files))
    if not windows_ready:
        blockers.append('PENDING_WINDOWS_ARTIFACTS')
    return dict(tag=tag, source_commit=commit,
                github_page='GITHUB_RELEASE_PUBLISHED' if page else 'PENDING',
                url=page['url'] if page else None,
                fedora_44='BUILT_VERIFIED' if qualified else 'PENDING_COPR',
                fedora_45='BUILT_VERIFIED' if qualified else 'PENDING_COPR',
                copr_build_id=copr.get('build_id') if qualified else None,
                fedora_github_assets='VERIFIED' if attached else 'PENDING',
                windows='VERIFIED' if windows_ready else 'PENDING_WINDOWS_ARTIFACTS',
                status='COMPLETE' if not blockers else ('PARTIAL' if page or qualified else 'PENDING'), blockers=blockers,
                evidence_scope='Existing exact-source receipts; use publisher --recheck for live readback')


def operator_command(arguments):
    config = operator_config()
    repository = config['DPDF_RELEASE_PAGE_REPOSITORY']
    outputs = config['DPDF_PUBLICATION_OUTPUTS']
    if arguments == ['delivery-status']:
        print(json.dumps(operator_status(repository, outputs), indent=2))
        return 0
    if len(arguments) != 3 or arguments[:2] != ['deliver', '--tag']:
        raise ValueError('Usage: release.sh delivery-status | deliver --tag EXISTING_TAG')
    tag = arguments[2]
    preview_number(tag)
    patterns = [re.compile(value) for value in json.loads(config['DPDF_PRIVACY_PATTERNS'].read_text())]
    if not patterns:
        raise ValueError('A private publication denylist is required')
    plan(repository, tag, patterns)
    # Starting an active unit joins the same service. Its existing tag scan and
    # advisory lock reconcile all pending pages/assets, without COPR submissions.
    subprocess.run(['systemctl', '--user', 'start', '--no-block', 'dpdf-release-pages.service'], check=True)
    print(json.dumps(dict(tag=tag, status='PUBLICATION_QUEUED',
                         service='dpdf-release-pages.service', new_version_allocated=False,
                         new_build_submitted=False)))
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repository', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--privacy-patterns', type=Path, required=True)
    selection = parser.add_mutually_exclusive_group(required=True)
    selection.add_argument('--tag')
    selection.add_argument('--since', help='First existing preview to reconcile, inclusive')
    parser.add_argument('--fetch', action='store_true')
    parser.add_argument('--apply', action='store_true', help='Create missing pages with existing gh API authentication')
    parser.add_argument('--recheck', action='store_true', help='Read back even previously verified pages')
    parser.add_argument('--asset-publisher', type=Path,
                        help='Maintained publish-copr.py for attachment-only recovery')
    parser.add_argument('--publication-root', type=Path,
                        help='Existing per-tag publication outputs; never submit a build here')
    parser.add_argument('--worktrees', type=Path, help='Existing exact-tag publication checkouts')
    parser.add_argument('--delivery-checkpoint', type=Path,
                        help='Update a frozen delivery checkpoint from verified receipts')
    args = parser.parse_args()
    if any((args.asset_publisher, args.publication_root, args.worktrees)) \
            and not all((args.asset_publisher, args.publication_root, args.worktrees)):
        parser.error('Asset recovery requires publisher, publication root and worktrees together')
    if git(args.repository, 'remote', 'get-url', 'origin') != 'git@github-w3f:w3fans/dPDF.git':
        raise ValueError('Unexpected source repository authority')
    if args.fetch:
        subprocess.run(['git', '-C', str(args.repository), 'fetch', '--quiet', 'origin',
                        'refs/heads/main:refs/remotes/origin/main', 'refs/tags/v*:refs/tags/v*'], check=True)
    patterns = [re.compile(value) for value in json.loads(args.privacy_patterns.read_text())]
    if not patterns:
        raise ValueError('A private publication denylist is required')
    if args.tag:
        tags = [args.tag]
    else:
        first = preview_number(args.since)
        tags = sorted((tag for tag in git(args.repository, 'tag', '--list', 'v0.1.*').splitlines()
                       if re.fullmatch(r'v0\.1\.\d{1,2}', tag) and preview_number(tag) >= first), key=preview_number)
    for tag in tags:
        preview_number(tag)
    ready = args.apply and shutil.which('gh') and subprocess.run(
        ['gh', 'auth', 'status', '--hostname', 'github.com'], stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL).returncode == 0
    args.output.mkdir(parents=True, exist_ok=True)
    with (args.output / '.release-pages.lock').open('a+') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        failed = False
        for tag in tags:
            item = dict(tag=tag, source_commit=None)
            failure = args.output / (tag + '.failure.json')
            phase = 'source_plan'
            try:
                item = plan(args.repository, tag, patterns)
                phase = 'release_page'
                receipt = reconcile(item, args.output, GitHub() if ready else None, args.apply, args.recheck)
                print(json.dumps(receipt), flush=True)
                # An apply run that could not publish must not look healthy to
                # the timer or its caller. Preserve pending plans and keep
                # processing later tags before returning the blocked status.
                if receipt.get('status') == 'PENDING_API_AUTH':
                    failed = True
                if ready and args.asset_publisher:
                    phase = 'copr_assets'
                    assets = recover_copr_assets(item, args.publication_root, args.worktrees,
                                                 args.asset_publisher, args.recheck)
                    print(json.dumps(dict(tag=item['tag'], github_assets=assets)), flush=True)
                failure.unlink(missing_ok=True)
            except Exception as error:
                # A historical tag must not prevent independent later pages or
                # the frozen delivery target from progressing. Keep verified
                # receipts; the timer retries only unfinished work next time.
                failed = True
                result = dict(tag=item['tag'], source_commit=item['source_commit'],
                              status='FAILED', phase=phase, error=str(error))
                failure.write_text(json.dumps(result, indent=2) + '\n')
                failure.chmod(0o600)
                print(json.dumps(result), flush=True)
        if args.delivery_checkpoint:
            if not args.publication_root:
                raise ValueError('Checkpoint synchronization requires the existing publication root')
            delivery_checkpoint.sync(args.delivery_checkpoint, args.output, args.publication_root)
    return 1 if failed else 0


if __name__ == '__main__':
    import sys
    if len(sys.argv) > 1 and sys.argv[1] in ('delivery-status', 'deliver'):
        raise SystemExit(operator_command(sys.argv[1:]))
    raise SystemExit(main())
