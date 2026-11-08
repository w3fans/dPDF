"""Publish an explicit pending receipt; it never implies Windows acceptance."""
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import sys

folder = Path(sys.argv[1])
report = dict(schema=1, source_commit=os.environ['VALIDATED_COMMIT'],
              source_ref=os.environ['VALIDATION_REF'], lane=os.environ['VALIDATION_LANE'],
              status='QUEUED', checks=[], run_attempt=os.environ['GITHUB_RUN_ATTEMPT'], reported_at=datetime.now(timezone.utc).isoformat(),
              run_url=f"https://github.com/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{os.environ['GITHUB_RUN_ID']}")
(folder / 'report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
(folder / 'README.md').write_text(
    f"# Windows validation: {report['lane']}\n\nResult: **QUEUED**\n\n"
    f"Source: `{report['source_ref']}` / `{report['source_commit']}`\n\n"
    f"Receipt time: {report['reported_at']}\n\n"
    f"Run: {report['run_url']}\n\n"
    'No Windows checks have passed in this receipt. The final report will update this branch.\n', encoding='utf-8')
