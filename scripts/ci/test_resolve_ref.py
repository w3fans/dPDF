"""Exercise Windows source admission against a real disposable Git remote."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

RESOLVER = Path(__file__).with_name('resolve_ref.py').resolve()

class TrustedSource(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='dpdf-ref-trust-')
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.remote = self.root / 'remote.git'
        self.source = self.root / 'source'
        self.runner = self.root / 'runner'
        self.git('init', '--bare', str(self.remote))
        self.git('init', '-b', 'main', str(self.source))
        self.git('-C', str(self.source), 'config', 'user.name', 'Synthetic test')
        self.git('-C', str(self.source), 'config', 'user.email', 'synthetic@example.invalid')
        self.git('-C', str(self.source), 'remote', 'add', 'origin', str(self.remote))
        self.first = self.commit('first')
        self.main = self.commit('second')
        self.git('-C', str(self.source), 'tag', 'v0.1.46', self.main)
        self.git('-C', str(self.source), 'push', 'origin', 'main', '--tags')
        self.git('-C', str(self.source), 'checkout', '-b', 'unreviewed')
        self.unreviewed = self.commit('unreviewed')
        self.git('-C', str(self.source), 'push', 'origin', 'unreviewed')
        self.git('--git-dir', str(self.remote), 'symbolic-ref', 'HEAD', 'refs/heads/main')
        self.git('clone', '--depth=1', self.remote.as_uri(), str(self.runner))

    def git(self, *args):
        return subprocess.check_output(['git', '-c', 'core.hooksPath=/dev/null', *args], stderr=subprocess.DEVNULL, text=True).strip()

    def commit(self, text):
        (self.source / 'fixture.txt').write_text(text)
        self.git('-C', str(self.source), 'add', 'fixture.txt')
        self.git('-C', str(self.source), 'commit', '-m', text)
        return self.git('-C', str(self.source), 'rev-parse', 'HEAD')

    def resolve(self, requested, event_sha=None):
        output = self.root / 'github-output'
        env = os.environ.copy()
        env.update(REQUESTED_REF=requested, GITHUB_REF='refs/heads/main', GITHUB_SHA=event_sha or self.main, GITHUB_OUTPUT=str(output))
        result = subprocess.run([sys.executable, str(RESOLVER)], cwd=self.runner, env=env, text=True, capture_output=True)
        return result, output.read_text() if output.exists() else ''

    def test_unreviewed_full_sha_is_rejected_without_worker_output(self):
        result, output = self.resolve(self.unreviewed)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('not in approved main history', result.stderr)
        self.assertEqual(output, '')

    def test_old_approved_sha_is_admitted_from_shallow_checkout(self):
        result, output = self.resolve(self.first)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('commit=' + self.first, output)

    def test_release_tag_remains_bound_to_its_exact_commit(self):
        result, output = self.resolve('v0.1.46')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('commit=' + self.main, output)
        self.assertIn('ref=v0.1.46', output)

    def test_push_uses_event_commit_when_main_has_advanced(self):
        result, output = self.resolve('', self.first)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('commit=' + self.first, output)
        self.assertNotIn('commit=' + self.main, output)

if __name__ == '__main__':
    unittest.main()
