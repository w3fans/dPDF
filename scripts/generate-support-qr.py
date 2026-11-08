#!/usr/bin/env python3
"""Regenerate the local About QR SVG with qrcode==8.2 (developer tool only)."""
import html
import importlib.metadata
from pathlib import Path
import re
import qrcode

ROOT = Path(__file__).resolve().parent.parent
if importlib.metadata.version('qrcode') != '8.2':
    raise SystemExit('Use qrcode==8.2 for deterministic asset regeneration')
source = (ROOT / 'src/about.rs').read_text(encoding='utf-8')
url = re.search(r'pub const SUPPORT_URL: &str = "([^"]+)";', source)[1]
code = qrcode.QRCode(error_correction=qrcode.constants.ERROR_CORRECT_M, border=4)
code.add_data(url, optimize=0)
code.make(fit=True)
matrix = code.get_matrix()
side = len(matrix)
paths = ''.join(f'M{x} {y}h1v1h-1z' for y,row in enumerate(matrix) for x,on in enumerate(row) if on)
svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{side*8}" height="{side*8}" viewBox="0 0 {side} {side}">'
       f'<title>{html.escape(url)}</title><rect width="{side}" height="{side}" fill="white"/>'
       f'<path fill="black" d="{paths}"/></svg>\n')
target = ROOT / 'ui/assets/support-qr.svg'
target.parent.mkdir(parents=True, exist_ok=True)
target.write_text(svg, encoding='utf-8')
print(f'Generated {side}×{side} modules with a four-module quiet zone: {url}')
