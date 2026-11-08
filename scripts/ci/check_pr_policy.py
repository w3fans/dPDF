#!/usr/bin/env python3
"""Pull-request repository policy: no agent/private files, no release metadata edits.

Development pull requests never change version, tag or release-note metadata; the
owner's release checkout cuts previews and stable milestones with ./release.sh.
Every pull request records its change under the CHANGELOG `## Unreleased` heading.
"""
import argparse
import re
import os
import subprocess
import tomllib

# Agent-specific folders and instruction files are never repository content.
AGENT_DIRECTORIES = {'.agents', '.claude', '.codex', '.kimi', '.cursor', '.windsurf', '.aider'}
AGENT_FILES = {'agents.md', 'claude.md', 'kimi.md', 'codex.md', 'gemini.md', '.cursorrules',
               '.windsurfrules', '.aider.conf.yml'}
# Private local data and credentials (mirrors .gitignore; a forced add still fails).
PRIVATE_DIRECTORY = re.compile(r'^(asset-.*|\.import-.*|recovery|\.session-.*|\.aws|\.ssh|\.creditentials)$')
PRIVATE_FILE = re.compile(r'(^\.env$|^\.env\..*|\.env$|\.dpdf$|\.pem$|\.key$|\.p12$|\.pfx$|\.kdbx$|'
                          r'^\.checkpoint-.*|^SECRETS\.local\.md$|^credentials\.local\.md$|\.secret\.md$)')
PRIVATE_PATHS = {'ops/project.env', 'ops/runner.yaml'}
RELEASE_FILES = ('release.json',)
RELEASE_PREFIXES = ('docs/releases/',)


def git(*args, binary=False):
    output = subprocess.check_output(['git', *args])
    return output if binary else output.decode('utf-8')


def show(commit, path):
    try:
        return git('show', f'{commit}:{path}')
    except subprocess.CalledProcessError:
        return None


def path_problems(paths):
    problems = []
    for path in paths:
        parts = path.split('/')
        name = parts[-1]
        if any(part.lower() in AGENT_DIRECTORIES for part in parts[:-1]) or name.lower() in AGENT_FILES:
            problems.append(f'{path}: agent-specific files are not repository content')
        elif path in PRIVATE_PATHS or any(PRIVATE_DIRECTORY.match(part) for part in parts[:-1]) \
                or PRIVATE_FILE.search(name):
            problems.append(f'{path}: private local data or credentials must not be committed')
    return problems


def package_version(cargo_toml):
    return tomllib.loads(cargo_toml)['package']['version'] if cargo_toml else None


def lock_version(cargo_lock):
    if not cargo_lock:
        return None
    return [p['version'] for p in tomllib.loads(cargo_lock).get('package', []) if p['name'] == 'dpdf']


def readme_version(readme):
    match = re.search(r'Current version: (\S+) ', readme or '')
    return match[1] if match else None


def changelog_parts(changelog):
    """Return (Unreleased body, every versioned section after it)."""
    match = re.search(r'(?ms)^## Unreleased[ \t]*\n(.*?)(?=^## |\Z)', changelog or '')
    if not match:
        return None, changelog
    return match[1], changelog[match.end():]


def redaction_only(before, after):
    """Permit explicit privacy removals in history, without rewriting release facts."""
    if before is None or after is None or before == after or '[private]' not in after:
        return False
    # Versions remain immutable even when a release's prose is sanitized.
    versions = r'\b(?:v)?\d+\.\d+\.\d+\b'
    if re.findall(versions, before) != re.findall(versions, after):
        return False
    # Every unchanged span must remain byte-for-byte; each marker may replace
    # only a short piece of text on the same line, never a paragraph or section.
    pattern = r'[^\r\n]{1,256}?'.join(re.escape(part) for part in after.split('[private]'))
    return re.fullmatch(pattern, before) is not None


def check(base, head):
    problems = path_problems(git('ls-tree', '-r', '--name-only', '-z', head).split('\0')[:-1])
    changed = [p for p in git('diff', '--name-only', '--no-renames', '-z', base, head).split('\0')[:-1]]
    for path in changed:
        if path in RELEASE_FILES or (path.startswith(RELEASE_PREFIXES)
                                     and not redaction_only(show(base, path), show(head, path))):
            problems.append(f'{path}: release metadata changes only through ./release.sh in the release checkout')
    before, after = (show(base, 'Cargo.toml'), show(head, 'Cargo.toml'))
    if package_version(before) != package_version(after):
        problems.append('Cargo.toml: the package version is set by the release tooling, not by pull requests')
    if lock_version(show(base, 'Cargo.lock')) != lock_version(show(head, 'Cargo.lock')):
        problems.append('Cargo.lock: the dpdf package version is set by the release tooling')
    if readme_version(show(base, 'README.md')) != readme_version(show(head, 'README.md')):
        problems.append('README.md: the "Current version" line is set by the release tooling')
    old_unreleased, old_history = changelog_parts(show(base, 'CHANGELOG.md'))
    new_unreleased, new_history = changelog_parts(show(head, 'CHANGELOG.md'))
    if new_unreleased is None:
        problems.append('CHANGELOG.md: keep the "## Unreleased" heading at the top')
    elif old_history != new_history and not redaction_only(old_history, new_history):
        problems.append('CHANGELOG.md: released sections are history; add entries under "## Unreleased" only')
    elif changed and (new_unreleased.strip() == (old_unreleased or '').strip()
                      or not re.search(r'(?m)^- \S', new_unreleased)):
        problems.append('CHANGELOG.md: describe this change under "## Unreleased"')
    return problems


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', required=True, help='Pull-request base commit')
    parser.add_argument('--head', default='HEAD')
    args = parser.parse_args()
    base = git('merge-base', args.base, args.head).strip()
    problems = check(base, args.head)
    for problem in problems:
        print(f'::error::{problem}' if os.environ.get('GITHUB_ACTIONS') == 'true' else problem)
    if problems:
        raise SystemExit(f'{len(problems)} pull-request policy problem(s)')
    print('Pull-request policy passed')


if __name__ == '__main__':
    main()
