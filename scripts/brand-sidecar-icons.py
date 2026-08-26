#!/usr/bin/env python3
"""Rasterize Eclipse pixel SVGs to PNG/ICO for sidecar EXE and tray-less host icons."""
from __future__ import annotations

import argparse
import re
import struct
import zlib
from pathlib import Path


def parse_color(hex_color: str) -> tuple[int, int, int]:
    h = hex_color.strip().lstrip("#")
    return int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)


def even_odd(point: tuple[float, float], ring: list[tuple[float, float]]) -> bool:
    x, y = point
    inside = False
    n = len(ring)
    if n < 2:
        return False
    j = n - 1
    for i in range(n):
        xi, yi = ring[i]
        xj, yj = ring[j]
        if (yi > y) != (yj > y):
            denom = yj - yi
            if denom != 0:
                xinters = (xj - xi) * (y - yi) / denom + xi
                if x < xinters:
                    inside = not inside
        j = i
    return inside


def parse_points(attr: str) -> list[tuple[float, float]]:
    nums = [float(n) for n in re.findall(r"-?\d+(?:\.\d+)?", attr)]
    return list(zip(nums[0::2], nums[1::2], strict=False))


def parse_path(d: str) -> list[list[tuple[float, float]]]:
    tokens = re.findall(r"[MmLlHhVvZz]|-?\d+(?:\.\d+)?", d)
    rings: list[list[tuple[float, float]]] = []
    ring: list[tuple[float, float]] = []
    cmd = "M"
    i = 0
    cx = cy = 0.0
    sx = sy = 0.0

    def take() -> float:
        nonlocal i
        v = float(tokens[i])
        i += 1
        return v

    while i < len(tokens):
        t = tokens[i]
        if re.fullmatch(r"[MmLlHhVvZz]", t):
            cmd = t
            i += 1
            if cmd in "Zz":
                if ring:
                    ring.append((sx, sy))
                    rings.append(ring)
                    ring = []
                cx, cy = sx, sy
            continue
        if cmd in "Mm":
            x, y = take(), take()
            if cmd == "m":
                x += cx
                y += cy
            if ring:
                rings.append(ring)
            cx, cy = x, y
            sx, sy = x, y
            ring = [(cx, cy)]
            cmd = "l" if cmd == "m" else "L"
        elif cmd in "Ll":
            x, y = take(), take()
            if cmd == "l":
                x += cx
                y += cy
            cx, cy = x, y
            ring.append((cx, cy))
        elif cmd in "Hh":
            x = take()
            if cmd == "h":
                x += cx
            cx = x
            ring.append((cx, cy))
        elif cmd in "Vv":
            y = take()
            if cmd == "v":
                y += cy
            cy = y
            ring.append((cx, cy))
        else:
            i += 1
    if ring:
        rings.append(ring)
    return rings


def svg_rings(svg: str) -> list[list[tuple[float, float]]]:
    rings: list[list[tuple[float, float]]] = []
    for m in re.finditer(r"<polygon[^>]*points=\"([^\"]+)\"", svg):
        pts = parse_points(m.group(1))
        if pts:
            rings.append(pts + [pts[0]])
    for m in re.finditer(r"<path[^>]*d=\"([^\"]+)\"", svg):
        rings.extend(parse_path(m.group(1)))
    return rings


def rasterize(svg: str, canvas: int, pad: int, color: tuple[int, int, int]) -> bytes:
    inner = canvas - 2 * pad
    rings = svg_rings(svg)
    pixels = bytearray(canvas * canvas * 4)
    for y in range(canvas):
        for x in range(canvas):
            sx = (x + 0.5 - pad) * 24.0 / inner
            sy = (y + 0.5 - pad) * 24.0 / inner
            hits = 0
            for ring in rings:
                if even_odd((sx, sy), ring):
                    hits += 1
            if hits % 2 == 1:
                i = (y * canvas + x) * 4
                pixels[i : i + 4] = bytes([color[0], color[1], color[2], 255])
    return bytes(pixels)


def png_bytes(w: int, h: int, rgba: bytes) -> bytes:
    def chunk(tag: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(tag + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

    raw = b""
    stride = w * 4
    for y in range(h):
        raw += b"\x00" + rgba[y * stride : (y + 1) * stride]
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def write_ico(path: Path, images: list[tuple[int, bytes]]) -> None:
    count = len(images)
    offset = 6 + 16 * count
    header = struct.pack("<HHH", 0, 1, count)
    entries = b""
    payload = b""
    for size, png in images:
        w = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", w, w, 0, 0, 1, 32, len(png), offset)
        payload += png
        offset += len(png)
    path.write_bytes(header + entries + payload)


def scale_nn(rgba: bytes, src: int, dst: int) -> bytes:
    out = bytearray(dst * dst * 4)
    for y in range(dst):
        sy = y * src // dst
        for x in range(dst):
            sx = x * src // dst
            si = (sy * src + sx) * 4
            di = (y * dst + x) * 4
            out[di : di + 4] = rgba[si : si + 4]
    return bytes(out)


def export_icon(svg_path: Path, color: str, png_path: Path, ico_path: Path) -> None:
    svg = svg_path.read_text(encoding="utf-8")
    rgb = parse_color(color)
    px32 = rasterize(svg, 32, 4, rgb)
    png_path.write_bytes(png_bytes(32, 32, px32))
    sizes = []
    for size in (16, 32, 48, 64):
        px = scale_nn(px32, 32, size)
        sizes.append((size, png_bytes(size, size, px)))
    write_ico(ico_path, sizes)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    args = parser.parse_args()
    root: Path = args.root
    icons = root / "src-tauri" / "icons"
    moon_svg = root / "src" / "assets" / "icons" / "moon-solid.svg"
    sun_svg = root / "src" / "assets" / "icons" / "sun-solid.svg"
    export_icon(moon_svg, "#d9d9fc", icons / "moonlight.png", icons / "moonlight.ico")
    export_icon(sun_svg, "#f7c708", icons / "sunshine.png", icons / "sunshine.ico")
    print(f"Wrote {icons / 'moonlight.ico'} and {icons / 'sunshine.ico'}")


if __name__ == "__main__":
    main()
