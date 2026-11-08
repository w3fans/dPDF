"""Local publication guards: existing tags only, clean main history and receipts."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import local_copr


class Publication(unittest.TestCase):
    def test_only_existing_preview_tag_shape_is_accepted(self):
        for value in ['v0.2.0', 'main', '../v0.1.46', 'v0.1.100', 'v0.1.46;true']:
            with self.subTest(value=value), self.assertRaises(ValueError):
                local_copr.tag_number(value)
        self.assertEqual(local_copr.tag_number('v0.1.46'), 46)

    def test_clean_checkout_identity_and_main_ancestry(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo = root / 'repo'
            repo.mkdir()
            def git(*args):
                return subprocess.check_output(['git', '-C', str(repo), '-c', 'user.name=Fixture',
                    '-c', 'user.email=fixture@example.invalid', *args], text=True, stderr=subprocess.DEVNULL).strip()
            git('init', '-b', 'main')
            (repo / 'source').write_text('reviewed')
            git('add', 'source')
            git('commit', '-m', 'controlled fixture')
            sha = git('rev-parse', 'HEAD')
            git('tag', 'v0.1.46')
            git('update-ref', 'refs/remotes/origin/main', sha)
            args = argparse.Namespace(repository=repo, worktrees=root / 'worktrees')
            args.worktrees.mkdir()
            checkout, actual = local_copr.source_checkout(args, 'v0.1.46')
            self.assertEqual(actual, sha)
            self.assertEqual((checkout / 'source').read_text(), 'reviewed')
            (checkout / 'source').write_text('uncommitted change')
            with self.assertRaisesRegex(ValueError, 'cleanliness'):
                local_copr.source_checkout(args, 'v0.1.46')

    def test_receipt_and_checksum_changes_invalidate_completed_stamp(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ['copr-release.json', 'SHA256SUMS-fedora.txt']:
                (root / name).write_text('original')
            before = local_copr.receipt_stamp(root)
            (root / 'copr-release.json').write_text(json.dumps({'status': 'changed'}))
            self.assertNotEqual(before, local_copr.receipt_stamp(root))


if __name__ == '__main__':
    unittest.main()
