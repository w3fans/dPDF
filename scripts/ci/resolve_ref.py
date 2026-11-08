"""Resolve a trusted branch/tag to a commit without interpreting shell text."""
import os
import re
import subprocess

ref = os.environ.get('REQUESTED_REF') or os.environ['GITHUB_REF']
if ref == 'refs/heads/main':
    ref = 'main'
elif ref.startswith('refs/tags/'):
    ref = ref[10:]
if not re.fullmatch(r'(main|v\d+\.\d+\.\d+|[0-9a-f]{40})', ref):
    raise SystemExit('Use main, a vMAJOR.MINOR.PATCH tag, or a full commit SHA')
fetch_ref = ('refs/tags/' + ref if ref.startswith('v') else ref)
# A push validates that event's commit, even if main advances before this job starts.
if not os.environ.get('REQUESTED_REF'):
    fetch_ref = os.environ['GITHUB_SHA']
subprocess.run(['git', 'fetch', '--no-tags', '--depth=1', 'origin', fetch_ref], check=True)
sha = subprocess.check_output(['git', 'rev-parse', 'FETCH_HEAD^{commit}'], text=True).strip()
# A syntactically valid SHA may still belong to an unreviewed PR. Admit only
# commits reachable from the approved main history before any Windows worker.
shallow = subprocess.check_output(['git', 'rev-parse', '--is-shallow-repository'], text=True).strip()
fetch_main = ['git', 'fetch', '--no-tags']
if shallow == 'true':
    fetch_main.append('--unshallow')
subprocess.run(fetch_main + ['origin', 'refs/heads/main:refs/remotes/origin/main'], check=True)
if subprocess.run(['git', 'merge-base', '--is-ancestor', sha, 'refs/remotes/origin/main']).returncode != 0:
    raise SystemExit('Requested commit is not in approved main history; Windows acceptance remains PENDING')
with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as output:
    output.write(f'commit={sha}\nref={ref}\n')
