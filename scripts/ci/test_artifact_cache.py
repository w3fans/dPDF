"""Provenance and mutation regressions for native artifact reuse."""
import json
from pathlib import Path
import tempfile
import unittest

import artifact_cache as cache
import build_windows
import copy


class Artifacts(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.env = {'CARGO_HOME': str(self.root / 'cargo')}
        self.identity = cache.identity('a' * 40, 'compiler', 'cargo', self.env)
        self.original = self.root / 'compiled.exe'
        self.original.write_bytes(b'controlled artifact fixture')

    def hosted_fixture(self):
        source = self.root / 'source'
        source.mkdir()
        (source / 'build.ps1').write_text('# synthetic adapter')
        identity = cache.identity('a' * 40, 'compiler', 'cargo', self.env,
                                  cache.digest(source / 'build.ps1'))
        target = self.root / 'target'
        binary = target / 'release/dpdf.exe'
        binary.parent.mkdir(parents=True)
        binary.write_bytes(self.original.read_bytes())
        names = {'ci-contract', 'powershell-syntax'} | {
            name for name, _, _ in build_windows.validate_windows.plan(
                source, 'binary', 'pdfium', 'qpdf', 'service')}
        report = dict(status='PASS', lane='hosted', source_commit='a' * 40,
                      compiler='compiler', toolchain='cargo', artifact_identity=identity,
                      binary_sha256=cache.digest(binary),
                      checks=[dict(name=name, status='PASS') for name in sorted(names)])
        return source, target, binary, report

    def test_hosted_recovery_requires_exact_passed_source_and_bytes(self):
        source, target, binary, report = self.hosted_fixture()
        snapshot, record = build_windows.reuse_hosted(source, 'a' * 40, report, target, self.root / 'cache')
        self.assertEqual(snapshot.read_bytes(), binary.read_bytes())
        self.assertTrue(record['artifact_reused'])
        self.assertNotIn('native_acceptance', record)
        for key, value in [('status', 'FAIL'), ('source_commit', 'b' * 40), ('lane', 'desktop')]:
            changed = copy.deepcopy(report)
            changed[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                build_windows.reuse_hosted(source, 'a' * 40, changed, target, self.root / 'cache')
        binary.write_bytes(b'tampered cache')
        with self.assertRaisesRegex(ValueError, 'no rebuild'):
            build_windows.reuse_hosted(source, 'a' * 40, report, target, self.root / 'cache')

    def test_hosted_recovery_rejects_missing_checks_or_changed_adapter(self):
        source, target, binary, report = self.hosted_fixture()
        changed = copy.deepcopy(report)
        changed['checks'].pop()
        with self.assertRaises(ValueError):
            build_windows.reuse_hosted(source, 'a' * 40, changed, target, self.root / 'cache')
        (source / 'build.ps1').write_text('# changed adapter')
        with self.assertRaisesRegex(ValueError, 'provenance'):
            build_windows.reuse_hosted(source, 'a' * 40, report, target, self.root / 'cache')

    def test_immutable_snapshot_survives_later_target_builds(self):
        snapshot = cache.store(self.root, self.identity, self.original)
        self.original.write_bytes(b'next commit output')
        self.assertEqual(cache.load(self.root, self.identity), snapshot)
        self.assertEqual(snapshot.read_bytes(), b'controlled artifact fixture')
        receipt = json.loads((cache.folder(self.root, self.identity) / 'artifact.json').read_text())
        self.assertEqual(receipt['scope'], 'build_identity_only')

    def test_commit_compiler_cargo_and_flags_do_not_cross_reuse(self):
        cache.store(self.root, self.identity, self.original)
        alternatives = [cache.identity('b' * 40, 'compiler', 'cargo', self.env),
                        cache.identity('a' * 40, 'changed compiler', 'cargo', self.env),
                        cache.identity('a' * 40, 'compiler', 'changed cargo', self.env),
                        cache.identity('a' * 40, 'compiler', 'cargo', self.env | {'RUSTFLAGS': '-C opt-level=1'}),
                        cache.identity('a' * 40, 'compiler', 'cargo', self.env | {'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS': '-C debuginfo=2'})]
        for expected in alternatives:
            with self.subTest(expected=expected):
                self.assertIsNone(cache.load(self.root, expected))

    def test_global_cargo_config_change_invalidates_identity(self):
        home = Path(self.env['CARGO_HOME'])
        home.mkdir()
        cache.store(self.root, self.identity, self.original)
        (home / 'config.toml').write_text('[build]\nrustflags = ["-C", "debuginfo=2"]\n')
        changed = cache.identity('a' * 40, 'compiler', 'cargo', self.env)
        self.assertNotEqual(changed, self.identity)
        self.assertIsNone(cache.load(self.root, changed))

    def test_changed_bytes_and_tampered_receipt_fail_closed(self):
        snapshot = cache.store(self.root, self.identity, self.original)
        snapshot.write_bytes(b'tampered')
        with self.assertRaisesRegex(ValueError, 'hash mismatch'):
            cache.load(self.root, self.identity)
        receipt = cache.folder(self.root, self.identity) / 'artifact.json'
        record = json.loads(receipt.read_text())
        record['binary_sha256'] = '../outside'
        receipt.write_text(json.dumps(record))
        with self.assertRaisesRegex(ValueError, 'identity mismatch'):
            cache.load(self.root, self.identity)

    def test_symlink_snapshot_is_refused(self):
        snapshot = cache.store(self.root, self.identity, self.original)
        snapshot.unlink()
        try:
            snapshot.symlink_to(self.original)
        except OSError:
            self.skipTest('Symlink privilege unavailable')
        with self.assertRaisesRegex(ValueError, 'artifact path'):
            cache.load(self.root, self.identity)

    def test_cargo_manifest_selects_real_output_in_both_encodings(self):
        target = self.root / 'target'
        binary = target / 'x86_64-pc-windows-msvc/release/dpdf.exe'
        binary.parent.mkdir(parents=True)
        binary.write_bytes(b'actual output fixture')
        event = dict(reason='compiler-artifact', target=dict(name='dpdf', kind=['bin']),
                     profile=dict(test=False), executable=str(binary))
        for encoding in ('utf-8', 'utf-16'):
            manifest = self.root / ('manifest-' + encoding)
            manifest.write_text(json.dumps(event) + '\n', encoding=encoding)
            self.assertEqual(cache.built_binary(manifest, target), binary.resolve())

    def test_cargo_manifest_rejects_unrelated_and_ambiguous_outputs(self):
        target = self.root / 'target'
        target.mkdir()
        event = dict(reason='compiler-artifact', target=dict(name='dpdf', kind=['bin']),
                     profile=dict(test=False), executable=str(self.original))
        manifest = self.root / 'manifest'
        manifest.write_text(json.dumps(event) + '\n')
        with self.assertRaisesRegex(ValueError, 'expected native'):
            cache.built_binary(manifest, target)
        event['profile']['test'] = True
        manifest.write_text(json.dumps(event) + '\n')
        with self.assertRaisesRegex(ValueError, 'exactly one'):
            cache.built_binary(manifest, target)

    def test_native_acceptance_cannot_be_inferred_from_cache_or_partial_results(self):
        report = dict(status='PASS', lane='self-hosted', source_commit='a' * 40,
                      binary_sha256='b' * 64, checks=[dict(name='build', status='PASS')])
        cache.validate_acceptance(report, 'a' * 40, 'b' * 64, {'build'})
        for field, value in [('status', 'QUEUED'), ('lane', 'hosted'),
                             ('source_commit', 'c' * 40), ('binary_sha256', 'd' * 64)]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                cache.validate_acceptance(report | {field: value}, 'a' * 40, 'b' * 64, {'build'})
        with self.assertRaisesRegex(ValueError, 'incomplete'):
            cache.validate_acceptance(report, 'a' * 40, 'b' * 64, {'build', 'reader'})
        with self.assertRaisesRegex(ValueError, 'incomplete'):
            cache.validate_acceptance(report | {'checks': [dict(name='build', status='SKIP')]},
                                      'a' * 40, 'b' * 64, {'build'})


if __name__ == '__main__':
    unittest.main()
