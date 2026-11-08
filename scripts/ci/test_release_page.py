"""Release API recovery guards, including ambiguous creation and changed notes."""
import copy
import contextlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import release_page as page


class API:
    def __init__(self, ambiguous=False):
        self.commit = 'a' * 40
        self.release = None
        self.writes = 0
        self.ambiguous = ambiguous

    def __call__(self, endpoint, payload=None, missing=False):
        if endpoint == page.ENDPOINT:
            return dict(full_name=page.REPOSITORY, permissions={'push': True})
        if '/git/ref/tags/' in endpoint:
            return {'object': {'type': 'tag', 'sha': 'b' * 40}}
        if '/git/tags/' in endpoint:
            return {'object': {'type': 'commit', 'sha': self.commit}}
        if payload is not None:
            self.writes += 1
            self.release = dict(payload, id=123, html_url='https://github.com/w3fans/dPDF/releases/tag/v0.1.48')
            if self.ambiguous:
                raise RuntimeError('Connection lost after server committed creation')
            return copy.deepcopy(self.release)
        if self.release is None and not missing:
            raise RuntimeError('Missing release')
        return copy.deepcopy(self.release)


class Recovery(unittest.TestCase):
    def item(self):
        return dict(tag='v0.1.48', source_commit='a' * 40,
                    notes='# dPDF v0.1.48 — development preview\n', notes_sha256='c' * 64)

    def test_real_cli_returns_failure_for_an_unfinished_historical_entry(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'source'
            source.mkdir()
            subprocess.run(['git', 'init', '-q', str(source)], check=True)
            subprocess.run(['git', '-C', str(source), 'remote', 'add', 'origin',
                            'git@github-w3f:w3fans/dPDF.git'], check=True)
            patterns = root / 'patterns.json'
            patterns.write_text('["fixture-private"]')
            result = subprocess.run([sys.executable, str(Path(page.__file__).resolve()),
                '--repository', str(source), '--output', str(root / 'output'),
                '--privacy-patterns', str(patterns), '--tag', 'v0.1.49'],
                capture_output=True, text=True)
            self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
            failure = json.loads((root / 'output/v0.1.49.failure.json').read_text())
            self.assertEqual(failure['status'], 'FAILED')
            self.assertEqual(failure['phase'], 'source_plan')

    def test_asset_recovery_does_not_create_missing_builds(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(page.subprocess, 'run') as run:
            root = Path(directory)
            self.assertEqual(page.recover_copr_assets(self.item(), root, root, root / 'publisher'),
                             'PENDING_VERIFIED_COPR_ARTIFACTS')
            run.assert_not_called()

    def test_asset_recovery_rejects_wrong_source_receipt(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(page.subprocess, 'run') as run:
            root = Path(directory)
            output = root / 'v0.1.48/copr'
            output.mkdir(parents=True)
            (output / 'copr-release.json').write_text(json.dumps(dict(
                tag='v0.1.48', source_commit='d' * 40, project='zvegelj/dpdf')))
            with self.assertRaisesRegex(ValueError, 'reviewed release source'):
                page.recover_copr_assets(self.item(), root, root, root / 'publisher')
            run.assert_not_called()

    def test_retained_asset_receipt_does_not_rehash_large_packages_or_rebuild(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(page.subprocess, 'run') as run:
            root = Path(directory)
            output = root / 'v0.1.48/copr'
            output.mkdir(parents=True)
            receipt = output / 'copr-release.json'
            receipt.write_text(json.dumps(dict(tag='v0.1.48', source_commit='a' * 40,
                project='zvegelj/dpdf', status='succeeded',
                verified_chroots=['fedora-44-x86_64', 'fedora-45-x86_64'])))
            checksum = output / 'SHA256SUMS-fedora.txt'
            checksum.write_text('fixture checksums')
            stamps = dict(copr_receipt=page.hashlib.sha256(receipt.read_bytes()).hexdigest(),
                          checksums=page.hashlib.sha256(checksum.read_bytes()).hexdigest())
            (output / 'github-assets.json').write_text(json.dumps(dict(tag='v0.1.48',
                source_commit='a' * 40, status='VERIFIED', input_stamp=stamps)))
            self.assertEqual(page.recover_copr_assets(self.item(), root, root, root / 'publisher'),
                             'VERIFIED_RECEIPT_RETAINED')
            run.assert_not_called()

    def test_missing_auth_apply_reports_failure_but_dry_run_succeeds(self):
        for apply in (False, True):
            with self.subTest(apply=apply), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                patterns = root / 'patterns.json'
                patterns.write_text('["fixture-private"]')
                output = root / 'output'
                argv = ['release_page.py', '--repository', str(root),
                        '--output', str(output), '--privacy-patterns', str(patterns),
                        '--tag', 'v0.1.48'] + (['--apply'] if apply else [])
                with patch.object(sys, 'argv', argv), \
                        patch.object(page, 'git', return_value='git@github-w3f:w3fans/dPDF.git'), \
                        patch.object(page, 'plan', return_value=self.item()), \
                        patch.object(page.shutil, 'which', return_value=None), \
                        patch.object(page, 'GitHub') as api, \
                        contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(page.main(), 1 if apply else 0)
                api.assert_not_called()
                plan = json.loads((output / 'v0.1.48.plan.json').read_text())
                self.assertEqual(plan['status'], 'PENDING_API_AUTH' if apply else 'DRY_RUN')
                self.assertFalse((output / 'v0.1.48.json').exists())

    def test_missing_auth_preserves_verified_receipt_and_processes_later_tags(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            patterns = root / 'patterns.json'
            patterns.write_text('["fixture-private"]')
            output = root / 'output'
            output.mkdir()
            first = self.item()
            saved = dict(first, status='EXISTING_VERIFIED', id=123)
            saved.pop('notes')
            receipt_path = output / 'v0.1.48.json'
            original = json.dumps(saved)
            receipt_path.write_text(original)
            later = dict(first, tag='v0.1.49')
            def repository(*args):
                return 'v0.1.48\nv0.1.49' if args[1:3] == ('tag', '--list') else 'git@github-w3f:w3fans/dPDF.git'
            with patch.object(sys, 'argv', ['release_page.py', '--repository', str(root),
                    '--output', str(output), '--privacy-patterns', str(patterns),
                    '--since', 'v0.1.48', '--apply']), \
                    patch.object(page, 'git', side_effect=repository), \
                    patch.object(page, 'plan', side_effect=[first, later]), \
                    patch.object(page.shutil, 'which', return_value=None), \
                    contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(page.main(), 1)
            self.assertEqual(receipt_path.read_text(), original)
            self.assertEqual(json.loads((output / 'v0.1.49.plan.json').read_text())['status'], 'PENDING_API_AUTH')

    def test_failed_historical_plan_does_not_block_later_release(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            patterns = root / 'patterns.json'
            patterns.write_text('["fixture-private"]')
            output = root / 'output'
            def repository(*args):
                return 'v0.1.47\nv0.1.48' if args[1:3] == ('tag', '--list') else 'git@github-w3f:w3fans/dPDF.git'
            with patch.object(sys, 'argv', ['release_page.py', '--repository', str(root),
                    '--output', str(output), '--privacy-patterns', str(patterns), '--since', 'v0.1.47']), \
                    patch.object(page, 'git', side_effect=repository), \
                    patch.object(page, 'plan', side_effect=[ValueError('Historical plan requires review'), self.item()]), \
                    patch.object(page, 'reconcile', return_value=dict(status='DRY_RUN')) as reconcile, \
                    contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(page.main(), 1)
            self.assertEqual(reconcile.call_count, 1)
            self.assertEqual(reconcile.call_args.args[0]['tag'], 'v0.1.48')
            failure = json.loads((output / 'v0.1.47.failure.json').read_text())
            self.assertEqual(failure['phase'], 'source_plan')

    def test_auth_recovery_reuses_same_tag_and_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            self.assertEqual(page.reconcile(self.item(), output, apply=True)['status'], 'PENDING_API_AUTH')
            api = API()
            self.assertEqual(page.reconcile(self.item(), output, api, apply=True)['status'], 'CREATED_VERIFIED')
            self.assertEqual(page.reconcile(self.item(), output, api, apply=True, recheck=True)['status'], 'EXISTING_VERIFIED')
            self.assertEqual(api.writes, 1)

    def test_operator_deliver_joins_existing_service_without_build(self):
        config = dict(DPDF_RELEASE_PAGE_REPOSITORY=Path('/fixture/index'),
                      DPDF_PUBLICATION_OUTPUTS=Path('/fixture/output'),
                      DPDF_PRIVACY_PATTERNS=Path('/fixture/patterns'))
        with patch.object(page, 'operator_config', return_value=config), \
                patch.object(Path, 'read_text', return_value='["private-fixture"]'), \
                patch.object(page, 'plan', return_value=self.item()) as planned, \
                patch.object(page.subprocess, 'run') as run, \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(page.operator_command(['deliver', '--tag', 'v0.1.48']), 0)
        planned.assert_called_once()
        run.assert_called_once_with(['systemctl', '--user', 'start', '--no-block',
                                     'dpdf-release-pages.service'], check=True)

    def test_status_missing_windows_and_packages_never_claims_complete(self):
        with tempfile.TemporaryDirectory() as directory, \
                patch.object(page, 'git', side_effect=['v0.1.48', 'a' * 40]), \
                patch.object(page.shutil, 'which', return_value=None):
            result = page.operator_status(Path('/fixture'), Path(directory))
        self.assertEqual(result['tag'], 'v0.1.48')
        self.assertEqual(result['status'], 'PENDING')
        for state in ('PENDING_API_AUTH', 'PENDING_WINDOWS_ARTIFACTS', 'PENDING_COPR'):
            self.assertIn(state, result['blockers'])

    def test_shell_delivery_dispatch_precedes_version_controller(self):
        root = Path(__file__).resolve().parents[2]
        text = (root / 'release.sh').read_text()
        self.assertLess(text.index('== delivery-status'), text.index('== patch'))
        self.assertIn('release_page.py deliver --tag "$tag"', text)
        self.assertIn('TAGGED $tag; PUBLICATION_QUEUED', text)

    def test_pushed_existing_tag_hands_off_without_version_allocation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ('scripts/ci', '.agents/runtime', 'bin', 'standard/tools',
                         '.config/dpdf'):
                (root / name).mkdir(parents=True)
            adapter = Path(__file__).resolve().parents[2] / 'release.sh'
            (root / 'release.sh').write_bytes(adapter.read_bytes())
            for name in ('.agents/validate', 'standard/tools/lease-adapter.sh'):
                path = root / name
                path.write_text('#!/bin/sh\nexit 0\n')
                path.chmod(0o700)
            (root / '.config/dpdf/publication.env').write_text('# fixture only\n')
            (root / 'scripts/release-version.py').write_text('print("v0.1.53")\n')
            (root / 'scripts/ci/release_page.py').write_text(
                'import os,sys\nfrom pathlib import Path\n'
                'with Path(os.environ["COMMAND_LOG"]).open("a") as f:\n'
                ' f.write("handoff " + " ".join(sys.argv[1:]) + "\\n")\n')
            git = root / 'bin/git'
            git.write_text('''#!/bin/sh
case "$*" in
  'remote get-url origin') echo git@github-w3f:w3fans/dPDF.git ;;
  'symbolic-ref --short HEAD') echo main ;;
  'rev-parse --verify HEAD'|'rev-parse HEAD'|'rev-parse v0.1.53^{}')
    echo aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ;;
  'fetch origin'|'show-ref --verify --quiet refs/remotes/origin/main'|\
'show-ref --verify --quiet refs/tags/v0.1.53'|\
'merge-base --is-ancestor origin/main HEAD'|'add -- .'|\
'diff --cached --check'|'diff --cached --quiet') ;;
  'push --atomic --set-upstream origin main refs/tags/v0.1.53')
    echo push >> "$COMMAND_LOG" ;;
  *) echo "unexpected Git mutation: $*" >&2; exit 97 ;;
esac
''')
            git.chmod(0o700)
            log = root / 'commands'
            env = dict(os.environ, PATH=str(root / 'bin') + ':' + os.environ['PATH'],
                       HOME=str(root), ORIG_HOME=str(root / 'standard'),
                       PROJECT_COPY_ROLE='build-release', DPDF_RELEASE_CHECKOUT=str(root),
                       COMMAND_LOG=str(log))
            result = subprocess.run(['bash', str(root / 'release.sh'), 'publish-preview'],
                                    env=env, text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(log.read_text().splitlines(),
                             ['push', 'handoff deliver --tag v0.1.53'])
            self.assertIn('TAGGED v0.1.53; PUBLICATION_QUEUED', result.stdout)

    def test_create_verify_then_existing_without_duplicate(self):
        api = API()
        self.assertEqual(page.publish(api, self.item())['status'], 'CREATED_VERIFIED')
        self.assertEqual(page.publish(api, self.item())['status'], 'EXISTING_VERIFIED')
        self.assertEqual(api.writes, 1)

    def test_ambiguous_post_is_read_back_not_created_twice(self):
        api = API(ambiguous=True)
        self.assertEqual(page.publish(api, self.item())['id'], 123)
        self.assertEqual(api.writes, 1)

    def test_remote_tag_mismatch_prevents_creation(self):
        api = API()
        api.commit = 'd' * 40
        with self.assertRaisesRegex(ValueError, 'Remote tag differs'):
            page.publish(api, self.item())
        self.assertEqual(api.writes, 0)

    def test_existing_changed_body_is_never_overwritten(self):
        api = API()
        page.publish(api, self.item())
        api.release['body'] = 'Owner edited this page'
        with self.assertRaisesRegex(ValueError, 'readback differs'):
            page.publish(api, self.item())
        self.assertEqual(api.writes, 1)

    def test_dry_run_and_auth_loss_preserve_verified_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            page.reconcile(self.item(), output, API(), apply=True)
            original = (output / 'v0.1.48.json').read_bytes()
            self.assertEqual(page.reconcile(self.item(), output)['status'], 'DRY_RUN')
            self.assertEqual(page.reconcile(self.item(), output, apply=True)['status'], 'PENDING_API_AUTH')
            self.assertEqual((output / 'v0.1.48.json').read_bytes(), original)

    def test_preview_gate_rejects_milestone_and_injection(self):
        for tag in ('v0.2.0', 'v0.1.48;true', '../v0.1.48'):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                page.preview_number(tag)

    def test_permission_failure_is_not_treated_as_missing_release(self):
        failed = subprocess.CompletedProcess([], 1, '', 'gh: Resource not accessible (HTTP 403)')
        with patch.object(page.subprocess, 'run', return_value=failed):
            with self.assertRaisesRegex(RuntimeError, 'API request failed'):
                page.GitHub()(page.ENDPOINT + '/releases/tags/v0.1.48', missing=True)

    def test_draft_readback_does_not_claim_published_page(self):
        api = API()
        page.publish(api, self.item())
        api.release['draft'] = True
        with self.assertRaisesRegex(ValueError, 'readback differs'):
            page.publish(api, self.item())
        self.assertEqual(api.writes, 1)

    def test_changed_source_does_not_replace_existing_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            item = self.item()
            page.reconcile(item, output, API(), apply=True)
            original = (output / 'v0.1.48.json').read_bytes()
            item['source_commit'] = 'e' * 40
            with self.assertRaisesRegex(ValueError, 'receipt identity differs'):
                page.reconcile(item, output, API(), apply=True)
            self.assertEqual((output / 'v0.1.48.json').read_bytes(), original)

    def test_notes_allow_only_privacy_redaction_not_release_fact_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            def git(*args):
                return subprocess.check_output(['git', '-C', str(repo), '-c', 'user.name=Fixture',
                    '-c', 'user.email=fixture@example.invalid', *args], stderr=subprocess.DEVNULL, text=True).strip()
            git('init', '-b', 'main')
            notes = repo / 'docs/releases/v0.1.48.md'
            notes.parent.mkdir(parents=True)
            notes.write_text('# dPDF v0.1.48 — development preview\nPrivate workstation-name\n')
            (repo / 'Cargo.toml').write_text('[package]\nname="dpdf"\nversion="0.1.48"\n')
            (repo / 'Cargo.lock').write_text('[[package]]\nname="dpdf"\nversion="0.1.48"\n')
            (repo / 'release.json').write_text(json.dumps(dict(tag='v0.1.48', version='0.1.48',
                channel='development-preview', notes='docs/releases/v0.1.48.md')))
            git('add', '.'); git('commit', '-m', 'Fixture preview'); git('tag', 'v0.1.48')
            notes.write_text('# dPDF v0.1.48 — development preview\nPrivate [private]\n')
            git('add', '.'); git('commit', '-m', 'Privacy redaction')
            git('update-ref', 'refs/remotes/origin/main', git('rev-parse', 'HEAD'))
            self.assertIn('[private]', page.plan(repo, 'v0.1.48', [re.compile('workstation-name')])['notes'])
            git('tag', 'v0.1.49')
            with self.assertRaisesRegex(ValueError, 'contract/version'):
                page.plan(repo, 'v0.1.49', [])
            notes.write_text('# dPDF v0.1.48 — development preview\nInvented production qualification\n')
            git('add', '.'); git('commit', '-m', 'Unapproved claim')
            git('update-ref', 'refs/remotes/origin/main', git('rev-parse', 'HEAD'))
            with self.assertRaisesRegex(ValueError, 'beyond explicit privacy'):
                page.plan(repo, 'v0.1.48', [])


if __name__ == '__main__':
    unittest.main()
