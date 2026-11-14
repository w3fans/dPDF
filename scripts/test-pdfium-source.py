#!/usr/bin/env python3
"""Immutable source-manifest contracts; no downloads, compiler or packaging."""
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import tarfile
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

    def test_invalid_fixture_exclusions_refuse_before_network_or_cache_creation(self):
        for value in [['../escape'], ['/absolute'], ['.'], ['a/../../escape'],
                      ['a\\escape'], [None], 'not-a-list']:
            with self.subTest(value=value), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                bad = copy.deepcopy(self.manifest)
                bad['excluded_precompiled_test_fixtures'] = value
                manifest = root / 'manifest.json'
                manifest.write_text(json.dumps(bad))
                with patch.object(collector.urllib.request, 'urlopen', side_effect=AssertionError('Network forbidden')):
                    with self.assertRaises(ValueError):
                        collector.collect(manifest, root / 'output.tgz', root / 'cache')
                self.assertFalse((root / 'cache').exists())

    def test_bundle_excludes_preassembled_golden_outputs_and_retains_sources_and_notices(self):
        manifest = copy.deepcopy(self.manifest)
        manifest['components'] = manifest['components'][:1]
        golden = ['third_party/nasm/travis/test/br3005117.o.t',
                  'third_party/nasm/travis/test/binexe.exe.t']
        self.assertTrue(set(golden).issubset(manifest['excluded_precompiled_test_fixtures']))
        self.assertEqual(len([p for p in manifest['excluded_precompiled_test_fixtures']
                              if p.startswith('third_party/nasm/travis/test/')]), 200)
        entries = {golden[0]: b'\x7fELF generated object fixture',
                   golden[1]: b'MZ generated golden fixture',
                   'third_party/nasm/travis/test/binexe.asm': b'; generated source\nnop\n',
                   'LICENSE': b'Generated test notice\n'}
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            cache = root / 'cache'
            cache.mkdir()
            url = collector.snapshot_url(manifest['components'][0])
            snapshot = cache / (hashlib.sha256(url.encode()).hexdigest() + '.tar.gz')
            with tarfile.open(snapshot, 'w:gz') as tar:
                for name, data in entries.items():
                    member = tarfile.TarInfo(name)
                    member.size = len(data)
                    tar.addfile(member, io.BytesIO(data))
            path = root / 'manifest.json'
            path.write_text(json.dumps(manifest))
            with patch.object(collector.urllib.request, 'urlopen', side_effect=AssertionError('Network forbidden')):
                output = collector.collect(path, root / 'bundle.tgz', cache)
            with tarfile.open(output) as tar:
                names = set(tar.getnames())
                self.assertTrue(all('pdfium-source/' + name not in names for name in golden))
                for name in ('third_party/nasm/travis/test/binexe.asm', 'LICENSE',
                             'dpdf-source-notices/LICENSE'):
                    original = 'LICENSE' if name.startswith('dpdf-source-notices/') else name
                    self.assertEqual(tar.extractfile('pdfium-source/' + name).read(), entries[original])
                retained = json.load(tar.extractfile('pdfium-source/SOURCE-MANIFEST.json'))
                self.assertEqual({item['path'] for item in retained['excluded_test_fixture_receipt']}, set(golden))
                for item in retained['excluded_test_fixture_receipt']:
                    self.assertEqual(item['sha256'], hashlib.sha256(entries[item['path']]).hexdigest())


if __name__ == '__main__':
    unittest.main()
