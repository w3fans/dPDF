#!/usr/bin/env python3
"""Scan a product export using a private, external denylist; never publish the patterns."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess


def matches(text, patterns):
    return [(line, index + 1) for line, value in enumerate(text.splitlines(), 1)
            for index, pattern in enumerate(patterns) if pattern.search(value)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--patterns-file', type=Path, required=True,
                        help='Private JSON list of regular expressions; keep outside tracked source')
    parser.add_argument('--ref', help='Inspect a committed tree instead of current source files')
    parser.add_argument('--report', type=Path, help='Write paths/line numbers only, never matched values')
    args = parser.parse_args()
    patterns = [re.compile(pattern) for pattern in json.loads(args.patterns_file.read_text())]
    if not patterns:
        raise ValueError('Provide the actual private publication denylist')
    if args.ref:
        ref = subprocess.check_output(['git', 'rev-parse', '--verify', args.ref + '^{commit}'], text=True).strip()
        names = subprocess.check_output(['git', 'ls-tree', '-rz', '--name-only', ref]).decode().split('\0')[:-1]
    else:
        names = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z']).decode().split('\0')[:-1]
    findings = []
    for name in sorted(set(names)):
        display = name
        for pattern in patterns:
            display = pattern.sub('[private]', display)
        location = dict(path=display)
        if display != name:
            location['path_sha256'] = hashlib.sha256(name.encode()).hexdigest()
        for index, pattern in enumerate(patterns, 1):
            if pattern.search(name):
                findings.append(dict(location, line=0, pattern=index, kind='path'))
        try:
            content = subprocess.check_output(['git', 'show', ref + ':' + name]) if args.ref else Path(name).read_bytes()
            if b'\0' in content:
                continue
            content = content.decode('utf-8')
        except (UnicodeError, FileNotFoundError):
            continue
        for line, pattern in matches(content, patterns):
            findings.append(dict(location, line=line, pattern=pattern, kind='content'))
    report = dict(scope=args.ref or 'working tree', files_checked=len(set(names)), findings=findings,
                  historical_commits_rewritten=False)
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + '\n')
    for finding in findings:
        print(f"{finding['path']}:{finding['line']}: private pattern #{finding['pattern']} ({finding['kind']})")
    print(f"Checked {report['files_checked']} product files; {len(findings)} private-reference findings.")
    raise SystemExit(bool(findings))


if __name__ == '__main__':
    main()
