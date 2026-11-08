#!/usr/bin/env python3
"""Project version-file adapter; the installed 0rig controller calculates versions."""
import argparse
from datetime import date
import json
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def require(condition, message):
    if not condition:
        raise ValueError(message)


def version():
    return tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))['package']['version']


def verify(tag=None, binary=None):
    manifest = json.loads((ROOT / 'release.json').read_text(encoding='utf-8'))
    package = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))['package']
    current = package['version']
    if not re.fullmatch(r'0\.1\.\d+', current):
        raise ValueError('This preview adapter only releases 0.1.x; stable milestones need qualification')
    require(re.fullmatch(r'0\.1\.\d{1,2}', current), 'Three-digit preview patch numbers are not used; ask the owner')
    expected = 'v' + current
    require(manifest['version'] == current and manifest['tag'] == expected, 'Release manifest/package mismatch')
    require(manifest['channel'] == 'development-preview', 'Only development previews are configured')
    require(tag is None or tag == expected, 'Pushed tag does not match the package')
    packages = tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding='utf-8'))['package']
    require([p['version'] for p in packages if p['name'] == 'dpdf'] == [current], 'Cargo.lock version mismatch')
    require(f'Current version: {current} ' in (ROOT / 'README.md').read_text(encoding='utf-8'), 'README version mismatch')
    require(f'## {current} - ' in (ROOT / 'CHANGELOG.md').read_text(encoding='utf-8'), 'Missing versioned changelog')
    require(manifest['notes'] == f'docs/releases/{expected}.md', 'Unexpected release-notes path')
    require((ROOT / manifest['notes']).is_file(), 'Missing release notes')
    if package.get('license') == 'Apache-2.0':
        require((ROOT / 'LICENSE').is_file() and (ROOT / 'NOTICE').is_file(), 'Missing product license/notice')
        require('Apache License' in (ROOT / 'LICENSE').read_text(encoding='utf-8'), 'Unexpected product license text')
        require((ROOT / 'packaging/licenses/slint/LicenseRef-Slint-Royalty-free-2.0.md').is_file(), 'Missing Slint desktop terms')
        require((ROOT / 'packaging/licenses/slint/GPL-3.0-only.txt').is_file(), 'Missing selected Slint GPL terms')
        for page in ['README.md', manifest['notes']]:
            require('MadeWithSlint-logo-whitebg.png' in (ROOT / page).read_text(encoding='utf-8'), 'Missing public Slint attribution')
    if binary:
        output = subprocess.check_output([str(binary.resolve()), '--version'], text=True)
        require(output.startswith(f'dPDF {current} '), 'Built executable version mismatch')
    return manifest


def prepare():
    pending = ROOT / '.agents/runtime/preview-pending.json'
    if pending.exists():
        previous = json.loads(pending.read_text(encoding='utf-8'))
        if previous['version'] != version():
            raise ValueError('Pending preview does not match the package version')
        return verify()
    current = version()
    policy = ROOT / '.agents/release-policy.yaml'
    target = subprocess.check_output([str(ROOT / 'release.sh'), 'preflight', 'patch', current,
                                     str(policy), str(ROOT)], text=True).strip()
    subprocess.run([str(ROOT / 'release.sh'), 'validate-result', 'patch', current, target, str(policy)], check=True)
    if not re.fullmatch(r'v0\.1\.\d+', target):
        raise ValueError('The next patch crosses the preview milestone boundary')
    new = target[1:]
    cargo = (ROOT / 'Cargo.toml').read_text(encoding='utf-8')
    cargo, count = re.subn(r'(?m)^version = "' + re.escape(current) + r'"$', f'version = "{new}"', cargo, count=1)
    require(count == 1, 'Expected exactly one package version to synchronize')
    lock = (ROOT / 'Cargo.lock').read_text(encoding='utf-8')
    lock, count = re.subn(r'(\[\[package\]\]\nname = "dpdf"\nversion = ")' + re.escape(current) + '"',
                          lambda match: match[1] + new + '"', lock)
    require(count == 1, 'Expected exactly one package version to synchronize')
    readme = (ROOT / 'README.md').read_text(encoding='utf-8').replace(f'Current version: {current} ', f'Current version: {new} ', 1)
    changelog = (ROOT / 'CHANGELOG.md').read_text(encoding='utf-8')
    section = re.search(r'(?ms)^## Unreleased\s*\n(.*?)(?=^## |\Z)', changelog)
    require(section and re.search(r'(?m)^- \S', section[1]), 'A patch release needs an Unreleased changelog entry')
    changes = section[1].strip()
    archived = f'## Unreleased\n\n## {new} - {date.today()} - Development preview\n\n{changes}\n\n'
    changelog = changelog[:section.start()] + archived + changelog[section.end():]
    notes = (f'# dPDF {target} — development preview\n\n'
             'This is a development preview. v0.2.0 remains reserved for the first stable reader.\n\n'
             f'{changes}\n\n'
             '## Validation and remaining work\n\n'
             'See docs/VALIDATION.md and docs/validation/ for exact test evidence.\n'
             'Physical printer/Windows test machine Windows 11 qualification, full desktop acceptance and broader interoperability,\n'
             'secure region baking and cross-platform review qualification remain unfinished.\n'
             'Source archives and build instructions accompany this preview. Fedora 44/45 RPMs are published only after\n'
             'successful source builds and package verification; consult the release assets and\n'
             '[COPR project](https://copr.fedorainfracloud.org/coprs/zvegelj/dpdf/) for available packages.\n'
             'Check the package source commit and validation record before treating a build as qualified.\n\n'
             'dPDF-owned source: Apache-2.0. The combined application uses Slint under GPL-3.0-only;\n'
             'third-party components retain their own licenses and notices. See docs/LICENSING.md.\n\n'
             '[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/v1.18.1/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev)\n')
    manifest = {'schema_version': 1, 'version': new, 'tag': target, 'previous_version': current,
                'channel': 'development-preview', 'notes': f'docs/releases/{target}.md'}
    outputs = {'Cargo.toml': cargo, 'Cargo.lock': lock, 'README.md': readme, 'CHANGELOG.md': changelog,
               manifest['notes']: notes, 'release.json': json.dumps(manifest, indent=2) + '\n'}
    for name, content in outputs.items():
        path = ROOT / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding='utf-8')
    pending.parent.mkdir(parents=True, exist_ok=True)
    pending.write_text(json.dumps({'version': new}) + '\n')
    return verify()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['prepare', 'verify'])
    parser.add_argument('--tag')
    parser.add_argument('--binary', type=Path)
    args = parser.parse_args()
    result = prepare() if args.command == 'prepare' else verify(args.tag, args.binary)
    print(result['tag'])
