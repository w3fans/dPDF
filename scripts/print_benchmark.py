"""Generated-fixture virtual-print measurements; never submits a physical job."""
import hashlib
import json
from pathlib import Path
import time


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


class Benchmark:
    def __init__(self, backend, binary):
        self.report = dict(schema=1, scope='generated_virtual_print_only', backend=backend,
                           binary_sha256=sha256(binary),
                           timing_scope='dialog startup through completed PDF validation',
                           physical_printer_qualified=False, cases=[])

    def measure(self, name, source, output, pages, scaling, submit, validate):
        """Validate the completed PDF before recording any successful measurement."""
        source, output = Path(source), Path(output)
        original = sha256(source)
        source_bytes = source.stat().st_size
        started = time.monotonic()
        result = submit()
        validate(result)
        seconds = time.monotonic() - started
        if result.get('status') != 'submitted' or result.get('pages_drawn') != pages:
            raise ValueError('Virtual print did not submit the requested pages')
        if sha256(source) != original:
            raise ValueError('Virtual printing changed its source fixture')
        output_bytes = output.stat().st_size
        if source_bytes <= 0 or output_bytes <= 0:
            raise ValueError('Virtual print measurement requires non-empty PDFs')
        case = dict(name=name, pages=pages, scaling=scaling, seconds=seconds,
                    source_bytes=source_bytes, output_bytes=output_bytes,
                    output_to_source_ratio=output_bytes / source_bytes,
                    source_sha256=original, output_sha256=sha256(output),
                    source_preserved=True)
        self.report['cases'].append(case)
        print(f"BENCHMARK: {name}: {len(pages)} pages, {source_bytes} source bytes, "
              f"{output_bytes} output bytes, {case['output_to_source_ratio']:.2f}x, "
              f"{seconds:.2f}s including dialog and validation", flush=True)
        return result

    def write(self, path):
        if not self.report['cases']:
            raise ValueError('No completed print measurements to save')
        path = Path(path)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(self.report, indent=2) + '\n', encoding='utf-8')
