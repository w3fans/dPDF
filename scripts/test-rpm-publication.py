"""Source-safe RPM identity, privacy-scan and release-redaction contracts."""
import importlib.util
import copy
import contextlib
import sys
import types
import io
import json
from pathlib import Path
import re
import shutil
import tempfile
import tarfile
import unittest
from unittest.mock import Mock, patch


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).parent / filename)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


srpm = module('srpm', 'build-srpm.py')
publisher = module('copr_publication', 'publish-copr.py')
privacy = module('privacy', 'check-public-export.py')
engine = module('engine_sources', 'collect-pdfium-source.py')
policy = module('policy', 'ci/check_pr_policy.py')
release_adapter = module('release_adapter', 'release-version.py')


class Publication(unittest.TestCase):
    def test_ambiguous_submission_is_not_silently_repeated(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'fixture.src.rpm'
            source.write_bytes(b'controlled source fixture')
            publisher.begin_submission(root, 'v0.1.46', 'a' * 40, source, list(publisher.CHROOTS))
            record = json.loads((root / 'submission-intent.json').read_text())
            self.assertEqual(record['source_commit'], 'a' * 40)
            self.assertEqual(record['srpm_sha256'], publisher.sha256_file(source))
            with self.assertRaisesRegex(ValueError, 'never blindly submit again'):
                publisher.begin_submission(root, 'v0.1.46', 'a' * 40, source, list(publisher.CHROOTS))

    def test_build_receipt_is_atomically_replaced(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'copr-release.json'
            publisher.save_receipt(path, {'build_id': 17, 'status': 'pending'})
            with patch.object(publisher.os, 'replace', side_effect=OSError('controlled interruption')):
                with self.assertRaises(OSError):
                    publisher.save_receipt(path, {'build_id': 17, 'status': 'interrupted'})
            self.assertEqual(json.loads(path.read_text()), {'build_id': 17, 'status': 'pending'})
            publisher.save_receipt(path, {'build_id': 17, 'status': 'succeeded'})
            self.assertEqual(json.loads(path.read_text()), {'build_id': 17, 'status': 'succeeded'})

    def test_each_fedora_target_requires_its_own_matching_distribution(self):
        self.assertEqual(publisher.check_rpm_identity('dpdf|0.1.45|1.fc45|x86_64', '0.1.45', 'fedora-45-x86_64'), 'x86_64')
        for identity, target in [('dpdf|0.1.45|1.fc44|x86_64', 'fedora-45-x86_64'),
                                 ('dpdf|0.1.45|1.fc45|x86_64', 'fedora-44-x86_64'),
                                 ('dpdf|0.1.45|1.fc45|aarch64', 'fedora-45-x86_64'),
                                 ('dpdf|0.1.45|1.fc45|x86_64', 'fedora-46-x86_64')]:
            with self.assertRaises(ValueError):
                publisher.check_rpm_identity(identity, '0.1.45', target)

    def test_release_rejects_missing_duplicate_and_misrouted_target_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'dpdf-0.1.45-1.fc44.src.rpm'
            source.touch()
            binaries = []
            for target in publisher.CHROOTS:
                folder = root / target
                folder.mkdir()
                binary = folder / 'dpdf.rpm'
                binary.touch()
                binaries.append(binary)
            def checked(path, version, commit, chroot=publisher.CHROOTS[0]):
                return 'src' if path.name.endswith('.src.rpm') else 'x86_64'
            with patch.object(publisher, 'check_rpm', side_effect=checked) as verify:
                self.assertEqual(len(publisher.verified_packages(root, '0.1.45', 'a' * 40, publisher.CHROOTS)), 3)
                self.assertTrue(any(call.args == (binaries[1], '0.1.45', 'a' * 40, 'fedora-45-x86_64') for call in verify.call_args_list))
                binaries[1].unlink()
                with self.assertRaises(ValueError):
                    publisher.verified_packages(root, '0.1.45', 'a' * 40, publisher.CHROOTS)
                binaries[1].touch()
                duplicate = binaries[1].with_name('duplicate.rpm')
                duplicate.touch()
                with self.assertRaises(ValueError):
                    publisher.verified_packages(root, '0.1.45', 'a' * 40, publisher.CHROOTS)
                duplicate.unlink()
                misplaced = root / 'outside-target.rpm'
                binaries[1].rename(misplaced)
                with self.assertRaises(ValueError):
                    publisher.verified_packages(root, '0.1.45', 'a' * 40, publisher.CHROOTS)
                misplaced.rename(binaries[1])
                for targets in [(), ('fedora-45-x86_64',) * 2, ('fedora-46-x86_64',)]:
                    with self.assertRaises(ValueError):
                        publisher.verified_packages(root, '0.1.45', 'a' * 40, targets)
                # Actual copr-cli target downloads include a rebuilt SRPM in
                # each target, rather than only one shared root-level SRPM.
                source.rename(root / publisher.CHROOTS[0] / source.name)
                target_source = root / publisher.CHROOTS[1] / 'dpdf-0.1.45-1.fc45.src.rpm'
                target_source.touch()
                self.assertEqual(len(publisher.verified_packages(root, '0.1.45', 'a' * 40, publisher.CHROOTS)), 4)
                target_source.with_name('duplicate.src.rpm').touch()
                with self.assertRaises(ValueError):
                    publisher.verified_packages(root, '0.1.45', 'a' * 40, publisher.CHROOTS)

    def test_copr_source_url_uses_actual_nested_dictionary_metadata(self):
        url = 'https://download.copr.fedorainfracloud.org/results/fixture/dpdf/source.src.rpm'
        self.assertEqual(publisher.source_package_url({'source_package': {'url': url}}), url)
        for package in (None, {}, {'url': None}, {'url': 42},
                        {'url': url.replace('https:', 'http:')},
                        {'url': url.replace('.src.rpm', '.x86_64.rpm')},
                        {'url': url.replace('.src.rpm', '.txt')},
                        {'url': url.replace('download.copr.fedorainfracloud.org', 'example.invalid')}):
            with self.assertRaises(ValueError):
                publisher.source_package_url({'source_package': package})

    def test_corresponding_source_requires_exact_unambiguous_regular_revision(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'source.tar.gz'

            def archive(values):
                with tarfile.open(path, 'w:gz') as output:
                    for name, value, kind in values:
                        member = tarfile.TarInfo(name)
                        member.type = kind
                        member.size = len(value) if kind == tarfile.REGTYPE else 0
                        if kind == tarfile.SYMTYPE:
                            member.linkname = '/private/enrollment.json'
                        output.addfile(member, io.BytesIO(value) if member.isfile() else None)

            name = 'dpdf-0.1.45/SOURCE-REVISION'
            marker = (name, b'a' * 40 + b'\n', tarfile.REGTYPE)
            archive([marker])
            self.assertEqual(publisher.source_revision(path, '0.1.45'), 'a' * 40)
            for values in ([], [marker, marker],
                           [(name, b'not-a-commit\n', tarfile.REGTYPE)],
                           [(name, b'', tarfile.SYMTYPE)],
                           [('dpdf-0.1.44/SOURCE-REVISION', marker[1], tarfile.REGTYPE)]):
                archive(values)
                with self.assertRaises(ValueError):
                    publisher.source_revision(path, '0.1.45')

    def test_selected_gpl_text_is_required_without_private_release_controller(self):
        source = Path(__file__).resolve().parent.parent
        notes = json.loads((source / 'release.json').read_text())['notes']
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for relative in ('Cargo.toml', 'Cargo.lock', 'release.json', 'README.md', 'CHANGELOG.md',
                             'LICENSE', 'NOTICE', notes,
                             'packaging/licenses/slint/LicenseRef-Slint-Royalty-free-2.0.md'):
                target = root / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source / relative, target)
            with patch.object(release_adapter, 'ROOT', root):
                with self.assertRaisesRegex(ValueError, 'Missing selected Slint GPL terms'):
                    release_adapter.verify()
                relative = 'packaging/licenses/slint/GPL-3.0-only.txt'
                shutil.copy2(source / relative, root / relative)
                self.assertEqual(release_adapter.verify()['channel'], 'development-preview')

    def test_engine_sources_require_pinned_upstream_and_confined_paths(self):
        component = dict(path='third_party/example', url='https://pdfium.googlesource.com/pdfium', revision='a' * 40)
        self.assertTrue(engine.snapshot_url(component).endswith('/' + 'a' * 40 + '.tar.gz'))
        for changes in (dict(revision='main'), dict(path='../outside'), dict(url='https://example.invalid/binary')):
            with self.assertRaises(ValueError):
                engine.snapshot_url(dict(component, **changes))

    def test_release_version_is_read_never_invented(self):
        self.assertEqual(srpm.version_from_manifest('[package]\nversion="0.1.45"\n'), '0.1.45')
        for version in ('0.2.0', '0.1.100', '0.1.4\n%post bad'):
            with self.assertRaises(ValueError):
                srpm.version_from_manifest('[package]\nversion="' + version + '"\n')
        self.assertEqual(srpm.render_spec('Version: @VERSION@', '0.1.45'), 'Version: 0.1.45')
        with self.assertRaises(ValueError):
            srpm.render_spec('no placeholder', '0.1.45')

    def test_wrong_platform_version_package_and_injected_rpm_identity_fail(self):
        self.assertEqual(publisher.check_rpm_identity('dpdf|0.1.45|1.fc44|x86_64', '0.1.45'), 'x86_64')
        for identity in ('other|0.1.45|1.fc44|x86_64', 'dpdf|0.1.44|1.fc44|x86_64',
                         'dpdf|0.1.45|1.fc43|x86_64', 'dpdf|0.1.45|1.fc44|aarch64',
                         'dpdf|0.1.45|1.fc44|x86_64|extra'):
            with self.assertRaises(ValueError):
                publisher.check_rpm_identity(identity, '0.1.45')

    def test_private_patterns_are_supplied_outside_the_product_tree(self):
        patterns = [re.compile(r'private-machine'), re.compile(r'/home/private-user/')]
        self.assertEqual(privacy.matches('generic instructions\nprivate-machine\n/home/private-user/file', patterns),
                         [(2, 1), (3, 2)])
        self.assertEqual(privacy.matches('Generic workstation; no personal identity.', patterns), [])

    def test_release_history_permits_only_short_explicit_privacy_removal(self):
        before = '# v0.1.45\nTested on private-machine; 62 checks passed.\n'
        after = '# v0.1.45\nTested on [private]; 62 checks passed.\n'
        self.assertTrue(policy.redaction_only(before, after))
        for altered in (after.replace('62', '63'), after.replace('v0.1.45', 'v0.1.46'),
                        '# v0.1.45\nAll checks passed.\n'):
            self.assertFalse(policy.redaction_only(before, altered))
        self.assertFalse(policy.redaction_only(before, '[private]'))
        self.assertFalse(policy.redaction_only('a' * 257, '[private]'))


class AssetAPI:
    def __init__(self):
        self.commit = 'a' * 40
        self.items = []
        self.uploads = []
        self.ambiguous = False
        self.bad_digest = False

    def __call__(self, endpoint):
        if '/git/ref/tags/' in endpoint:
            return {'object': {'type': 'commit', 'sha': self.commit}}
        if '/releases/tags/' in endpoint:
            return dict(id=12, tag_name='v0.1.49', draft=False, prerelease=True,
                        html_url='https://github.com/w3fans/dPDF/releases/tag/v0.1.49')
        if '/releases/12/assets?' in endpoint:
            return copy.deepcopy(self.items)
        raise AssertionError(endpoint)

    def upload(self, tag, path):
        self.uploads.append(path.name)
        self.items.append(dict(id=len(self.items) + 1, name=path.name, state='uploaded',
            size=path.stat().st_size,
            digest='sha256:' + ('f' * 64 if self.bad_digest else publisher.sha256_file(path)),
            browser_download_url='https://github.com/w3fans/dPDF/releases/download/' + tag + '/' + path.name))
        if self.ambiguous:
            raise RuntimeError('Connection lost after server committed upload')


class ReleaseAssets(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.paths = [self.root / 'first.rpm', self.root / 'second.src.rpm']
        for number, path in enumerate(self.paths):
            path.write_bytes(('fixture ' + str(number)).encode())
        self.api = AssetAPI()

    def publish(self):
        return publisher.attach_verified_assets('v0.1.49', 'a' * 40, self.paths,
                                                 self.api, self.api.upload)

    def test_verified_retry_never_uploads_twice(self):
        result = self.publish()
        self.assertEqual(result['status'], 'VERIFIED')
        self.assertEqual(len(result['assets']), 2)
        self.publish()
        self.assertEqual(self.api.uploads, [p.name for p in self.paths])

    def test_all_existing_collisions_are_checked_before_any_write(self):
        self.api.upload('v0.1.49', self.paths[1])
        self.api.items[0]['digest'] = 'sha256:' + 'f' * 64
        self.api.uploads.clear()
        with self.assertRaisesRegex(ValueError, 'nothing overwritten'):
            self.publish()
        self.assertEqual(self.api.uploads, [])

    def test_missing_digest_is_not_assumed_verified(self):
        self.api.upload('v0.1.49', self.paths[0])
        self.api.items[0].pop('digest')
        self.api.uploads.clear()
        with self.assertRaises(ValueError):
            self.publish()
        self.assertEqual(self.api.uploads, [])

    def test_wrong_remote_tag_prevents_upload(self):
        self.api.commit = 'b' * 40
        with self.assertRaisesRegex(ValueError, 'verified source'):
            self.publish()
        self.assertEqual(self.api.uploads, [])

    def test_ambiguous_upload_is_read_back_without_clobber(self):
        self.api.ambiguous = True
        self.assertEqual(self.publish()['status'], 'VERIFIED')
        self.assertEqual(len(self.api.uploads), 2)

    def test_bad_server_hash_prevents_verified_result(self):
        self.api.bad_digest = True
        with self.assertRaisesRegex(ValueError, 'readback differs'):
            self.publish()
        self.assertEqual(len(self.api.uploads), 1)

    def test_duplicate_remote_names_prevent_upload(self):
        self.api.upload('v0.1.49', self.paths[0])
        self.api.items.append(copy.deepcopy(self.api.items[0]))
        self.api.uploads.clear()
        with self.assertRaisesRegex(ValueError, 'Duplicate remote'):
            self.publish()
        self.assertEqual(self.api.uploads, [])

    def test_upload_command_never_clobbers(self):
        with patch.object(publisher.subprocess, 'run') as run:
            run.return_value.returncode = 0
            publisher.github_upload('v0.1.49', self.paths[0])
        self.assertNotIn('--clobber', run.call_args.args[0])

    def test_attachment_only_never_builds_or_downloads(self):
        receipt = dict(tag='v0.1.49', source_commit='a' * 40, project='zvegelj/dpdf',
                       status='succeeded', build_id=77, verified_chroots=list(publisher.CHROOTS))
        (self.root / 'copr-release.json').write_text(json.dumps(receipt))
        (self.root / 'SHA256SUMS-fedora.txt').write_text(''.join(
            publisher.sha256_file(p) + '  ' + p.name + '\n' for p in self.paths))
        client = Mock()
        client.build_proxy.get.return_value = types.SimpleNamespace(
            id=77, state='succeeded', ownername='zvegelj', projectname='dpdf')
        client.build_chroot_proxy.get.return_value = types.SimpleNamespace(state='succeeded')
        copr = types.ModuleType('copr')
        copr_v3 = types.ModuleType('copr.v3')
        copr_v3.Client = types.SimpleNamespace(create_from_config_file=lambda: client)
        with patch.dict(sys.modules, {'copr': copr, 'copr.v3': copr_v3}), \
                patch.dict(publisher.os.environ, {'COPR_LOGIN': '', 'COPR_TOKEN': ''}), \
                patch.object(sys, 'argv', ['publish-copr.py', '--tag', 'v0.1.49',
                                           '--output', str(self.root), '--attach-only']), \
                patch.object(publisher, 'release_identity', return_value='a' * 40), \
                patch.object(publisher, 'verified_packages', return_value=self.paths), \
                patch.object(publisher, 'attach_verified_assets', return_value={
                    'status': 'VERIFIED', 'tag': 'v0.1.49', 'source_commit': 'a' * 40}), \
                patch.object(publisher.subprocess, 'check_output', return_value=''), \
                patch.object(publisher.subprocess, 'run') as command:
            with contextlib.redirect_stdout(io.StringIO()):
                publisher.main()
        command.assert_not_called()
        client.build_proxy.create_from_file.assert_not_called()
        self.assertEqual(json.loads((self.root / 'github-assets.json').read_text())['status'], 'VERIFIED')


if __name__ == '__main__':
    unittest.main()
