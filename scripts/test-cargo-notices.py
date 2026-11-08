#!/usr/bin/env python3
"""Notice collection must respect target selection and package filesystem boundaries."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('notices', Path(__file__).with_name('collect-cargo-notices.py'))
notices = importlib.util.module_from_spec(spec)
spec.loader.exec_module(notices)


class Collection(unittest.TestCase):
    def fixture(self, root):
        packages = []
        for name in ('dpdf', 'selected', 'also-selected', 'unused'):
            directory = root / name
            directory.mkdir()
            (directory / 'LICENSE').write_text('Copyright Test Fixture. Permission granted for fixture use.\n')
            packages.append(dict(id=name, name=name, version='1.2.3', source='registry+https://example.invalid',
                                 manifest_path=str(directory / 'Cargo.toml'), license='MIT OR Apache-2.0',
                                 license_file=None, repository='https://example.invalid/project?drop=this'))
        metadata = dict(version=1, packages=packages, workspace_members=['dpdf'],
                        resolve=dict(root='dpdf', nodes=[dict(id=p['id'], features=[]) for p in packages]))
        lock = dict(package=[dict(name=p['name'], version=p['version'], source=p['source'], checksum='a' * 64)
                             for p in packages])
        return metadata, lock

    def test_target_selection_dedup_and_reproducibility(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            metadata, lock = self.fixture(root)
            tree = 'dpdf v1.2.3 (workspace)\nselected v1.2.3\nalso-selected v1.2.3 (*)\n'
            result = notices.collect(metadata, lock, 'test-target', root / 'first', tree)
            self.assertEqual([p['name'] for p in result['components']], ['also-selected', 'selected'])
            self.assertEqual(len(list((root / 'first' / 'texts').iterdir())), 1)
            self.assertFalse(result['qualified_for_binary_distribution'])
            self.assertEqual(result['missing_license_text'], [])
            self.assertEqual(result['components'][0]['repository'], 'https://example.invalid/project')
            metadata['packages'][0]['version'] = '9.9.9'
            second = notices.collect(metadata, lock, 'test-target', root / 'second', tree.replace('dpdf v1.2.3', 'dpdf v9.9.9'))
            self.assertEqual(result['dependency_fingerprint'], second['dependency_fingerprint'])

    def test_missing_text_is_explicit_and_never_clearance(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            metadata, lock = self.fixture(root)
            (root / 'selected' / 'LICENSE').unlink()
            result = notices.collect(metadata, lock, 'test-target', root / 'out', 'selected v1.2.3\n')
            self.assertEqual(result['missing_license_text'], ['selected@1.2.3'])
            self.assertEqual(result['audit_status'], 'inventory_only')
            self.assertFalse(result['qualified_for_binary_distribution'])

    def test_declared_file_cannot_escape_package(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            metadata, lock = self.fixture(root)
            (root / 'outside.txt').write_text('not a package notice')
            metadata['packages'][1]['license_file'] = '../outside.txt'
            with self.assertRaisesRegex(ValueError, 'escapes package'):
                notices.collect(metadata, lock, 'test-target', root / 'out', 'selected v1.2.3\n')
            self.assertEqual(list((root / 'out' / 'texts').iterdir()), [])

    def test_source_ambiguity_and_unknown_tree_entries_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            metadata, lock = self.fixture(root)
            with self.assertRaisesRegex(ValueError, 'versions differ'):
                notices.collect(metadata, lock, 'test-target', root / 'unknown', 'unknown v1.0.0\n')
            with self.assertRaisesRegex(ValueError, 'no packages'):
                notices.collect(metadata, lock, 'test-target', root / 'empty', '')
            duplicate = copy.deepcopy(metadata['packages'][1])
            duplicate['id'] = 'other-source'
            duplicate['source'] = 'another-registry'
            metadata['packages'].append(duplicate)
            metadata['resolve']['nodes'].append(dict(id='other-source', features=[]))
            with self.assertRaisesRegex(ValueError, 'Ambiguous package source'):
                notices.collect(metadata, lock, 'test-target', root / 'ambiguous', 'selected v1.2.3\n')


if __name__ == '__main__':
    unittest.main()
