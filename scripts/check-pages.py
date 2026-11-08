#!/usr/bin/env python3
"""Verify qpdf page workflows, raw content preservation, rendered equality and failure safety."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import struct
import tempfile
import zlib

spec = importlib.util.spec_from_file_location("sample", Path(__file__).with_name("make-sample.py"))
sample = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sample)


def png(width, height, payload=None):
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    if payload is None:
        # Red at the left, blue at the right; half-transparent lower half.
        payload = b''.join(b'\0' + b''.join(bytes((190, 40, 30, 255 if y < height // 2 else 128))
                            if x < width // 2 else bytes((30, 60, 180, 255 if y < height // 2 else 128))
                            for x in range(width)) for y in range(height))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(payload)) + chunk(b'IEND', b''))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/dpdf"))
    parser.add_argument("--qpdf", type=Path, required=True)
    parser.add_argument("--pdfium", type=Path, required=True)
    args = parser.parse_args()
    binary, qpdf = str(args.binary.resolve()), str(args.qpdf.resolve())
    env = dict(os.environ, DPDF_QPDF_BINARY=qpdf, DPDF_PDFIUM_LIBRARY=str(args.pdfium.resolve()))

    def run(command):
        return subprocess.run(command, env=env, capture_output=True, check=True, timeout=60)

    def pages(path):
        return json.loads(run([qpdf, "--json", "--json-key=pages", str(path)]).stdout)["pages"]

    def outlines(path):
        data = json.loads(run([qpdf, '--json', '--json-key=outlines', str(path)]).stdout)['outlines']
        def flatten(nodes):
            return [item for node in nodes for item in [node, *flatten(node['kids'])]]
        return flatten(data)

    def streams(path):
        return [[run([qpdf, f"--show-object={reference.split()[0]},{reference.split()[1]}",
                      "--raw-stream-data", str(path)]).stdout for reference in page["contents"]]
                for page in pages(path)]

    def stream_data(path, reference, raw=True):
        numbers = reference.split()
        return run([qpdf, f"--show-object={numbers[0]},{numbers[1]}",
                    "--raw-stream-data" if raw else "--filtered-stream-data", str(path)]).stdout

    def checksum(path, page):
        result = run([binary, "--probe-page", str(path), str(page)]).stdout.decode()
        return re.search(r"rgba_checksum=([0-9a-f]+)", result).group(1)

    def assemble(kind, primary, output, selected, source_selected="", *sources):
        run([binary, "--assemble", kind, str(primary), str(output), selected, source_selected,
             *map(str, sources)])
        run([qpdf, "--check", str(output)])
        return streams(output)

    with tempfile.TemporaryDirectory(prefix="dpdf-page-acceptance-") as folder:
        folder = Path(folder)
        primary, signed, extra = [folder / name for name in ("contract (original).pdf", "signed last page.pdf", "extra @pages.pdf")]
        primary.write_bytes(sample.make_pdf(10))
        signed.write_bytes(sample.make_pdf(1, title="SIGNED FINAL PAGE"))
        extra.write_bytes(sample.make_pdf(2, title="INSERTED PAGE"))
        originals = {path: hashlib.sha256(path.read_bytes()).digest() for path in (primary, signed, extra)}
        original, signed_streams, extra_streams = streams(primary), streams(signed), streams(extra)

        result = folder / 'contract (final) @copy.pdf'
        replacement = assemble("replace", primary, result, "10", "1", signed)
        assert replacement[:9] == original[:9] and replacement[9] == signed_streams[0]
        for page in range(1, 10):
            assert checksum(primary, page) == checksum(result, page), page
        assert checksum(signed, 1) == checksum(result, 10)
        assert [node['destpageposfrom1'] for node in outlines(result)] == list(range(1, 11))
        print("PASS: contract page 10 replacement; pages 1-9 retain exact raw streams and rendered pixels")
        print('PASS: all ten original bookmarks retain their targets after last-page PDF replacement')

        assert assemble("extract", primary, folder / "extracted.pdf", "3,1") == [original[2], original[0]]
        assert all(not page["outlines"] for page in pages(folder / "extracted.pdf"))
        print("PASS: extraction order and clean catalog without unrelated outlines")
        assert assemble("remove", primary, folder / "removed.pdf", "2,4-5") == [original[i] for i in (0,2,5,6,7,8,9)]
        assert assemble("reorder", primary, folder / "reordered.pdf", "10,1-9") == [original[9], *original[:9]]
        print("PASS: remove and reorder preserve selected source streams")
        assert assemble("insert", primary, folder / "inserted.pdf", "2", "1-2", extra) == [original[0], *extra_streams, *original[1:]]
        assert assemble("merge", primary, folder / "merged.pdf", "", "", signed, extra) == [*original, *signed_streams, *extra_streams]
        print("PASS: insert and multi-PDF merge preserve order and source streams")
        for operation, angle in (("rotate-right", 90), ("rotate-left", 270)):
            output = folder / f"{operation}.pdf"
            assert assemble(operation, primary, output, "1") == original
            page = pages(output)[0]
            reference = page["object"].split()
            dictionary = run([qpdf, f"--show-object={reference[0]},{reference[1]}", str(output)]).stdout.decode()
            assert f"/Rotate {angle}" in dictionary
            rendered = run([binary, "--probe-page", str(output), "1"]).stdout.decode()
            width, height = map(int, re.search(r"first_frame=(\d+)x(\d+)", rendered).groups())
            assert width > height
        print("PASS: page rotation changes metadata/rendering while preserving content streams")

        scan, photo, oriented = [folder / name for name in ('signed scan.png', 'photo.jpg', 'phone-oriented.jpeg')]
        scan.write_bytes(png(300, 400))
        jpeg = Path(__file__).with_name('fixtures').joinpath('page-photo.jpg').read_bytes()
        photo.write_bytes(jpeg)
        exif = b'Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0'
        oriented.write_bytes(jpeg[:2] + b'\xff\xe1' + struct.pack('>H', len(exif)+2) + exif + jpeg[2:])
        originals.update({path: hashlib.sha256(path.read_bytes()).digest() for path in (scan, photo, oriented)})
        for image, expected_dimensions in ((scan, (300, 400)), (photo, (400, 200)), (oriented, (200, 400))):
            output = folder / f'{image.stem}-replacement.pdf'
            assert assemble('replace', primary, output, '10', '1', image)[:9] == original[:9]
            last = pages(output)[9]
            assert len(last['images']) == 1
            assert [node['destpageposfrom1'] for node in outlines(output)] == list(range(1, 11))
            embedded = last['images'][0]
            assert (embedded['width'], embedded['height']) == expected_dimensions, embedded
            if image == photo:
                assert '/DCTDecode' in embedded['filter']
                assert stream_data(output, embedded['object']) == jpeg
            content = b' '.join(stream_data(output, ref, raw=False) for ref in last['contents']).decode()
            matrix = re.search(r'([\d.]+) 0 0 ([\d.]+) ([\d.]+) ([\d.]+) cm', content)
            assert matrix, content
            width, height, x, y = map(float, matrix.groups())
            assert abs(width / height - expected_dimensions[0] / expected_dimensions[1]) < 0.001
            assert abs(x * 2 + width - 612) < 0.02 and abs(y * 2 + height - 792) < 0.02
            assert checksum(output, 1) == checksum(primary, 1)
            checksum(output, 10)
        print('PASS: PNG/JPEG replacement, alpha image, phone orientation, aspect/centering, and original JPEG stream preservation')
        navigation = folder / 'bookmark-kinds.pdf'
        objects = [
            b'<< /Type /Catalog /Pages 2 0 R /Outlines 9 0 R /Names << /Dests << /Names [(last) [5 0 R /Fit]] >> >> >>',
            b'<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R] /Count 3 >>',
        ]
        objects.extend(f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << >> /Contents {i+6} 0 R >>'.encode() for i in range(3))
        objects.extend([b'<< /Length 0 >>\nstream\nendstream']*3)
        objects.extend([
            b'<< /Type /Outlines /First 10 0 R /Last 14 0 R /Count 4 >>',
            b'<< /Title (Direct) /Parent 9 0 R /Dest [5 0 R /XYZ null null 1.25] /First 12 0 R /Last 12 0 R /Count 1 /Next 11 0 R /C [0 0 1] /F 2 >>',
            b'<< /Title (Action) /Parent 9 0 R /Prev 10 0 R /Next 14 0 R /A 13 0 R >>',
            b'<< /Title (Named) /Parent 10 0 R /Dest (last) >>',
            b'<< /S /GoTo /D [5 0 R /Fit] /Next << /S /URI /URI (https://example.invalid/after) >> >>',
            b'<< /Title (External) /Parent 9 0 R /Prev 11 0 R /A << /S /URI /URI (https://example.invalid/) >> >>',
        ])
        navigation.write_bytes(sample.write_pdf(objects))
        originals[navigation] = hashlib.sha256(navigation.read_bytes()).digest()
        for mode in ('replace', 'overlay'):
            output = folder / ('navigation-'+mode+'.pdf')
            if mode == 'replace':
                assemble('replace', navigation, output, '3', '1', scan)
            else:
                recipe = folder / 'mark.json'
                recipe.write_text(json.dumps([dict(page=3, image=scan.name,
                    geometry=dict(x_mm=10, y_mm=10, width_mm=20, height_mm=10))]))
                run([binary, '--overlay', str(navigation), str(output), str(recipe)])
            nodes = outlines(output)
            assert [node['title'] for node in nodes] == ['Direct', 'Named', 'Action', 'External']
            assert [node['destpageposfrom1'] for node in nodes] == [3, 3, 3, None]
            assert nodes[0]['dest'][1:] == ['/XYZ', None, None, 1.25]
            assert nodes[1]['dest'][1:] == nodes[2]['dest'][1:] == ['/Fit']
            assert streams(navigation)[:2] == streams(output)[:2]
            full = json.loads(run([qpdf, '--json', '--json-stream-data=none', str(output)]).stdout)
            direct = full['qpdf'][1]['obj:'+nodes[0]['object']]['value']
            external = full['qpdf'][1]['obj:'+nodes[3]['object']]['value']
            action = full['qpdf'][1]['obj:'+nodes[2]['object']]['value']['/A']
            assert action['/S'] == '/GoTo' and action['/Next']['/URI'] == 'u:https://example.invalid/after'
            assert direct['/C'] == [0, 0, 1] and direct['/F'] == 2
            assert external['/A']['/URI'] == 'u:https://example.invalid/'
        print('PASS: nested/direct/action/named bookmarks, destination zoom, styling and external actions survive PDF/image/mark page replacement')
        inserted_image = folder / 'image-inserted.pdf'
        assert len(assemble('insert', primary, inserted_image, '11', '', scan)) == 11
        assert pages(inserted_image)[10]['images'][0]['width'] == 300
        assert len(assemble('merge', primary, folder / 'mixed-merge.pdf', '', '', photo, extra, scan)) == 14
        landscape = folder / 'landscape.pdf'
        landscape.write_bytes(sample.make_pdf(1, rotation=90, cropped=True))
        output = folder / 'landscape-scan.pdf'
        assemble('replace', landscape, output, '1', '1', scan)
        rendered = run([binary, '--probe', str(output)]).stdout.decode()
        width, height = map(int, re.search(r'first_frame=(\d+)x(\d+)', rendered).groups())
        assert width > height
        print('PASS: image insertion, mixed PDF/image merge, and rotated/cropped target replacement')

        sentinel = folder / "existing-output.pdf"
        sentinel.write_bytes(b"previous destination must survive failure")
        malformed_navigation = folder / 'unsupported-bookmark.pdf'
        malformed_navigation.write_bytes(sample.write_pdf([
            obj.replace(b'/XYZ null null 1.25', b'/XYZ (bad coordinate) null 1.25')
            for obj in objects
        ]))
        before = hashlib.sha256(malformed_navigation.read_bytes()).digest()
        rejected = subprocess.run([binary, '--assemble', 'replace', str(malformed_navigation),
            str(sentinel), '3', '1', str(scan)], env=env, capture_output=True, timeout=60)
        assert rejected.returncode != 0
        assert b'bookmark coordinate' in rejected.stderr.lower(), rejected.stderr
        assert sentinel.read_bytes() == b'previous destination must survive failure'
        assert hashlib.sha256(malformed_navigation.read_bytes()).digest() == before
        print('PASS: unsupported bookmark coordinates preserve the source and prior destination')
        oversized, corrupt = folder / 'oversized.png', folder / 'corrupt.png'
        oversized.write_bytes(png(9000, 4000, b'not a decoded image'))
        corrupt.write_bytes(b'\x89PNG\r\n\x1a\nnot an image')
        for image in (oversized, corrupt):
            attempt = subprocess.run([binary, '--assemble', 'replace', str(primary), str(sentinel), '10', '1', str(image)], env=env, capture_output=True, timeout=60)
            assert attempt.returncode != 0
            assert sentinel.read_bytes() == b'previous destination must survive failure'
        for operation, selection in (("replace", "99"), ("remove", "1-10"), ("reorder", "1,1")):
            attempt = subprocess.run([binary, "--assemble", operation, str(primary), str(sentinel), selection, "1", *([str(signed)] if operation == "replace" else [])], env=env, capture_output=True, timeout=60)
            assert attempt.returncode != 0
            assert sentinel.read_bytes() == b"previous destination must survive failure"
        overwrite = subprocess.run([binary, "--assemble", "extract", str(primary), str(primary), "1", ""], env=env, capture_output=True, timeout=60)
        assert overwrite.returncode != 0
        for path, digest in originals.items():
            assert hashlib.sha256(path.read_bytes()).digest() == digest, path
        print("PASS: invalid operations preserve prior output; source overwrite is refused; all inputs unchanged")


if __name__ == "__main__":
    main()
