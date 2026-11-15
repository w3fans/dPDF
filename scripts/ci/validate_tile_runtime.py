"""Run the draft's native tile oracle using the unchanged base Windows Job Object adapter."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--harness', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    source, output = args.source.resolve(), args.report.resolve()
    if os.name != 'nt':
        raise RuntimeError('Use the existing hosted native Windows runner')
    sys.path.insert(0, str(args.harness.resolve() / 'scripts' / 'ci'))
    from validate_windows import environment, write_report
    from windows_job import run
    from runtime import prepare
    report = json.loads((output / 'report.json').read_text(encoding='utf-8'))
    if report['status'] != 'PASS' or report['source_commit'] != args.commit:
        raise RuntimeError('Require successful base native checks for this exact source')
    if subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip() != args.commit:
        raise RuntimeError('Candidate source differs from the native report')
    command = next(check['command'] for check in report['checks'] if check['name'] == 'reader' and check['status'] == 'PASS')
    binary = Path(command[command.index('--binary') + 1])
    digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    if digest(binary) != report['binary_sha256']:
        raise RuntimeError('Candidate executable differs from the base native checks')
    if any(check['name'] == 'render-tiles' for check in report['checks']):
        print('Native tile oracle already passed under the base validation plan')
        return 0
    started = time.monotonic()
    log = output / 'logs' / 'render-tiles.log'
    check = dict(name='render-tiles', status='FAIL')
    report['checks'].append(check)
    try:
        with tempfile.TemporaryDirectory(prefix='dpdf-native-tile-oracle-') as directory:
            root = Path(directory)
            env = environment(root, root / 'unused-target')
            runtimes = prepare(root / 'runtime')
            if runtimes['pdfium_archive_sha256'] != report['runtimes']['pdfium_archive_sha256']:
                raise RuntimeError('Native tile oracle changed the validated PDFium archive')
            check['pdfium_sha256'] = digest(Path(runtimes['pdfium']))
            command = [sys.executable, str(source / 'scripts' / 'check-render-tiles.py'),
                       '--binary', str(binary), '--pdfium', runtimes['pdfium']]
            check['command'] = command
            code = run(command, source, env, log, 600)
            check['exit_code'] = code
            if code != 0:
                raise RuntimeError('Native tile oracle failed; see retained render-tiles.log')
            if digest(binary) != report['binary_sha256']:
                raise RuntimeError('Candidate executable changed during native tile checks')
            if subprocess.check_output(['git', '-C', str(source), 'status', '--porcelain', '--untracked-files=all'], text=True).strip():
                raise RuntimeError('Native tile checks changed candidate source')
            check['status'] = 'PASS'
    except Exception as error:
        check['error'] = str(error)
        report['status'] = 'FAIL'
    finally:
        check['seconds'] = round(time.monotonic() - started, 3)
        report['finished_at'] = datetime.now(timezone.utc).isoformat()
        write_report(report, output)
        if log.exists():
            print(log.read_text(encoding='utf-8', errors='replace')[-4000:])
        print(json.dumps(check))
    return 0 if check['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
