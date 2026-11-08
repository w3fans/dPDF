"""Source-safe CI contract checks plus native Windows process-tree qualification."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import publish_report
import failed_job_report
import runtime
import validate_windows
import windows_job
import validate_linux


class Contracts(unittest.TestCase):
    def test_print_benchmarks_are_artifacts_only_in_desktop_lanes(self):
        report = Path('artifacts') / 'print-benchmark.json'
        linux = validate_linux.plan(Path('source'), Path('binary'), Path('pdfium'), Path('qpdf'),
                                    'Xvfb', print_report=report)
        windows = validate_windows.plan(Path('source'), Path('binary'), 'pdfium', 'qpdf', 'desktop',
                                        print_report=report)
        for name, checks in [('native-print', linux), ('windows-print', windows)]:
            command = next(command for check, command, _ in checks if check == name)
            self.assertIn('--stress', command)
            self.assertEqual(command[command.index('--benchmark-report') + 1], str(report))
        self.assertFalse(any(name == 'native-print' for name, _, _ in
                             validate_linux.plan(Path('s'), Path('b'), Path('p'), Path('q'),
                                                 'Xvfb', desktop=False)))
        self.assertFalse(any(name == 'windows-print' for name, _, _ in
                             validate_windows.plan(Path('s'), Path('b'), 'p', 'q', 'service')))

    def test_archive_paths(self):
        for value in ('../outside', '/root', 'C:/outside', 'bin/../../file', '..\\outside'):
            with self.assertRaises(ValueError):
                runtime.safe_member(value)
        self.assertEqual(str(runtime.safe_member('qpdf/bin/qpdf.exe')), 'qpdf/bin/qpdf.exe')

    def test_no_false_pass_or_wrong_commit(self):
        commit = 'a' * 40
        report = dict(schema=1, source_commit=commit, lane='self-hosted', status='PASS', checks=[])
        with self.assertRaises(ValueError):
            publish_report.validate(report, commit, 'self-hosted')
        report['status'] = 'QUEUED'
        with self.assertRaises(ValueError):
            publish_report.validate(report, commit, 'self-hosted')
        publish_report.validate(report, commit, 'self-hosted', allow_pending=True)
        report['status'] = 'PASS'
        report['checks'] = [dict(name='build', status='PASS')]
        publish_report.validate(report, commit, 'self-hosted')
        for sha, lane in [('b' * 40, 'self-hosted'), (commit, 'hosted')]:
            with self.assertRaises(ValueError):
                publish_report.validate(report, sha, lane)
        report['checks'].append(dict(name='reader', status='SKIP'))
        with self.assertRaises(ValueError):
            publish_report.validate(report, commit, 'self-hosted')

    def test_cancellation_does_not_claim_qualification_or_hide_failures(self):
        env = dict(VALIDATED_COMMIT='a'*40, VALIDATION_REF='main', VALIDATION_LANE='hosted',
                   GITHUB_RUN_ATTEMPT='1', VALIDATION_RESULT='cancelled', GITHUB_REPOSITORY='fixture/repo', GITHUB_RUN_ID='1')
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            report = failed_job_report.finalize(folder, env)
            self.assertEqual(report['status'], 'CANCELLED')
            publish_report.validate(report, 'a'*40, 'hosted')
            for status, checks, expected in [
                ('FAIL', [dict(name='build', status='PASS')], 'CANCELLED'),
                ('FAIL', [dict(name='build', status='FAIL')], 'FAIL'),
                ('PASS', [dict(name='build', status='PASS')], 'PASS'),
            ]:
                report.update(status=status, checks=checks)
                report.pop('error', None)
                (folder / 'report.json').write_text(json.dumps(report))
                final = failed_job_report.finalize(folder, env)
                self.assertEqual(final['status'], expected)
                publish_report.validate(final, 'a'*40, 'hosted')
            report.update(status='CANCELLED', checks=[dict(name='build', status='FAIL')])
            with self.assertRaises(ValueError):
                publish_report.validate(report, 'a'*40, 'hosted')

    def test_service_has_no_desktop_actions_and_uses_native_build_script(self):
        plan = validate_windows.plan(Path('source'), Path('target/dpdf.exe'), 'pdfium.dll', 'qpdf.exe', 'service')
        names = [name for name, _, _ in plan]
        self.assertIn('user-registration', names)
        self.assertIn('aliases', names)
        self.assertIn('review', names)
        self.assertIn('geometry', names)
        self.assertIn('block-probe', names)
        self.assertIn('engine-paths', names)
        self.assertIn('page-units', names)
        self.assertNotIn('windows-print', names)
        self.assertNotIn('passwords', names)
        self.assertFalse(any(name.startswith('clipboard-') for name in names))
        self.assertFalse(any(name.startswith('document-tabs-') for name in names))
        self.assertFalse(any(name.startswith('viewport-') for name in names))
        self.assertFalse(any(name.startswith('physical-reader-') for name in names))
        self.assertFalse(any(name.startswith('native-file-drop-') for name in names))
        self.assertIn('source/build.ps1', [str(arg).replace('\\', '/') for arg in plan[0][1]])
        for name, command, _ in plan:
            if name in ('reader', 'pages', 'marks', 'overlays', 'page-units', 'review', 'aliases'):
                self.assertIn('--binary', command)
        desktop = validate_windows.plan(Path('source'), Path('binary'), 'pdfium', 'qpdf', 'desktop')
        self.assertIn('windows-print', [name for name, _, _ in desktop])
        clipboard = [command for name, command, _ in desktop if name.startswith('clipboard-')]
        self.assertEqual(len(clipboard), 6)
        self.assertTrue(all('--allow-native-clipboard' in command for command in clipboard))
        tabs = [command for name, command, _ in desktop if name.startswith('document-tabs-')]
        self.assertEqual(len(tabs), 3)
        self.assertTrue(all('--pdfium' in command and '--scale' in command for command in tabs))
        physical = [command for name, command, _ in desktop if name.startswith('physical-reader-')]
        self.assertEqual(len(physical), 3)
        self.assertTrue(all('--qpdf' in command and '--scale' in command for command in physical))
        viewport = [command for name, command, _ in desktop if name.startswith('viewport-')]
        self.assertEqual(len(viewport), 3)
        drops = [command for name, command, _ in desktop if name.startswith('native-file-drop-')]
        self.assertEqual(len(drops), 3)
        self.assertTrue(all('--allow-native-input' in command for command in drops))

    def test_block_probe_report_keeps_exact_identity_and_unqualified_scope(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'probe.json'
            report = dict(scope='generated_rust_block_probe_only', binary_sha256='fixture-hash',
                          qualified_for_secure_export=False, dPDF_secure_export_enabled=False,
                          requested_cases=list(range(32)), status='INCOMPLETE', cases=[], refusals=[])
            path.write_text(json.dumps(report))
            self.assertEqual(validate_windows.load_block_report(path, 'fixture-hash')['status'], 'INCOMPLETE')
            with self.assertRaises(ValueError):
                validate_windows.load_block_report(path, 'other-build')
            report['status'] = 'PASS'
            path.write_text(json.dumps(report))
            with self.assertRaises(ValueError):
                validate_windows.load_block_report(path, 'fixture-hash')
            report.update(cases=[dict(case=i) for i in range(32)], refusals=[dict(case=name) for name in validate_windows.BLOCK_REFUSALS],
                          graph_cases=[dict(case='indirect-resources')])
            path.write_text(json.dumps(report))
            self.assertEqual(validate_windows.load_block_report(path, 'fixture-hash')['status'], 'PASS')
            report['refusals'][-1] = dict(case='catalog-stream')
            path.write_text(json.dumps(report))
            with self.assertRaises(ValueError):
                validate_windows.load_block_report(path, 'fixture-hash')
            report['refusals'][-1] = dict(case=validate_windows.BLOCK_REFUSALS[-1])
            report['graph_cases'] = []
            path.write_text(json.dumps(report))
            with self.assertRaises(ValueError):
                validate_windows.load_block_report(path, 'fixture-hash')
            report['graph_cases'] = [dict(case='indirect-resources')]
            report['qualified_for_secure_export'] = True
            path.write_text(json.dumps(report))
            with self.assertRaises(ValueError):
                validate_windows.load_block_report(path, 'fixture-hash')

    def test_isolates_smoke_flags_and_data(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.dict(os.environ, {'DPDF_SMOKE_PASSWORD': 'secret', 'DPDF_TEST_NATIVE_PRINT': '1',
                                         'DPDF_DATA_DIR': 'personal', 'CARGO_BUILD_JOBS': '50', 'PYTHONOPTIMIZE': '2'}):
                env = validate_windows.environment(root, root / 'build')
            self.assertNotIn('DPDF_SMOKE_PASSWORD', env)
            self.assertNotIn('DPDF_TEST_NATIVE_PRINT', env)
            self.assertEqual(env['DPDF_DATA_DIR'], str(root / 'data'))
            self.assertEqual(env['CARGO_BUILD_JOBS'], '1')
            self.assertEqual(env['PYTHONOPTIMIZE'], '0')

    def test_report_branch_update_preserves_application_and_main(self):
        # Use disposable local Git repositories, with no credentials or hooks.
        script = Path(__file__).with_name('publish_report.py').resolve()
        env = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM='1',
                   GITHUB_REPOSITORY='fixture/repo')
        with tempfile.TemporaryDirectory(prefix='dpdf-ci-publication-test-') as directory:
            root = Path(directory)
            repo, remote, artifact = root / 'repo', root / 'remote.git', root / 'artifact'
            artifact.mkdir()

            def git(path, *args):
                return subprocess.check_output(
                    ['git', '-C', str(path), '-c', 'core.hooksPath=' + os.devnull,
                     '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', *args],
                    env=env, text=True, stderr=subprocess.DEVNULL).strip()

            for command in (['git', 'init', '--bare', str(remote)],
                            ['git', 'init', '-b', 'main', str(repo)]):
                subprocess.run(command, env=env, check=True, capture_output=True)
            (repo / 'application.rs').write_text('unchanged application\n')
            git(repo, 'add', 'application.rs')
            git(repo, 'commit', '-m', 'fixture')
            commit = git(repo, 'rev-parse', 'HEAD')
            git(repo, 'remote', 'add', 'origin', str(remote))
            git(repo, 'push', 'origin', 'main')
            report = dict(schema=1, source_commit=commit, lane='hosted', status='QUEUED', checks=[],
                          run_url=f"https://github.com/{env.get('GITHUB_REPOSITORY', 'fixture/local')}/actions/runs/123",
                          run_attempt='1')
            (artifact / 'README.md').write_text('Generated report\n')
            (artifact / 'actions-jobs.json').write_text('{}\n')
            command = [sys.executable, str(script), '--artifact', str(artifact), '--commit',
                       commit, '--lane', 'hosted', '--run', '123-1']
            (artifact / 'report.json').write_text(json.dumps(report))
            subprocess.run([*command, '--pending'], cwd=repo, env=env, check=True, capture_output=True)
            branch = 'validation/ci-windows-hosted-123-1'
            queued = git(remote, 'rev-parse', branch)
            report['status'] = 'FAIL'
            report['checks'] = [dict(name='clipboard-placement-1-5', status='FAIL')]
            (artifact / 'logs').mkdir()
            (artifact / 'logs' / 'clipboard-placement-1-5.log').write_text('Fixture scale mismatch\n')
            (artifact / 'bootstrap.log').write_text('Native startup failed.\n', encoding='utf-16')
            (artifact / 'report.json').write_text(json.dumps(report))
            subprocess.run(command, cwd=repo, env=env, check=True, capture_output=True)
            final = git(remote, 'rev-parse', branch)
            final_report = json.loads(git(remote, 'show', branch + ':docs/validation/automated/report.json'))
            self.assertIn('Native startup failed.', final_report['failure_logs'][0]['tail'])
            self.assertTrue(any(item['name'] == 'clipboard-placement-1-5' and 'Fixture scale mismatch' in item['tail']
                                for item in final_report['failure_logs']))
            self.assertNotEqual(queued, final)
            self.assertEqual(git(remote, 'rev-parse', final + '^'), queued)
            self.assertEqual(git(repo, 'rev-parse', 'HEAD'), commit)
            self.assertEqual(git(remote, 'rev-parse', 'main'), commit)
            self.assertEqual(git(remote, 'show', branch + ':application.rs'), 'unchanged application')
            changed = git(remote, 'diff', '--name-only', commit, final).splitlines()
            self.assertEqual(set(changed), {'docs/validation/automated/' + name for name in
                                           ('README.md', 'report.json', 'actions-jobs.json')})
            report['status'] = 'QUEUED'
            (artifact / 'report.json').write_text(json.dumps(report))
            attempt = subprocess.run([*command, '--pending'], cwd=repo, env=env, capture_output=True)
            self.assertNotEqual(attempt.returncode, 0)
            self.assertEqual(git(remote, 'rev-parse', branch), final)
            report.update(status='FAIL', run_attempt='0')
            (artifact / 'report.json').write_text(json.dumps(report))
            mismatch = subprocess.run(command, cwd=repo, env=env, capture_output=True)
            self.assertNotEqual(mismatch.returncode, 0)
            self.assertEqual(git(remote, 'rev-parse', branch), final)

    @unittest.skipUnless(os.name == 'nt', 'Native Windows clipboard opt-in guard')
    def test_clipboard_requires_explicit_opt_in(self):
        script = Path(__file__).resolve().parents[1] / 'check-clipboard.py'
        result = subprocess.run([sys.executable, str(script), '--marks'], capture_output=True,
                                text=True, timeout=10)
        self.assertEqual(result.returncode, 2)
        self.assertIn('--allow-native-clipboard', result.stderr)

    @unittest.skipUnless(os.name == 'nt', 'Native Windows pointer input opt-in guard')
    def test_file_drop_requires_explicit_opt_in(self):
        script = Path(__file__).resolve().parents[1] / 'check-windows-drop.py'
        env = dict(os.environ, DPDF_TEST_NATIVE_DISPLAY='1')
        result = subprocess.run([sys.executable, str(script), '--binary', 'unused.exe',
                                 '--pdfium', 'unused.dll'], env=env, capture_output=True,
                                text=True, timeout=10)
        self.assertEqual(result.returncode, 2)
        self.assertIn('--allow-native-input', result.stderr)

    @unittest.skipUnless(os.name == 'nt', 'Native Windows fixture sharing violation')
    def test_file_drop_workspace_waits_for_windows_log_release(self):
        import threading
        script = Path(__file__).resolve().parents[1] / 'check-windows-drop.py'
        spec = importlib.util.spec_from_file_location('drop_cleanup', script)
        drop = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(drop)
        handle = timer = None
        try:
            with drop.fixture_workspace() as directory:
                folder = Path(directory)
                handle = (folder / 'reader.log').open('wb')
                timer = threading.Timer(.3, handle.close)
                timer.start()
            self.assertFalse(folder.exists(), 'Fixture cleanup did not remove the released log')
        finally:
            if timer:
                timer.join(timeout=2)
            if handle:
                handle.close()

    @unittest.skipUnless(os.name == 'nt', 'Native Windows Job Object test')
    def test_timeout_kills_descendants(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / 'escaped.txt'
            child = f"import time,pathlib;time.sleep(4);pathlib.Path({str(marker)!r}).write_text('escaped')"
            parent = 'import subprocess,sys,time;subprocess.Popen([sys.executable,"-c",sys.argv[1]]);time.sleep(30)'
            code = windows_job.run([sys.executable, '-c', parent, child], root, dict(os.environ), root / 'job.log', 1)
            self.assertEqual(code, 124)
            time.sleep(5)
            self.assertFalse(marker.exists(), 'Timed-out child escaped the Job Object')


if __name__ == '__main__':
    unittest.main()
