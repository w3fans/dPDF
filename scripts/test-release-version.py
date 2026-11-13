#!/usr/bin/env python3
"""Public adapter unit checks; opt-in installed-controller integration uses temporary files."""
import argparse
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

INTEGRATION_POLICY = None

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('release_version', ROOT / 'scripts/release-version.py')
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseVersionTests(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory(prefix='dpdf-release-test-')
        self.addCleanup(self.folder.cleanup)
        self.root = Path(self.folder.name)
        self.addCleanup(setattr, release, 'ROOT', release.ROOT)
        release.ROOT = self.root
        if INTEGRATION_POLICY is None:
            self.preflight = self.enterContext(patch.object(release.subprocess, 'check_output', return_value='v0.1.1\n'))
            self.validate = self.enterContext(patch.object(release.subprocess, 'run'))
        else:
            policy = self.root / 'ops/release-policy.json'
            policy.parent.mkdir(parents=True)
            shutil.copy2(INTEGRATION_POLICY, policy)
        for relative in ('release.sh',):
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / relative, target)
        (self.root / 'Cargo.toml').write_text('[package]\nname = "dpdf"\nversion = "0.1.0"\n')
        (self.root / 'Cargo.lock').write_text('version = 4\n\n[[package]]\nname = "dpdf"\nversion = "0.1.0"\n')
        (self.root / 'README.md').write_text('**Current version: 0.1.0 — development preview.**\n', encoding='utf-8')
        (self.root / 'CHANGELOG.md').write_text('# Changelog\n\n## Unreleased\n\n- Fix a Windows export failure.\n\n## 2026-10-28\n\n- Original history.\n')

    def test_standard_patch_synchronizes_files_and_is_resumable(self):
        prepared = release.prepare()
        if INTEGRATION_POLICY is None:
            policy = str(self.root / 'ops/release-policy.json')
            self.preflight.assert_called_once_with(
                [str(self.root / 'release.sh'), 'preflight', 'patch', '0.1.0', policy, str(self.root)], text=True)
            self.validate.assert_called_once_with(
                [str(self.root / 'release.sh'), 'validate-result', 'patch', '0.1.0', 'v0.1.1', policy], check=True)
        self.assertEqual(prepared['tag'], 'v0.1.1')
        self.assertEqual(release.prepare(), prepared)
        if INTEGRATION_POLICY is None:
            self.assertEqual(self.preflight.call_count, 1)
            self.assertEqual(self.validate.call_count, 1)
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

    def snapshot(self):
        return {str(path.relative_to(self.root)): path.read_bytes()
                for path in self.root.rglob('*') if path.is_file()}

    def test_failed_controller_preserves_files(self):
        if INTEGRATION_POLICY is not None:
            self.skipTest('Injected process failure belongs to the public unit suite')
        for boundary in (self.preflight, self.validate):
            with self.subTest(boundary=boundary):
                before = self.snapshot()
                boundary.side_effect = subprocess.CalledProcessError(2, 'controller')
                with self.assertRaises(subprocess.CalledProcessError):
                    release.prepare()
                self.assertEqual(self.snapshot(), before)
                boundary.side_effect = None

    def test_forbidden_targets_preserve_files(self):
        if INTEGRATION_POLICY is not None:
            self.skipTest('Injected controller results belong to the public unit suite')
        for target in ('v0.2.0', 'v0.1.100', 'garbage'):
            with self.subTest(target=target):
                before = self.snapshot()
                self.preflight.return_value = target
                with self.assertRaises(ValueError):
                    release.prepare()
                self.assertEqual(self.snapshot(), before)

    def test_pending_version_mismatch_preserves_files(self):
        release.prepare()
        pending = self.root / '.agents/runtime/preview-pending.json'
        pending.write_text(json.dumps({'version': '0.1.9'}))
        before = self.snapshot()
        with self.assertRaisesRegex(ValueError, 'Pending preview'):
            release.prepare()
        self.assertEqual(self.snapshot(), before)

    def set_version(self, value):
        for relative in ('Cargo.toml', 'Cargo.lock', 'README.md'):
            target = self.root / relative
            target.write_text(target.read_text().replace('0.1.0', value))

    def test_patch_after_99_never_releases(self):
        self.set_version('0.1.99')
        if INTEGRATION_POLICY is None:
            self.preflight.return_value = 'v0.2.0\n'
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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--integration-policy', type=Path, help='Opt in to real installed-controller tests with an authority policy')
    args, remaining = parser.parse_known_args()
    if args.integration_policy:
        INTEGRATION_POLICY = args.integration_policy.resolve()
        if not INTEGRATION_POLICY.is_file():
            parser.error('Integration unavailable: authority policy file is missing')
        controller = Path(__import__('os').environ.get('ORIG_HOME', '/www/0rig')) / 'tools/release-controller.sh'
        if not controller.is_file() or not shutil.which('ruby'):
            parser.error('Integration unavailable: installed standard controller and Ruby are required')
        print('INTEGRATION: real installed controller; temporary project files only', flush=True)
    unittest.main(argv=[sys.argv[0], *remaining])
