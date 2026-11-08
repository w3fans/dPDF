#!/usr/bin/env python3
"""Check retained notice bytes/configuration, without claiming legal clearance."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', choices=['linux', 'windows'], required=True)
    args = parser.parse_args()
    source = Path(__file__).resolve().parent.parent
    spec = importlib.util.spec_from_file_location('collector', Path(__file__).with_name('collect-cargo-notices.py'))
    collector = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(collector)
    root = source / 'packaging' / 'licenses' / 'rust'
    data = json.loads((root / ('inventory-' + args.target + '.json')).read_text(encoding='utf-8'))
    config = tomllib.loads((source / 'Cargo.toml').read_text(encoding='utf-8'))
    lock = tomllib.loads((source / 'Cargo.lock').read_text(encoding='utf-8'))
    assert data['config_fingerprint'] == collector.config_fingerprint(config), 'Regenerate notices after dependency/feature configuration changes'
    assert data['lock_fingerprint'] == collector.lock_fingerprint(lock, {config['package']['name']}), 'Regenerate notices after lockfile dependency changes'
    assert data['components'] and not data['missing_license_text']
    assert data['audit_status'] == 'inventory_only' and not data['qualified_for_binary_distribution']
    packages = {(p['name'], p['version'], p.get('checksum')) for p in lock['package']}
    count = 0
    for component in data['components']:
        assert (component['name'], component['version'], component['registry_checksum']) in packages
        assert component['license_declaration'] and component['notices']
        for notice in component['notices']:
            digest = notice['sha256']
            assert re.fullmatch('[0-9a-f]{64}', digest)
            assert notice['text'] == 'texts/' + digest + '.txt'
            path = root / notice['text']
            assert path.resolve().is_relative_to(root.resolve())
            assert hashlib.sha256(path.read_bytes()).hexdigest() == digest, path
            count += 1
    print(f"PASS: {args.target} notice inventory for {len(data['components'])} packages / {count} file references; hashes and dependency configuration match (inventory only)")


if __name__ == '__main__':
    main()
