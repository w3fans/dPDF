#!/usr/bin/env python3
"""Pull-request policy checks against throwaway Git repositories."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_pr_policy

CARGO = '[package]\nname = "dpdf"\nversion = "0.1.43"\n'
LOCK = 'version = 4\n\n[[package]]\nname = "dpdf"\nversion = "0.1.43"\n'
README = '**Current version: 0.1.43 — reader development preview.**\n'
CHANGELOG = '# Changelog\n\n## Unreleased\n\n## 0.1.43 - 2026-11-03 - Development preview\n\n- Old entry.\n'


class PolicyTests(unittest.TestCase):
    def setUp(self):
        folder = tempfile.TemporaryDirectory(prefix='dpdf-policy-test-')
        self.addCleanup(folder.cleanup)
        self.root = Path(folder.name)
        self.previous = os.getcwd()
        os.chdir(self.root)
        self.addCleanup(os.chdir, self.previous)
        self.git('init', '-q', '-b', 'main')
        self.git('config', 'user.email', 'ci@example.invalid')
        self.git('config', 'user.name', 'CI')
        self.write({'Cargo.toml': CARGO, 'Cargo.lock': LOCK, 'README.md': README, 'CHANGELOG.md': CHANGELOG,
                    'release.json': '{}\n', 'src/main.rs': 'fn main() {}\n'})
        self.base = self.commit()

    def git(self, *args):
        return subprocess.check_output(['git', *args], text=True).strip()

    def write(self, files):
        for name, content in files.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding='utf-8')

    def commit(self):
        self.git('add', '-A', '-f')
        self.git('commit', '-q', '-m', 'change')
        return self.git('rev-parse', 'HEAD')

    def problems(self, files):
        self.write(files)
        return check_pr_policy.check(self.base, self.commit())

    def entry(self, text='- Fix a reader defect.'):
        return CHANGELOG.replace('## Unreleased\n', f'## Unreleased\n\n{text}\n')

    def test_ordinary_change_with_unreleased_entry_passes(self):
        self.assertEqual(self.problems({'src/main.rs': 'fn main() { }\n', 'CHANGELOG.md': self.entry()}), [])

    def test_missing_unreleased_entry_fails(self):
        problems = self.problems({'src/main.rs': 'fn main() { }\n'})
        self.assertTrue(any('Unreleased' in p for p in problems), problems)

    def test_editing_released_history_fails(self):
        changed = self.entry().replace('- Old entry.', '- Rewritten entry.')
        problems = self.problems({'src/main.rs': '// x\n', 'CHANGELOG.md': changed})
        self.assertTrue(any('released sections' in p for p in problems), problems)

    def test_version_and_release_metadata_are_refused(self):
        problems = self.problems({'CHANGELOG.md': self.entry(), 'release.json': '{"v": 1}\n',
                                  'Cargo.toml': CARGO.replace('0.1.43', '0.1.44'),
                                  'Cargo.lock': LOCK.replace('0.1.43', '0.1.44'),
                                  'README.md': README.replace('0.1.43', '0.2.0'),
                                  'docs/releases/v0.1.44.md': '# notes\n'})
        for fragment in ('release.json', 'Cargo.toml', 'Cargo.lock', 'README.md', 'docs/releases/v0.1.44.md'):
            self.assertTrue(any(p.startswith(fragment) for p in problems), (fragment, problems))

    def test_agent_and_private_files_are_refused(self):
        files = {'CHANGELOG.md': self.entry(), '.claude/settings.json': '{}', 'docs/AGENTS.md': 'x',
                 'CLAUDE.md': 'x', 'tools/.cursor/rules': 'x', 'ops/project.env': 'X=1',
                 'data/recovery/draft.json': '{}', 'contract.dpdf': 'x', 'keys/signing.pem': 'x',
                 'marks/asset-1234/original.png': 'x'}
        problems = self.problems(files)
        for name in files:
            if name != 'CHANGELOG.md':
                self.assertTrue(any(p.startswith(name + ':') for p in problems), (name, problems))

    def test_neutral_names_are_not_false_positives(self):
        files = {'CHANGELOG.md': self.entry(), 'docs/validation/asset-lock-baseline.json': '{}',
                 'src/recovery.rs': '// recovery module\n', '.github/workflows/ci.yml': 'on: push\n',
                 '.cargo/config.toml': '[build]\n'}
        self.assertEqual(self.problems(files), [])


if __name__ == '__main__':
    unittest.main()
