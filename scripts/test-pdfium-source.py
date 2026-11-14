#!/usr/bin/env python3
"""Immutable source-manifest contracts; no downloads, compiler or packaging."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('pdfium_source_collector', ROOT / 'scripts/collect-pdfium-source.py')
collector = importlib.util.module_from_spec(spec)
spec.loader.exec_module(collector)


class ManifestContracts(unittest.TestCase):
    def setUp(self):
        self.manifest = json.loads((ROOT / 'packaging/fedora/pdfium-sources.json').read_text())

    def test_committed_pair_is_immutable_source_only_and_uses_expected_branch(self):
        collector.validate_manifest(self.manifest)
        self.assertEqual(len({c['path'] for c in self.manifest['components']}), len(self.manifest['components']))
        for component in self.manifest['components']:
            self.assertTrue(collector.snapshot_url(component).endswith('/+archive/' + component['revision'] + '.tar.gz'))

    def test_branch_root_configuration_and_binary_changes_refuse_before_network(self):
        changes = [
            ('chromium_branch', 7881), ('binary_downloads', True),
            ('binary_downloads', 'false'), ('checkout_configuration', 'default'),
            ('components', []), ('components', None),
        ]
        variants = []
        for key, value in changes:
            bad = copy.deepcopy(self.manifest)
            bad[key] = value
            variants.append(bad)
        bad = copy.deepcopy(self.manifest)
        bad['components'][0]['revision'] = '0' * 40
        variants.append(bad)
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for bad in variants:
                with self.subTest(change=bad):
                    manifest = root / 'manifest.json'
                    manifest.write_text(json.dumps(bad))
                    with patch.object(collector.urllib.request, 'urlopen', side_effect=AssertionError('Network forbidden in this test')):
                        with self.assertRaises(ValueError):
                            collector.collect(manifest, root / 'output.tgz', root / 'cache')
                    self.assertFalse((root / 'cache').exists())

    def test_unpinned_foreign_or_traversal_component_refuses_before_network(self):
        for key, value in [('revision', 'main'), ('url', 'https://example.invalid/unreviewed'), ('path', '../escape')]:
            bad = copy.deepcopy(self.manifest)
            bad['components'][1][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                collector.validate_manifest(bad)


if __name__ == '__main__':
    unittest.main()
