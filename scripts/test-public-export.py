"""Exercise source-export privacy checks through the actual CLI and Git modes."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name('check-public-export.py')


class ExportPrivacy(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.base = Path(self.directory.name)
        self.repo = self.base / 'source'
        self.repo.mkdir()
        self.patterns = self.base / 'patterns.json'
        self.patterns.write_text(json.dumps([r'(?i)\bprivate-host\b']))
        self.report = self.base / 'report.json'
        self.git('init', '-b', 'main')

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.repo), '-c', 'user.name=Fixture',
            '-c', 'user.email=fixture@example.invalid', *args], text=True, stderr=subprocess.DEVNULL).strip()

    def scan(self, committed=False):
        command = [sys.executable, str(SCRIPT), '--patterns-file', str(self.patterns),
                   '--report', str(self.report)]
        if committed:
            command += ['--ref', 'HEAD']
        result = subprocess.run(command, cwd=self.repo, capture_output=True, text=True)
        return result, json.loads(self.report.read_text())

    def commit(self):
        self.git('add', '.')
        self.git('commit', '-m', 'Controlled synthetic export fixture')
        return self.git('rev-parse', 'HEAD')

    def test_private_binary_filename_rejected_without_reading_binary_text(self):
        name = 'private-host.ico'
        (self.repo / name).write_bytes(b'\0synthetic binary fixture')
        for committed in (False, True):
            if committed:
                original = self.commit()
            result, report = self.scan(committed)
            self.assertEqual(result.returncode, 1)
            self.assertEqual(len(report['findings']), 1)
            finding = report['findings'][0]
            self.assertEqual(finding['kind'], 'path')
            self.assertEqual(finding['path'], '[private].ico')
            self.assertEqual(finding['path_sha256'], hashlib.sha256(name.encode()).hexdigest())
            self.assertNotIn('private-host', result.stdout + result.stderr + self.report.read_text())
            if committed:
                self.assertEqual(self.git('rev-parse', 'HEAD'), original)

    def test_private_directory_rejected_with_innocent_file_contents(self):
        path = self.repo / 'private-host/notes.md'
        path.parent.mkdir()
        path.write_text('Ordinary product documentation.\n')
        original = self.commit()
        for committed in (False, True):
            result, report = self.scan(committed)
            self.assertEqual(result.returncode, 1)
            self.assertEqual(report['findings'][0]['path'], '[private]/notes.md')
            self.assertNotIn('private-host', result.stdout + result.stderr + self.report.read_text())
        self.assertEqual(self.git('rev-parse', 'HEAD'), original)

    def test_content_matches_still_fail_without_echoing_matched_text(self):
        (self.repo / 'notes.md').write_text('A private-host reference.\n')
        self.commit()
        for committed in (False, True):
            result, report = self.scan(committed)
            self.assertEqual(result.returncode, 1)
            self.assertEqual(report['findings'][0]['kind'], 'content')
            self.assertEqual(report['findings'][0]['line'], 1)
            self.assertNotIn('private-host', result.stdout + result.stderr + self.report.read_text())

    def test_benign_export_passes_both_modes(self):
        (self.repo / 'notes.md').write_text('Ordinary product documentation.\n')
        original = self.commit()
        for committed in (False, True):
            result, report = self.scan(committed)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(report['findings'], [])
            self.assertEqual(report['files_checked'], 1)
        self.assertEqual(self.git('rev-parse', 'HEAD'), original)


if __name__ == '__main__':
    unittest.main()
