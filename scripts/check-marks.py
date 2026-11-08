#!/usr/bin/env python3
"""Exercise visual-mark cleanup/library diagnostics without writing to the user's library."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/dpdf'))
    args = parser.parse_args()
    binary = str(args.binary.resolve())

    def run(*arguments):
        result = subprocess.run([binary, '--marks', *map(str, arguments)], capture_output=True,
                                text=True, check=True, timeout=30)
        return json.loads(result.stdout)

    def dimensions(path):
        data = path.read_bytes()
        assert data[:8] == b'\x89PNG\r\n\x1a\n'
        return struct.unpack('>II', data[16:24])

    with tempfile.TemporaryDirectory(prefix='dpdf-mark-acceptance-') as directory:
        directory = Path(directory)
        source, output, root = directory / 'original (photo) @scan.jpg', directory / 'cleaned.png', directory / 'library'
        source.write_bytes(Path(__file__).with_name('fixtures').joinpath('page-photo.jpg').read_bytes())
        original = hashlib.sha256(source.read_bytes()).digest()
        settings = directory / 'settings.json'
        settings.write_text(json.dumps({'crop': {'x':120,'y':20,'width':160,'height':150},
            'deskew_degrees': -3.0, 'remove_white': True, 'white_level':240, 'black_level':10}))
        angle = math.radians(3)
        expected = (math.ceil(160*math.cos(angle)+150*math.sin(angle)),
                    math.ceil(160*math.sin(angle)+150*math.cos(angle)))
        run('clean', source, output, settings)
        assert dimensions(output) == expected
        asset = run('import', source, 'Fixture signature', 'signature', root, settings)
        folder = root / asset['id']
        assert (folder / 'original').read_bytes() == source.read_bytes()
        assert (folder / 'cleaned.png').read_bytes() == output.read_bytes()
        assert dimensions(folder / 'thumbnail.png')[0] <= 160
        assert dimensions(folder / 'thumbnail.png')[1] <= 100
        assert tuple(asset['pixels']) == expected
        assert run('list', root)[0]['id'] == asset['id']
        height = 30*expected[1]/expected[0]
        saved = run('size', asset['id'], 30, height, root)
        assert abs(saved['preferred_width_mm']-30) < 0.001
        assert abs(run('list', root)[0]['preferred_height_mm']-height) < 0.001
        print('PASS: crop/deskew cleanup, private asset publication, original bytes, PNG/thumbnail and millimetre-size persistence')
        metadata = folder / 'metadata.json'
        before = metadata.read_bytes()
        for width, height in (('NaN','15'), ('30','30'), ('0','1')):
            result = subprocess.run([binary, '--marks', 'size', asset['id'], width, height, str(root)], capture_output=True, timeout=30)
            assert result.returncode != 0 and metadata.read_bytes() == before
        overwrite = subprocess.run([binary, '--marks', 'clean', str(source), str(source)], capture_output=True, timeout=30)
        assert overwrite.returncode != 0
        assert hashlib.sha256(source.read_bytes()).digest() == original
        future = json.loads(before)
        future['schema_version'] = 99
        metadata.write_text(json.dumps(future))
        before = metadata.read_bytes()
        rejected = subprocess.run([binary, '--marks', 'list', str(root)], capture_output=True, timeout=30)
        assert rejected.returncode != 0 and metadata.read_bytes() == before
        assert not any(path.name.startswith('.import-') for path in root.iterdir())
        print('PASS: invalid size/aspect, source overwrite and future schema fail without changing stored data')


if __name__ == '__main__':
    main()
