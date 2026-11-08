#!/usr/bin/env python3
"""Headless reader acceptance against original fixtures; requires a built executable and PDFium."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

from importlib.util import module_from_spec, spec_from_file_location

spec = spec_from_file_location("sample", Path(__file__).with_name("make-sample.py"))
sample = module_from_spec(spec)
spec.loader.exec_module(sample)


def rotate(rect, degrees):
    x, y, width, height = rect
    if degrees == 90:
        return [1 - y - height, x, height, width]
    if degrees == 180:
        return [1 - x - width, 1 - y - height, width, height]
    if degrees == 270:
        return [y, 1 - x - width, height, width]
    return rect


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/dpdf"))
    parser.add_argument("--pdfium", type=Path)
    args = parser.parse_args()
    binary = str(args.binary.resolve())
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    if args.pdfium:
        env["DPDF_PDFIUM_LIBRARY"] = str(args.pdfium.resolve())

    def search(path, query):
        result = subprocess.run([binary, "--probe-search", str(path), query], env=env,
                                text=True, capture_output=True, check=True, timeout=30)
        return json.loads(result.stdout)

    def select(path, rect):
        result = subprocess.run([binary, '--probe-select', str(path), '1', *map(str, rect)], env=env,
                                text=True, capture_output=True, check=True, timeout=30)
        return json.loads(result.stdout)['text']

    with tempfile.TemporaryDirectory(prefix="dpdf-reader-acceptance-") as folder:
        folder = Path(folder)
        plain = folder / "50-pages.pdf"
        plain.write_bytes(sample.make_pdf(50))
        for query in ("payment", "PAYMENT"):
            result = search(plain, query)
            assert result["matches"] == 100 and result["rectangles"] == 100, result
            assert not result["truncated"], result
        for query in ("", "a phrase absent from the sample"):
            assert search(plain, query)["matches"] == 0
        print("PASS: occurrence counts, case-insensitive, empty and absent queries")

        for cropped in (False, True):
            reference = None
            for degrees in (0, 90, 180, 270):
                path = folder / f"crop-{cropped}-rotation-{degrees}.pdf"
                path.write_bytes(sample.make_pdf(3, degrees, cropped))
                # Use a unique phrase per page: intrinsic rotation can change the
                # extraction order of repeated words, so 'first payment' is ambiguous.
                result = search(path, "payment due")
                assert result["matches"] == 3 and result["rectangles"] == 3, result
                rect = result["first"]
                assert len(rect) == 4 and rect[2] > 0 and rect[3] > 0, result
                assert all(-0.001 <= value <= 1.001 for value in rect), result
                if degrees == 0:
                    reference = rect
                expected = rotate(reference, degrees)
                assert all(abs(a - b) < 0.001 for a, b in zip(rect, expected)), (cropped, degrees, rect, expected)
                x, y, width, height = rect
                selected = select(path, [max(0, x-0.001), max(0, y-0.001), width+0.002, height+0.002])
                assert selected.strip() == 'payment due', (cropped, degrees, selected)
                assert 'Authorized signature:' in select(path, [0, 0, 1, 1])
                assert not select(path, rotate([0.01, 0.5, 0.1, 0.1], degrees)).strip()
            print(f"PASS: highlight geometry at 0/90/180/270 degrees, cropped={cropped}")
        print('PASS: bounded text selection at all intrinsic rotations/crop offsets; full-page and empty regions')
        for rect in ([0, 0, 0, 0.1], [0, 0, float('nan'), 0.1], [0.9, 0.9, 0.5, 0.5]):
            result = subprocess.run([binary, '--probe-select', str(plain), '1', *map(str, rect)], env=env,
                                    text=True, capture_output=True, timeout=30)
            assert result.returncode != 0, rect
        print('PASS: malformed, empty and out-of-page selection rectangles rejected')

        many = folder / "many-matches.pdf"
        many.write_bytes(sample.make_pdf(1000))
        limited = search(many, "e")
        assert limited["matches"] == 10000 and limited["truncated"], limited
        print("PASS: occurrence limit is explicit and bounded at 10,000")

        invalid = folder / "invalid.pdf"
        invalid.write_bytes(b"not a PDF")
        for path in (invalid, folder / "missing.pdf"):
            result = subprocess.run([binary, "--probe", str(path)], env=env,
                                    text=True, capture_output=True, timeout=30)
            assert result.returncode != 0, path
        print("PASS: malformed and missing PDFs fail without a successful result")


if __name__ == "__main__":
    main()
