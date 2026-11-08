"""Publish diagnostics on a new report-only branch, never modify application branches."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile


def validate(report, commit, lane, allow_pending=False):
    if not re.fullmatch('[0-9a-f]{40}', commit):
        raise ValueError('Invalid commit')
    if report.get('schema') != 1 or report.get('source_commit') != commit or report.get('lane') != lane:
        raise ValueError('Report identity mismatch')
    if report.get('status') not in (('PASS', 'FAIL', 'CANCELLED', 'QUEUED') if allow_pending else ('PASS', 'FAIL', 'CANCELLED')):
        raise ValueError('Invalid report status')
    if report['status'] == 'CANCELLED' and (report.get('job_result') != 'cancelled' or
                                           any(item.get('status') == 'FAIL' for item in report.get('checks', []))):
        raise ValueError('Cancellation needs actual job evidence and must not mask a failed check')
    if report['status'] == 'PASS' and (not report.get('checks') or
                                       any(item.get('status') != 'PASS' for item in report['checks'])):
        raise ValueError('A passing report must contain actual passing checks')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--artifact', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--lane', choices=['hosted', 'self-hosted', 'desktop'], required=True)
    parser.add_argument('--run', required=True)
    parser.add_argument('--pending', action='store_true')
    args = parser.parse_args()
    report = json.loads((args.artifact / 'report.json').read_text(encoding='utf-8'))
    validate(report, args.commit, args.lane, allow_pending=args.pending)
    notice_summary = args.artifact / 'notices' / 'summary.json'
    if notice_summary.is_file():
        summary = json.loads(notice_summary.read_text(encoding='utf-8'))
        if summary.get('source_commit') != args.commit or summary.get('schema') != 1:
            raise ValueError('Dependency notice snapshot identity mismatch')
        report['dependency_notices'] = summary
        (args.artifact / 'report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    if report['status'] in ('FAIL', 'CANCELLED'):
        logs = []
        candidates = [('bootstrap', args.artifact / 'bootstrap.log')]
        candidates += [(name, args.artifact / 'logs' / (name + '.log')) for name in
                       ('ci-contract', 'powershell-syntax', 'toolchain', 'compiler')]
        for check in report.get('checks', []):
            name = check.get('name', '')
            if check.get('status') == 'FAIL' and re.fullmatch('[a-z0-9-]+', name):
                candidates.append((name, args.artifact / 'logs' / (name + '.log')))
        active = report.get('active_check', '')
        if re.fullmatch('[a-z0-9-]+', active):
            candidates.append((active, args.artifact / 'logs' / (active + '.log')))
        remaining = 98304
        for name, path in candidates:
            if path.is_file() and remaining:
                data = path.read_bytes()
                encoding = 'utf-16' if data.startswith((b'\xff\xfe', b'\xfe\xff')) else 'utf-8-sig'
                tail = data.decode(encoding, errors='replace')[-min(16384, remaining):]
                logs.append(dict(name=name, tail=tail))
                remaining -= len(tail)
        report['failure_logs'] = logs
        (args.artifact / 'report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    if not re.fullmatch(r'\d+-\d+', args.run):
        raise ValueError('Expected run ID and attempt')
    if os.environ.get('GITHUB_REPOSITORY'):
        run_id, attempt = args.run.split('-')
        expected_url = f"https://github.com/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{run_id}"
        if report.get('run_url') != expected_url or report.get('run_attempt') != attempt:
            raise ValueError('Report run/attempt identity mismatch')
    branch = f'validation/ci-windows-{args.lane}-{args.run}'
    subprocess.run(['git', 'fetch', '--depth=1', 'origin', args.commit], check=True)
    remote_ref = f'refs/heads/{branch}'
    existing = subprocess.check_output(['git', 'ls-remote', 'origin', remote_ref], text=True).strip()
    base = args.commit
    if existing:
        subprocess.run(['git', 'fetch', '--depth=1', 'origin', remote_ref], check=True)
        base = subprocess.check_output(['git', 'rev-parse', 'FETCH_HEAD'], text=True).strip()
        previous = json.loads(subprocess.check_output(
            ['git', 'show', base + ':docs/validation/automated/report.json'], text=True))
        validate(previous, args.commit, args.lane, allow_pending=True)
        if args.pending:
            raise ValueError('An existing receipt must never be reset to queued')
        changed = subprocess.check_output(['git', 'diff', '--no-renames', '--name-only', args.commit, base], text=True).splitlines()
        allowed = {'docs/validation/automated/' + name for name in ('report.json', 'README.md', 'actions-jobs.json')}
        if not set(changed) <= allowed:
            raise ValueError('Report branch contains application changes')
    with tempfile.TemporaryDirectory(prefix='dpdf-report-') as directory:
        tree = Path(directory) / 'tree'
        subprocess.run(['git', 'worktree', 'add', '--detach', str(tree), base], check=True)
        try:
            destination = tree / 'docs' / 'validation' / 'automated'
            if not destination.resolve().is_relative_to(tree.resolve()):
                raise ValueError('Report destination escapes worktree')
            destination.mkdir(parents=True, exist_ok=True)
            for name in ('report.json', 'README.md', 'actions-jobs.json'):
                data = (args.artifact / name).read_bytes()
                if len(data) > 1024 ** 2:
                    raise ValueError('Report exceeds 1 MiB limit')
                (destination / name).write_bytes(data)
            subprocess.run(['git', '-C', str(tree), 'add', '--', 'docs/validation/automated/report.json',
                            'docs/validation/automated/README.md',
                            'docs/validation/automated/actions-jobs.json'], check=True)
            subprocess.run(['git', '-C', str(tree), '-c', 'user.name=github-actions[bot]',
                            '-c', 'user.email=41898282+github-actions[bot]@users.noreply.github.com',
                            '-c', 'core.hooksPath=/dev/null', 'commit', '-m',
                            f"Windows {args.lane} validation: {report['status']} ({args.commit[:12]})"], check=True)
            published = subprocess.check_output(['git', '-C', str(tree), 'rev-parse', 'HEAD'], text=True).strip()
            # Checkout v7 scopes persisted credentials to its original Git directory.
            # Push the shared object from that checkout, not from the temporary worktree.
            subprocess.run(['git', 'push', 'origin', f'{published}:refs/heads/{branch}'], check=True)
        finally:
            subprocess.run(['git', 'worktree', 'remove', '--force', str(tree)], check=True)
    print('Report branch: ' + branch)


if __name__ == '__main__':
    main()
