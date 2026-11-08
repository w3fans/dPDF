"""Capture CI metadata and one recent failed worker log for SSH-only maintainers."""
import json
import os
from pathlib import Path
import subprocess
import sys


def api(path):
    result = subprocess.run(['gh', 'api', path], capture_output=True)
    if result.returncode:
        raise subprocess.CalledProcessError(result.returncode, ['gh', 'api', path],
                                            output=result.stdout, stderr=result.stderr)
    return result.stdout


repo = os.environ['GITHUB_REPOSITORY']
run_id = os.environ['GITHUB_RUN_ID']
runs = json.loads(api(f'repos/{repo}/actions/workflows/windows-validation.yml/runs?per_page=8'))
snapshot = dict(current_jobs=json.loads(api(f'repos/{repo}/actions/runs/{run_id}/jobs?per_page=100')),
                recent_runs=runs, recent_failure=None)
for run in runs['workflow_runs']:
    if run['status'] != 'completed' or run['conclusion'] != 'failure':
        continue
    jobs = json.loads(api(f'repos/{repo}/actions/runs/{run["id"]}/jobs?per_page=100'))
    failed = next((job for job in jobs['jobs'] if job['name'].endswith('/ validate')
                   and job['conclusion'] == 'failure'), None)
    if failed:
        try:
            # gh follows the signed log redirect without forwarding GitHub auth to blob storage.
            log = api(f'repos/{repo}/actions/jobs/{failed["id"]}/logs')
            snapshot['recent_failure'] = dict(job_id=failed['id'], run_id=run['id'],
                                              source_commit=run['head_sha'], url=failed['html_url'],
                                              log_tail=log[-65536:].decode('utf-8', errors='replace'))
        except subprocess.CalledProcessError as error:
            import re
            status = re.search(rb'HTTP (\d{3})', error.stderr or b'')
            snapshot['recent_failure'] = dict(job_id=failed['id'], error='Job log unavailable',
                                              http_status=status[1].decode() if status else 'unknown')
            continue
        break
Path(sys.argv[1]).write_text(json.dumps(snapshot, indent=2) + '\n', encoding='utf-8')
