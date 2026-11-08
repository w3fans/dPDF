"""Source-safe checks that print measurements cannot hide failed output or mutation."""
import json
from pathlib import Path
import tempfile
import unittest

from print_benchmark import Benchmark, sha256


class Measurements(unittest.TestCase):
    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.addCleanup(self.folder.cleanup)
        self.root = Path(self.folder.name)
        self.source, self.output = self.root / 'source.pdf', self.root / 'output.pdf'
        self.source.write_bytes(b'%PDF-source-fixture')
        self.benchmark = Benchmark('fixture-virtual-printer', self.source)

    def submit(self):
        self.output.write_bytes(b'%PDF-completed-output-fixture')
        return dict(status='submitted', pages_drawn=[6])

    def measure(self, submit=None, validate=lambda result: None):
        return self.benchmark.measure('page-7', self.source, self.output, [6], 'fit',
                                      submit or self.submit, validate)

    def test_complete_output_identity_sizes_and_saved_report(self):
        self.measure()
        path = self.root / 'artifacts' / 'print.json'
        self.benchmark.write(path)
        report = json.loads(path.read_text())
        self.assertFalse(report['physical_printer_qualified'])
        self.assertEqual(report['scope'], 'generated_virtual_print_only')
        case = report['cases'][0]
        self.assertEqual(case['source_sha256'], sha256(self.source))
        self.assertEqual(case['output_sha256'], sha256(self.output))
        self.assertEqual(case['source_bytes'], self.source.stat().st_size)
        self.assertEqual(case['output_bytes'], self.output.stat().st_size)
        self.assertEqual(case['output_to_source_ratio'], case['output_bytes'] / case['source_bytes'])
        self.assertGreaterEqual(case['seconds'], 0)

    def test_validation_failure_never_records_a_success(self):
        def invalid(result):
            raise ValueError('Malformed or incomplete spooled PDF')
        with self.assertRaisesRegex(ValueError, 'incomplete'):
            self.measure(validate=invalid)
        self.assertEqual(self.benchmark.report['cases'], [])

    def test_source_mutation_missing_output_wrong_pages_and_cancellation(self):
        def mutate():
            result = self.submit()
            self.source.write_bytes(b'changed source')
            return result
        with self.assertRaisesRegex(ValueError, 'changed its source'):
            self.measure(mutate)
        self.output.unlink()
        for result in (dict(status='cancelled', pages_drawn=[]),
                       dict(status='submitted', pages_drawn=[0])):
            with self.assertRaisesRegex(ValueError, 'requested pages'):
                self.measure(lambda: result)
        with self.assertRaises(FileNotFoundError):
            self.measure(lambda: dict(status='submitted', pages_drawn=[6]))
        self.assertEqual(self.benchmark.report['cases'], [])
        with self.assertRaisesRegex(ValueError, 'No completed'):
            self.benchmark.write(self.root / 'invalid.json')


if __name__ == '__main__':
    unittest.main()
