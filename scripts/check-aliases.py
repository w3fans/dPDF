#!/usr/bin/env python3
"""Exercise protected-file identity using disposable aliases, never private files."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile

spec = importlib.util.spec_from_file_location('page_fixture', Path(__file__).with_name('check-pages.py'))
fixtures = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixtures)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/dpdf'))
    parser.add_argument('--pdfium', type=Path, required=True)
    parser.add_argument('--qpdf', type=Path, required=True)
    args = parser.parse_args()
    binary = str(args.binary.resolve())
    env = dict(os.environ, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()), DPDF_QPDF_BINARY=str(args.qpdf.resolve()))
    with tempfile.TemporaryDirectory(prefix='dpdf-file-identity-') as directory:
        folder = Path(directory)
        pdf, other, image, recipe = [folder / name for name in ('Pogodba ŽČŠ.pdf', 'replacement.pdf', 'Žig.png', 'placements.json')]
        pdf.write_bytes(fixtures.sample.make_pdf(3))
        other.write_bytes(fixtures.sample.make_pdf(1))
        image.write_bytes(fixtures.png(120, 80))
        recipe.write_text(json.dumps([dict(page=1, image=image.name, geometry=dict(x_mm=25, y_mm=40, width_mm=40, height_mm=20))]))
        originals = {path: hashlib.sha256(path.read_bytes()).digest() for path in (pdf, other, image, recipe)}
        cases = [
            (pdf, ['--assemble', 'remove', str(pdf), '{output}', '1', '']),
            (other, ['--assemble', 'replace', str(pdf), '{output}', '3', '1', str(other)]),
            (image, ['--assemble', 'replace', str(pdf), '{output}', '3', '1', str(image)]),
            (pdf, ['--overlay', str(pdf), '{output}', str(recipe)]),
            (image, ['--overlay', str(pdf), '{output}', str(recipe)]),
            (recipe, ['--overlay', str(pdf), '{output}', str(recipe)]),
            (image, ['--marks', 'clean', str(image), '{output}']),
            (pdf, ['--render-page', str(pdf), '1', '{output}']),
        ]

        def refused(source, output, command):
            result = subprocess.run([binary, *[str(output) if item == '{output}' else item for item in command]], env=env, capture_output=True, timeout=60)
            assert result.returncode != 0, (source, output, command, result.stdout)
            assert b'preserve' in result.stderr or b'protected input' in result.stderr, result.stderr
            for path, digest in originals.items():
                assert hashlib.sha256(path.read_bytes()).digest() == digest, (path, output)
            assert hashlib.sha256(output.read_bytes()).digest() == originals[source]

        for index, (source, command) in enumerate(cases):
            alias = folder / f'hard link {index}.output'
            os.link(source, alias)
            refused(source, alias, command)
        print('PASS: PDF/source-page/image/recipe/cleanup/raster aliases refused by file identity; bytes unchanged')
        if os.name != 'nt':
            alias_folder = folder / 'directory alias'
            alias_folder.symlink_to(folder, target_is_directory=True)
            for source, command in cases:
                refused(source, alias_folder / source.name, command)
            print('PASS: symlink-directory aliases refused; bytes unchanged')
        else:
            # Reuse an existing loopback administrative share only if accessible.
            # Never create shares or change security to make a diagnostic pass.
            if folder.drive.endswith(':'):
                unc = Path('\\\\localhost\\' + folder.drive[0] + '$' + str(folder)[2:])
                if (unc / pdf.name).is_file():
                    for source, command in cases:
                        refused(source, unc / source.name, command)
                    print('PASS: local-drive / loopback UNC aliases refused; bytes unchanged')
                else:
                    print('SKIP: existing loopback administrative share is not accessible')


if __name__ == '__main__':
    main()
