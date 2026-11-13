"""Collect target-scoped Cargo notices without retaining machine-local paths."""
import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import tomllib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--source', type=Path, required=True)
parser.add_argument('--cargo', required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--target', choices=['x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu'],
                    default='x86_64-pc-windows-msvc')
parser.add_argument('--retained', type=Path, help='Require the collected inventory to match this retained JSON')
args = parser.parse_args()
source = args.source.resolve()
tools = Path(__file__).resolve().parents[2]
args.output.mkdir(parents=True, exist_ok=True)
metadata_path, tree_path = args.output / 'metadata.json', args.output / 'tree.txt'
for output, command in [
    (metadata_path, ['metadata', '--locked', '--format-version', '1', '--filter-platform', args.target]),
    (tree_path, ['tree', '--locked', '--target', args.target, '--edges', 'normal,build', '--prefix', 'none', '--format', '{p}'])
]:
    with output.open('wb') as file:
        subprocess.run([args.cargo, *command, '--manifest-path', str(source / 'Cargo.toml')],
                       stdout=file, check=True, timeout=120)
spec = importlib.util.spec_from_file_location('notices', tools / 'scripts' / 'collect-cargo-notices.py')
notices = importlib.util.module_from_spec(spec)
spec.loader.exec_module(notices)
inventory = notices.collect(json.loads(metadata_path.read_text(encoding='utf-8')),
    tomllib.loads((source / 'Cargo.lock').read_text(encoding='utf-8')), args.target,
    args.output / 'bundle', tree_path.read_text(encoding='utf-8'),
    tools / 'packaging' / 'licenses' / 'rust' / 'overrides.json',
    tomllib.loads((source / 'Cargo.toml').read_text(encoding='utf-8')))
# Existing source blobs need not be duplicated in Git diagnostics. Preserve new
# bytes losslessly for the maintainer to import through the normal source workflow.
known = tools / 'packaging' / 'licenses' / 'rust' / 'texts'
extra = {path.stem: path.read_bytes().hex() for path in (args.output / 'bundle' / 'texts').iterdir()
         if not (known / path.name).is_file()}
summary = dict(schema=1, source_commit=subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip(),
               inventory=inventory, additional_texts_hex=extra)
(args.output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n', encoding='utf-8')
metadata_path.unlink()  # The portable inventory contains no machine-local manifest paths.
tree_path.unlink()
print(json.dumps(dict(packages=len(inventory['components']), missing=inventory['missing_license_text'],
                      status='inventory_only', qualified_for_binary_distribution=False)))

if args.retained and json.loads(args.retained.read_text(encoding='utf-8')) != inventory:
    raise RuntimeError('Collected Cargo notice inventory differs from retained target inventory')
