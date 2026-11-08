#!/usr/bin/env python3
"""upstream-sync.py against throwaway origin/checkout/editable-source trees."""
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('upstream_sync', ROOT / 'scripts/upstream-sync.py')
upstream = importlib.util.module_from_spec(spec)
spec.loader.exec_module(upstream)


def run(cwd, *args):
    return subprocess.check_output(['git', '-C', str(cwd), *args], text=True).strip()


class UpstreamSyncTests(unittest.TestCase):
    def setUp(self):
        folder = tempfile.TemporaryDirectory(prefix='dpdf-upstream-test-')
        self.addCleanup(folder.cleanup)
        root = Path(folder.name)
        self.origin, self.checkout, self.github, self.source = (root / n for n in ('origin.git', 'checkout', 'github', 'source'))
        subprocess.check_call(['git', 'init', '-q', '--bare', '-b', 'main', str(self.origin)])
        for clone in (self.checkout, self.github):
            if clone == self.checkout:
                subprocess.check_call(['git', 'init', '-q', '-b', 'main', str(clone)])
                run(clone, 'remote', 'add', 'origin', str(self.origin))
            for key, value in (('user.email', 'ci@example.invalid'), ('user.name', 'CI')):
                if clone.exists():
                    run(clone, 'config', key, value)
        self.write(self.checkout, {'src/main.rs': 'fn main() {}\n', 'README.md': 'readme\n',
                                   'scripts/tool.sh': '#!/bin/sh\n', 'docs/old.md': 'old\n'})
        os.chmod(self.checkout / 'scripts/tool.sh', 0o755)
        self.commit(self.checkout, 'base')
        run(self.checkout, '-c', 'push.negotiate=false', 'push', '-q', 'origin', 'main')
        subprocess.check_call(['git', 'clone', '-q', str(self.origin), str(self.github)])
        run(self.github, 'config', 'user.email', 'ci@example.invalid')
        run(self.github, 'config', 'user.name', 'CI')
        # The editable source is a plain copy of the checkout's tracked files.
        shutil.copytree(self.checkout, self.source, ignore=shutil.ignore_patterns('.git'), symlinks=True)
        self.messages = []

    def write(self, tree, files):
        for name, content in files.items():
            path = tree / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding='utf-8')

    def commit(self, tree, message):
        run(tree, 'add', '-A')
        run(tree, 'commit', '-q', '-m', message)
        return run(tree, 'rev-parse', 'HEAD')

    def merge_on_github(self):
        self.write(self.github, {'src/main.rs': 'fn main() { println!("merged"); }\n',
                                 'src/new.rs': 'pub fn added() {}\n'})
        (self.github / 'docs/old.md').unlink()
        os.chmod(self.github / 'scripts/tool.sh', 0o644)
        merged = self.commit(self.github, 'Merged pull request')
        run(self.github, '-c', 'push.negotiate=false', 'push', '-q', 'origin', 'main')
        return merged

    def sync(self, **options):
        return upstream.sync(self.source, self.checkout, out=self.messages.append, **options)

    def test_merged_changes_reach_checkout_and_source(self):
        merged = self.merge_on_github()
        self.write(self.source, {'README.md': 'local unrelated edit\n'})  # Unrelated local work survives.
        self.sync()
        self.assertEqual(run(self.checkout, 'rev-parse', 'HEAD'), merged)
        self.assertIn('merged', (self.source / 'src/main.rs').read_text())
        self.assertTrue((self.source / 'src/new.rs').is_file())
        self.assertFalse((self.source / 'docs').exists())
        self.assertFalse(os.stat(self.source / 'scripts/tool.sh').st_mode & 0o111)
        self.assertEqual((self.source / 'README.md').read_text(), 'local unrelated edit\n')
        self.assertEqual(self.sync(), [])  # Idempotent.

    def test_conflicting_local_edit_refuses_without_writing(self):
        self.merge_on_github()
        self.write(self.source, {'src/main.rs': 'fn main() { local(); }\n'})
        before = run(self.checkout, 'rev-parse', 'HEAD')
        with self.assertRaises(upstream.Refused) as raised:
            self.sync()
        self.assertIn('src/main.rs', str(raised.exception))
        self.assertEqual(run(self.checkout, 'rev-parse', 'HEAD'), before)
        self.assertFalse((self.source / 'src/new.rs').exists())
        self.assertTrue((self.source / 'docs/old.md').exists())

    def test_interrupted_update_completes_on_rerun(self):
        merged = self.merge_on_github()
        self.write(self.source, {'src/main.rs': 'fn main() { println!("merged"); }\n'})  # Partially applied.
        self.sync()
        self.assertEqual(run(self.checkout, 'rev-parse', 'HEAD'), merged)
        self.assertTrue((self.source / 'src/new.rs').is_file())

    def test_dirty_or_diverged_checkout_is_refused(self):
        self.merge_on_github()
        self.write(self.checkout, {'README.md': 'uncommitted\n'})
        with self.assertRaises(upstream.Refused):
            self.sync()
        run(self.checkout, 'checkout', '-q', '--', 'README.md')
        self.write(self.checkout, {'README.md': 'local commit\n'})
        self.commit(self.checkout, 'Unpushed local commit')
        with self.assertRaises(upstream.Refused) as raised:
            self.sync()
        self.assertIn('diverged', str(raised.exception))

    def test_dry_run_and_up_to_date(self):
        self.assertEqual(self.sync(), [])
        self.merge_on_github()
        before = run(self.checkout, 'rev-parse', 'HEAD')
        pending = self.sync(dry_run=True)
        self.assertEqual({path for path, _, _ in pending}, {'src/main.rs', 'src/new.rs', 'docs/old.md', 'scripts/tool.sh'})
        self.assertEqual(run(self.checkout, 'rev-parse', 'HEAD'), before)
        self.assertNotIn('merged', (self.source / 'src/main.rs').read_text())

    def test_unsafe_paths_are_rejected(self):
        for value in ('../outside', '/etc/passwd', '.git/config', 'a/../../b'):
            with self.assertRaises(upstream.Refused):
                upstream.safe_relative(value)


if __name__ == '__main__':
    unittest.main()
