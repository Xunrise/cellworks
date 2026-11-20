#!/usr/bin/env python3
"""Draws the Cellworks icon (site/favicon.svg's mark: a black tile with two
cut corners and three yellow bars) into assets/icon/: icon.png (512 px, the
window icon) and Cellworks.icns (the app bundle's icon, via iconutil).

Standard library only; anti-aliased by 4x4 supersampling.

    python3 scripts/make-icon.py
"""
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "assets", "icon")

BLACK = (0x07, 0x09, 0x0C)
YELLOW = (0xFC, 0xEE, 0x0A)
# The favicon's 32-unit design: tile with the top-right and bottom-left
# corners cut by 6 units, three bars.
TILE = [(0, 0), (26, 0), (32, 6), (32, 32), (6, 32), (0, 26)]
BARS = [(7, 14, 12, 24), (14, 7, 19, 24), (21, 11, 26, 24)]
# macOS icons leave a margin around the body (824 of 1024 on the Big Sur grid).
MARGIN = 100 / 1024


def inside(poly, x, y):
    hit = False
    j = len(poly) - 1
    for i, (xi, yi) in enumerate(poly):
        xj, yj = poly[j]
        if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / (yj - yi) + xi:
            hit = not hit
        j = i
    return hit


def colour_at(u, v):
    """The colour at design coordinates (u, v), or None outside the tile."""
    for x0, y0, x1, y1 in BARS:
        if x0 <= u < x1 and y0 <= v < y1:
            return YELLOW
    return BLACK if inside(TILE, u, v) else None


def render(size, margin=MARGIN, samples=4):
    body = size * (1 - 2 * margin)
    offset = size * margin
    rows = []
    for py in range(size):
        row = bytearray([0])  # PNG filter: none
        for px in range(size):
            r = g = b = a = 0
            for sy in range(samples):
                for sx in range(samples):
                    u = ((px + (sx + 0.5) / samples) - offset) / body * 32
                    v = ((py + (sy + 0.5) / samples) - offset) / body * 32
                    c = colour_at(u, v) if 0 <= u < 32 and 0 <= v < 32 else None
                    if c:
                        r, g, b, a = r + c[0], g + c[1], b + c[2], a + 1
            n = samples * samples
            row += bytes((r // a, g // a, b // a, 255 * a // n) if a else (0, 0, 0, 0))
        rows.append(bytes(row))
    return png(size, size, b"".join(rows))


def png(width, height, raw):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def main():
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, "icon.png"), "wb") as f:
        f.write(render(512, margin=0.0))
    if not shutil.which("iconutil"):
        sys.exit("iconutil not found (macOS only); icon.png written, Cellworks.icns not")
    cache = {}
    with tempfile.TemporaryDirectory() as tmp:
        iconset = os.path.join(tmp, "Cellworks.iconset")
        os.mkdir(iconset)
        for base in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                px = base * scale
                if px not in cache:
                    cache[px] = render(px)
                name = f"icon_{base}x{base}{'@2x' if scale == 2 else ''}.png"
                with open(os.path.join(iconset, name), "wb") as f:
                    f.write(cache[px])
        subprocess.run(
            ["iconutil", "-c", "icns", iconset, "-o", os.path.join(OUT, "Cellworks.icns")],
            check=True,
        )
    print(os.path.join(OUT, "icon.png"))
    print(os.path.join(OUT, "Cellworks.icns"))


if __name__ == "__main__":
    main()
