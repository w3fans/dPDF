"""Frozen-target and evidence guards for automatic delivery checkpoint updates."""
import json
from pathlib import Path
import tempfile
import unittest

import delivery_checkpoint as delivery


class Checkpoint(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.pages = self.root / 'pages'
        self.pages.mkdir()
        self.output = self.root / 'publications/v0.1.49/copr'
        self.output.mkdir(parents=True)
        self.path = self.root / 'checkpoint.json'
        self.target = dict(tag='v0.1.49', source_commit='a' * 40, frozen=True)
        self.document = dict(target=self.target, complete=False,
            backfill={'v0.1.49': dict(source_commit='a' * 40)}, milestones={
                'CODE_READY': dict(status='PASS', source_commit='a' * 40),
                'TAGGED': dict(status='PASS', source_commit='a' * 40),
                'COPR_PUBLISHED': dict(status='PASS', artifacts=[
                    dict(file=f'fedora-{version}-x86_64/dpdf-0.1.49-1.fc{version}.{arch}.rpm',
                         sha256=digest * 64, bytes=123)
                    for version, arch, digest in [(44, 'x86_64', 'b'), (45, 'x86_64', 'c'),
                                                  (44, 'src', 'd'), (45, 'src', 'e')]]),
                'GITHUB_RELEASE_PUBLISHED': dict(status='BLOCKED'),
                'WINDOWS_ASSETS_PUBLISHED': dict(status='PENDING_NATIVE_BUILD'),
                'VERIFIED': dict(status='PARTIAL'),
                'BLOCKED': dict(status='YES', code='GITHUB_API_AUTH', required_owner_action='authorize publisher')})
        self.save()

    def save(self):
        self.path.write_text(json.dumps(self.document))

    def page(self):
        value = dict(tag='v0.1.49', source_commit='a' * 40, status='CREATED_VERIFIED',
                     id=10, notes_sha256='c' * 64,
                     url='https://github.com/w3fans/dPDF/releases/tag/v0.1.49')
        (self.pages / 'v0.1.49.json').write_text(json.dumps(value))
        return value

    def assets(self):
        base = 'https://github.com/w3fans/dPDF/releases/download/v0.1.49/'
        value = dict(tag='v0.1.49', source_commit='a' * 40, status='VERIFIED',
                     input_stamp=dict(copr_receipt='f' * 64, checksums='0' * 64), assets=[
                         dict(name=Path(entry['file']).name, sha256=entry['sha256'], size=entry['bytes'],
                              url=base + Path(entry['file']).name)
                         for entry in self.document['milestones']['COPR_PUBLISHED']['artifacts']])
        value['assets'] += [dict(name='copr-release.json', sha256='f' * 64),
                            dict(name='SHA256SUMS-fedora.txt', sha256='0' * 64)]
        (self.output / 'github-assets.json').write_text(json.dumps(value))
        return value

    def sync(self):
        delivery.sync(self.path, self.pages, self.root / 'publications')
        return json.loads(self.path.read_text())

    def test_auth_pending_never_becomes_published(self):
        (self.pages / 'v0.1.49.plan.json').write_text(json.dumps(dict(
            tag='v0.1.49', source_commit='a' * 40, status='PENDING_API_AUTH')))
        result = self.sync()
        self.assertEqual(result['target'], self.target)
        self.assertEqual(result['milestones']['GITHUB_RELEASE_PUBLISHED']['status'], 'BLOCKED')
        self.assertEqual(result['milestones']['BLOCKED']['code'], 'GITHUB_API_AUTH')
        self.assertFalse(result['delivery_ready_for_final_verification'])

    def test_page_and_packages_advance_without_claiming_windows_or_completion(self):
        self.page()
        self.assets()
        result = self.sync()
        self.assertEqual(result['milestones']['GITHUB_RELEASE_PUBLISHED']['status'], 'PASS')
        self.assertEqual(result['milestones']['COPR_PUBLISHED']['github_assets']['status'], 'PASS')
        self.assertEqual(result['milestones']['BLOCKED']['code'], 'WINDOWS_NATIVE_BUILD_PENDING')
        self.assertNotIn('required_owner_action', result['milestones']['BLOCKED'])
        self.assertEqual(result['milestones']['VERIFIED']['status'], 'PARTIAL')
        self.assertFalse(result['complete'])

    def test_other_source_receipt_cannot_advance_or_retarget_checkpoint(self):
        value = self.page()
        value['source_commit'] = 'f' * 40
        (self.pages / 'v0.1.49.json').write_text(json.dumps(value))
        original = self.path.read_bytes()
        with self.assertRaises(ValueError):
            self.sync()
        self.assertEqual(self.path.read_bytes(), original)

    def test_wrong_package_hash_does_not_write_verification(self):
        self.page()
        value = self.assets()
        value['assets'][0]['sha256'] = 'f' * 64
        (self.output / 'github-assets.json').write_text(json.dumps(value))
        original = self.path.read_bytes()
        with self.assertRaisesRegex(ValueError, 'qualified packages'):
            self.sync()
        self.assertEqual(self.path.read_bytes(), original)

    def test_all_phase_states_only_queue_final_live_verification(self):
        self.page()
        self.assets()
        self.document['milestones']['WINDOWS_ASSETS_PUBLISHED']['status'] = 'PASS'
        self.save()
        result = self.sync()
        self.assertTrue(result['delivery_ready_for_final_verification'])
        self.assertEqual(result['milestones']['VERIFIED']['status'], 'PENDING_FINAL_READBACK')
        self.assertFalse(result['complete'])

    def test_unfinished_historical_page_remains_required(self):
        self.page()
        self.assets()
        self.document['backfill']['v0.1.48'] = dict(source_commit='f' * 40)
        self.document['milestones']['WINDOWS_ASSETS_PUBLISHED']['status'] = 'PASS'
        self.save()
        result = self.sync()
        self.assertFalse(result['delivery_ready_for_final_verification'])
        self.assertEqual(result['milestones']['BLOCKED']['code'], 'HISTORICAL_PAGE_RECOVERY_PENDING')

    def test_incomplete_target_coverage_cannot_be_verified_from_matching_subset(self):
        self.page()
        self.assets()
        self.document['milestones']['COPR_PUBLISHED']['artifacts'].pop(1)
        self.save()
        with self.assertRaisesRegex(ValueError, 'both targets'):
            self.sync()


if __name__ == '__main__':
    unittest.main()
