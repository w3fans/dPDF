#!/usr/bin/env python3
"""Source-only release-adapter checks using the real portable standard controller."""
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('release_version', ROOT / 'scripts/release-version.py')
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseVersionTests(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory(prefix='dpdf-release-test-')
        self.addCleanup(self.folder.cleanup)
        self.root = Path(self.folder.name)
        release.ROOT = self.root
        for relative in ('release.sh', '.agents/bin/release-controller', '.agents/release-policy.yaml'):
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, target)
        (self.root / 'Cargo.toml').write_text('[package]\nname = "dpdf"\nversion = "0.1.0"\n')
        (self.root / 'Cargo.lock').write_text('version = 4\n\n[[package]]\nname = "dpdf"\nversion = "0.1.0"\n')
        (self.root / 'README.md').write_text('**Current version: 0.1.0 — development preview.**\n', encoding='utf-8')
        (self.root / 'CHANGELOG.md').write_text('# Changelog\n\n## Unreleased\n\n- Fix a Windows export failure.\n\n## 2026-10-28\n\n- Original history.\n')

    def test_standard_patch_synchronizes_files_and_is_resumable(self):
        prepared = release.prepare()
        self.assertEqual(prepared['tag'], 'v0.1.1')
        self.assertEqual(release.prepare(), prepared)
        self.assertEqual(release.verify('v0.1.1'), prepared)
        changelog = (self.root / 'CHANGELOG.md').read_text()
        self.assertEqual(changelog.count('- Fix a Windows export failure.'), 1)
        self.assertIn('## Unreleased\n\n## 0.1.1 - ', changelog)
        self.assertIn('## 2026-10-28', changelog)
        self.assertTrue((self.root / prepared['notes']).is_file())

    def test_notes_describe_conditional_packages_and_combined_license(self):
        prepared = release.prepare()
        notes = (self.root / prepared['notes']).read_text()
        self.assertIn('Fedora 44/45 RPMs are published only after', notes)
        self.assertIn('Check the package source commit and validation record', notes)
        self.assertIn('dPDF-owned source: Apache-2.0', notes)
        self.assertIn('combined application uses Slint under GPL-3.0-only', notes)
        self.assertNotIn('no qualified installer is attached', notes)
        self.assertIn('MadeWithSlint-logo-whitebg.png', notes)

    def test_mismatched_versions_and_tags_fail_closed(self):
        release.prepare()
        with self.assertRaises(ValueError):
            release.verify('v0.2.0')
        (self.root / 'Cargo.lock').write_text('version = 4\n\n[[package]]\nname = "dpdf"\nversion = "0.1.0"\n')
        with self.assertRaises(ValueError):
            release.verify()
        manifest = json.loads((self.root / 'release.json').read_text())
        self.assertEqual(manifest['tag'], 'v0.1.1')

    def set_version(self, value):
        for relative in ('Cargo.toml', 'Cargo.lock', 'README.md'):
            target = self.root / relative
            target.write_text(target.read_text().replace('0.1.0', value))

    def test_patch_after_99_never_releases(self):
        self.set_version('0.1.99')
        with self.assertRaisesRegex(ValueError, 'milestone boundary'):
            release.prepare()
        self.assertIn('version = "0.1.99"', (self.root / 'Cargo.toml').read_text())

    def test_three_digit_patch_numbers_fail_closed(self):
        self.set_version('0.1.100')
        (self.root / 'release.json').write_text(json.dumps({'version': '0.1.100', 'tag': 'v0.1.100'}))
        with self.assertRaisesRegex(ValueError, 'Three-digit'):
            release.verify()

    def test_license_and_public_attribution_are_required_for_apache_preview(self):
        with (self.root / 'Cargo.toml').open('a') as stream:
            stream.write('license = "Apache-2.0"\n')
        with self.assertRaisesRegex(ValueError, 'Missing product license/notice'):
            release.prepare()
        for relative in ['LICENSE', 'NOTICE', 'packaging/licenses/slint/LicenseRef-Slint-Royalty-free-2.0.md',
                         'packaging/licenses/slint/GPL-3.0-only.txt']:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, target)
        with self.assertRaisesRegex(ValueError, 'Missing public Slint attribution'):
            release.verify()
        with (self.root / 'README.md').open('a') as stream:
            stream.write('MadeWithSlint-logo-whitebg.png\n')
        self.assertEqual(release.verify()['tag'], 'v0.1.1')


if __name__ == '__main__':
    unittest.main()
