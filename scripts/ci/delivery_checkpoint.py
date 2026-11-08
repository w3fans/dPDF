"""Advance a frozen delivery checkpoint from exact-source publication receipts."""
from datetime import datetime, timezone
import fcntl
import json
import os
from pathlib import Path
import re


def read(path):
    return json.loads(path.read_text()) if path.is_file() else None


def page_receipt(root, tag, commit):
    receipt = read(root / (tag + '.json'))
    if receipt is None:
        return None
    if receipt.get('tag') != tag or receipt.get('source_commit') != commit \
            or receipt.get('status') not in ('CREATED_VERIFIED', 'EXISTING_VERIFIED') \
            or receipt.get('url') != 'https://github.com/w3fans/dPDF/releases/tag/' + tag \
            or type(receipt.get('id')) is not int or receipt['id'] <= 0 \
            or not re.fullmatch(r'[a-f0-9]{64}', receipt.get('notes_sha256', '')):
        raise ValueError('Publication receipt differs from the frozen delivery identity')
    return receipt


def sync(path, pages, publications):
    """No network, builds or goal completion; missing receipts remain unfinished."""
    if not path.exists():
        return 'NO_CHECKPOINT'
    with path.with_suffix('.lock').open('a+') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        checkpoint = read(path)
        target = checkpoint['target']
        tag, commit = target['tag'], target['source_commit']
        if target.get('frozen') is not True or not re.fullmatch(r'v0\.1\.\d{1,2}', tag) \
                or not re.fullmatch(r'[a-f0-9]{40}', commit):
            raise ValueError('A frozen existing-preview target is required')
        milestones = checkpoint['milestones']
        for name in ('CODE_READY', 'TAGGED'):
            if milestones[name].get('source_commit') != commit:
                raise ValueError('Checkpoint source milestones differ from the frozen target')
        backfill_ready = True
        for previous_tag, row in checkpoint.get('backfill', {}).items():
            if not re.fullmatch(r'v0\.1\.\d{1,2}', previous_tag) \
                    or not re.fullmatch(r'[a-f0-9]{40}', row['source_commit']):
                raise ValueError('Invalid backfill identity')
            receipt = page_receipt(pages, previous_tag, row['source_commit'])
            row['github_release'] = 'PASS' if receipt else 'PENDING'
            if receipt:
                row['release_url'] = receipt['url']
            backfill_ready = backfill_ready and receipt is not None
        page = page_receipt(pages, tag, commit)
        failure = read(pages / (tag + '.failure.json'))
        pending = read(pages / (tag + '.plan.json'))
        if pending and (pending.get('tag') != tag or pending.get('source_commit') != commit):
            raise ValueError('Pending publication plan differs from the frozen target')
        state = milestones['GITHUB_RELEASE_PUBLISHED']
        state['status'] = 'PASS' if page else 'BLOCKED'
        if page:
            state.update(source_commit=commit, url=page['url'], notes_sha256=page['notes_sha256'],
                         receipt=str(pages / (tag + '.json')))
            state.pop('blocker', None)
        output = publications / tag / 'copr'
        assets = read(output / 'github-assets.json')
        attached = dict(status='PENDING', receipt=str(output / 'github-assets.json'))
        if assets is not None:
            if assets.get('tag') != tag or assets.get('source_commit') != commit \
                    or assets.get('status') != 'VERIFIED':
                raise ValueError('Asset receipt differs from the frozen delivery identity')
            files = assets.get('assets', [])
            actual = {entry['name']: entry for entry in files}
            if len(actual) != len(files):
                raise ValueError('Ambiguous asset receipt names')
            packages = milestones['COPR_PUBLISHED']['artifacts']
            binaries = [entry for entry in packages if entry['file'].endswith('.x86_64.rpm')]
            if len(binaries) != 2 or {Path(entry['file']).parts[0] for entry in binaries} \
                    != {'fedora-44-x86_64', 'fedora-45-x86_64'} \
                    or not any(entry['file'].endswith('.src.rpm') for entry in packages):
                raise ValueError('Qualified package evidence must cover both targets and source RPMs')
            expected = {Path(entry['file']).name: (entry['sha256'], entry['bytes']) for entry in packages}
            if len(expected) != len(packages):
                raise ValueError('Ambiguous qualified package names')
            for name, (digest, size) in expected.items():
                entry = actual.get(name, {})
                if entry.get('sha256') != digest or entry.get('size') != size \
                        or entry.get('url') != f'https://github.com/w3fans/dPDF/releases/download/{tag}/{name}':
                    raise ValueError('Published Fedora asset identity differs from qualified packages')
            stamp = assets.get('input_stamp', {})
            for name, field in [('copr-release.json', 'copr_receipt'), ('SHA256SUMS-fedora.txt', 'checksums')]:
                if not re.fullmatch(r'[a-f0-9]{64}', stamp.get(field, '')) \
                        or actual.get(name, {}).get('sha256') != stamp[field]:
                    raise ValueError('Published source/checksum metadata is not verified')
            attached.update(status='PASS', matched_package_assets=len(expected))
        milestones['COPR_PUBLISHED']['github_assets'] = attached
        block = milestones['BLOCKED']
        windows_ready = milestones['WINDOWS_ASSETS_PUBLISHED']['status'] == 'PASS'
        if not page:
            code = 'RELEASE_RECOVERY_FAILED' if failure else 'GITHUB_RELEASE_PENDING'
            if not failure and pending and pending.get('status') == 'PENDING_API_AUTH':
                code = 'GITHUB_API_AUTH'
            block.update(status='YES', code=code,
                         detail='No exact-source verified GitHub Release page receipt; inspect the recorded recovery state')
            state['blocker'] = code
        elif attached['status'] != 'PASS':
            block.update(status='YES', code='FEDORA_GITHUB_ASSETS_PENDING',
                         detail='Release page verified; exact-source Fedora asset recovery unfinished')
        elif not windows_ready:
            block.update(status='YES', code='WINDOWS_NATIVE_BUILD_PENDING',
                         detail='Native Windows publication remains unfinished; a staged request is not a build receipt')
            block.pop('required_owner_action', None)
        elif not backfill_ready:
            block.update(status='YES', code='HISTORICAL_PAGE_RECOVERY_PENDING')
        else:
            block.update(status='NO', code=None)
            block.pop('required_owner_action', None)
        if block['code'] != 'GITHUB_API_AUTH':
            block.pop('required_owner_action', None)
        ready = page is not None and attached['status'] == 'PASS' and windows_ready and backfill_ready
        checkpoint['delivery_ready_for_final_verification'] = ready
        if not checkpoint.get('complete', False):
            milestones['VERIFIED']['status'] = 'PENDING_FINAL_READBACK' if ready else 'PARTIAL'
        checkpoint['last_checkpoint_sync'] = datetime.now(timezone.utc).isoformat()
        checkpoint['checkpoint_sync_scope'] = 'Local verified receipts; final live publication audit remains separate'
        incoming = path.with_suffix('.incoming')
        with incoming.open('w') as stream:
            json.dump(checkpoint, stream, indent=2)
            stream.write('\n')
            stream.flush()
            os.fsync(stream.fileno())
        incoming.chmod(0o600)
        incoming.replace(path)
    return 'UPDATED'
