#!/usr/bin/env python3
"""Render the existing code-native SVG into deterministic PNG-compressed Windows ICO frames."""
import argparse
from pathlib import Path
import struct
import subprocess


def main():
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--magick', default='magick', help='Existing developer ImageMagick tool; never downloaded by the app')
    parser.add_argument('--output', type=Path, default=root / 'ui/assets/dpdf.ico')
    args = parser.parse_args()
    frames = []
    for size in (256, 128, 64, 48, 32, 16):
        rendered = subprocess.run([args.magick, '-background', 'none', '-density', '192',
                                   str(root / 'ui/assets/dpdf.svg'), '-resize', f'{size}x{size}',
                                   '-strip', 'PNG32:-'], check=True, capture_output=True, timeout=30).stdout
        if not rendered.startswith(b'\x89PNG\r\n\x1a\n') or len(rendered) > 1024**2:
            raise RuntimeError('Unexpected icon frame from SVG renderer')
        frames.append((size, rendered))
    offset = 6 + 16 * len(frames)
    directory = []
    for size, data in frames:
        dimension = 0 if size == 256 else size
        directory.append(struct.pack('<BBBBHHII', dimension, dimension, 0, 0, 1, 32, len(data), offset))
        offset += len(data)
    args.output.write_bytes(struct.pack('<HHH', 0, 1, len(frames)) + b''.join(directory) +
                            b''.join(data for _, data in frames))
    print('Rendered dPDF Windows icon from the existing SVG: ' + str(args.output))


if __name__ == '__main__':
    main()
