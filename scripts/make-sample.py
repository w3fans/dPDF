#!/usr/bin/env python3
"""Generate an original PDF fixture with text, page numbers, and bookmarks (stdlib only)."""
import argparse
import math
from pathlib import Path


def write_pdf(objects: list[bytes]) -> bytes:
    """Serialize original generated fixture objects with a complete fresh xref."""
    data = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = [0]
    for number, obj in enumerate(objects, 1):
        offsets.append(len(data))
        data.extend(f"{number} 0 obj\n".encode() + obj + b"\nendobj\n")
    xref = len(data)
    data.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode())
    for offset in offsets[1:]:
        data.extend(f"{offset:010d} 00000 n \n".encode())
    data.extend(f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode())
    return bytes(data)


def make_physical_unit_pdf(unit: float, count: int = 3) -> bytes:
    """Equivalent letter pages with a physical 1-by-0.5-inch color marker."""
    if unit not in (0.5, 1, 2) or not 1 <= count <= 3:
        raise ValueError('Choose a generated physical-unit fixture')
    objects = [b'<< /Type /Catalog /Pages 2 0 R >>', b'',
               b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>']
    pages = []
    for index in range(count):
        page, content_id = len(objects)+1, len(objects)+2
        pages.append(page)
        color = ('0.8 0.1 0.1', '0.1 0.8 0.1', '0.1 0.1 0.8')[index]
        content = (f'{color} rg {72/unit:g} {72/unit:g} {72/unit:g} {36/unit:g} re f '
                   f'0 g BT /F1 {14/unit:g} Tf {60/unit:g} {730/unit:g} Td (Physical units) Tj ET\n').encode()
        objects.append((f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {612/unit:g} {792/unit:g}] '
                        f'/UserUnit {unit:g} /Resources << /Font << /F1 3 0 R >> >> /Contents {content_id} 0 R >>').encode())
        objects.append(f'<< /Length {len(content)} >>\nstream\n'.encode()+content+b'endstream')
    objects[1] = f'<< /Type /Pages /Count {count} /Kids [{" ".join(f"{page} 0 R" for page in pages)}] >>'.encode()
    return write_pdf(objects)


def make_pdf(count: int, rotation: int = 0, cropped: bool = False, title: str = "dPDF sample contract", print_marker: bool = False, user_unit: float = 1) -> bytes:
    if not math.isfinite(user_unit) or user_unit <= 0:
        raise ValueError("Choose positive finite page units")
    title = title.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")
    objects = [b"<< /Type /Catalog /Pages 2 0 R /Outlines 4 0 R /PageMode /UseOutlines >>", b"", b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>", b""]
    page_ids = []
    for page in range(1, count + 1):
        page_id = len(objects) + 1
        page_ids.append(page_id)
        content_id = page_id + 1
        content = (f"BT /F1 24 Tf 60 730 Td ({title} - page {page}) Tj "
                   f"/F1 12 Tf 0 -42 Td (Review clause {page}: payment due within 30 days.) Tj "
                   "0 -24 Td (Search for payment; navigate by thumbnail or bookmark.) Tj ET "
                   "60 100 m 350 100 l S "
                   "BT /F1 11 Tf 60 120 Td (Authorized signature:) Tj ET\n").encode("ascii")
        if print_marker:
            colors = ("0.8 0.1 0.1", "0.1 0.8 0.1", "0.1 0.1 0.8")
            content += f"{colors[(page-1)%3]} rg 72 72 72 36 re f 0 g\n".encode()
        boundaries = f"/Rotate {rotation} " + ("/CropBox [40 60 572 752] " if cropped else "")
        if user_unit != 1:
            boundaries += f"/UserUnit {user_unit:g} "
        objects.append(f"<< /Type /Page {boundaries}/Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents {content_id} 0 R >>".encode())
        objects.append(f"<< /Length {len(content)} >>\nstream\n".encode() + content + b"endstream")
    outline_ids = list(range(len(objects) + 1, len(objects) + count + 1))
    for index, (page_id, outline_id) in enumerate(zip(page_ids, outline_ids)):
        prev = f"/Prev {outline_id - 1} 0 R " if index else ""
        next_ = f"/Next {outline_id + 1} 0 R " if index + 1 < count else ""
        objects.append(f"<< /Title (Page {index + 1}) /Parent 4 0 R {prev}{next_}/Dest [{page_id} 0 R /Fit] >>".encode())
    objects[1] = f"<< /Type /Pages /Count {count} /Kids [{' '.join(f'{i} 0 R' for i in page_ids)}] >>".encode()
    objects[3] = f"<< /Type /Outlines /First {outline_ids[0]} 0 R /Last {outline_ids[-1]} 0 R /Count {count} >>".encode()
    return write_pdf(objects)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--pages", type=int, default=50)
    parser.add_argument("--rotate", type=int, choices=[0, 90, 180, 270], default=0)
    parser.add_argument("--crop", action="store_true")
    args = parser.parse_args()
    if not 1 <= args.pages <= 10000:
        parser.error("--pages must be between 1 and 10000")
    args.output.write_bytes(make_pdf(args.pages, args.rotate, args.crop))
    print(f"Created {args.output}: {args.pages} pages")
