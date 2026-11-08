"""Retain an honest final receipt if setup/cancellation interrupted the Windows harness."""
import json
import os
from pathlib import Path
import sys


def finalize(folder, environment=None):
    env = os.environ if environment is None else environment
    path = folder / 'report.json'
    missing = not path.is_file()
    report = json.loads(path.read_text(encoding='utf-8')) if not missing else dict(
        schema=1, source_commit=env['VALIDATED_COMMIT'], source_ref=env['VALIDATION_REF'],
        lane=env['VALIDATION_LANE'], status='FAIL', checks=[], run_attempt=env['GITHUB_RUN_ATTEMPT'],
        run_url=f"https://github.com/{env['GITHUB_REPOSITORY']}/actions/runs/{env['GITHUB_RUN_ID']}")
    report['job_result'] = env['VALIDATION_RESULT']
    interrupted = (env['VALIDATION_RESULT'] == 'cancelled' and report['status'] != 'PASS'
                   and not any(item.get('status') == 'FAIL' for item in report['checks'])
                   and not report.get('error'))
    if interrupted:
        report.update(status='CANCELLED', error='Windows validation was cancelled before completion; partial checks do not qualify this commit')
    elif missing:
        report['error'] = 'Windows setup did not produce a validation report; see Actions job steps'
    path.write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
    if missing or interrupted:
        lines = [f"# Windows validation: {report['lane']}", '', f"Result: **{report['status']}**", '',
                 f"Source: `{report['source_commit']}`", '', report['error'], '', f"Run: {report['run_url']}", '']
        lines += [f"- {item['name']}: {item['status']}" for item in report['checks']]
        (folder / 'README.md').write_text('\n'.join(lines)+'\n', encoding='utf-8')
    return report


if __name__ == '__main__':
    finalize(Path(sys.argv[1]))
