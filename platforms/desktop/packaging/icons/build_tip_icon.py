# /// script
# requires-python = ">=3.11"
# dependencies = ["cairosvg>=2.7", "pillow>=10"]
# ///
"""Builds crates/zt-tsf/logo.ico, the input method's icon in the Windows language list and input indicator.

Each frame is its own SVG (tip-16.svg … tip-256.svg), drawn on that size's pixel grid so the strokes stay sharp at
100–300 % scaling; nothing is scaled from another size. The design follows Microsoft's IME requirements for brand
icons: black and white only, a white box with the glyph in black and a one-pixel black outline at 50 % opacity, so
the one icon reads on dark, light and high-contrast taskbars (Windows reads it from the registered file and offers
no way to swap it with the theme).

Usage (needs the Cairo library, e.g. `brew install cairo` or libcairo2):
    uv run platforms/desktop/packaging/icons/build_tip_icon.py
"""

from __future__ import annotations

import io
import os
import struct
import sys
from pathlib import Path

if sys.platform == "darwin":
    # cairocffi finds Cairo through ctypes.util.find_library, which on macOS searches this list (read when called)
    # but not Homebrew's prefix; the imports below come after it.
    fallback = os.environ.get("DYLD_FALLBACK_LIBRARY_PATH", "")
    os.environ["DYLD_FALLBACK_LIBRARY_PATH"] = ":".join(
        p for p in (fallback, "/opt/homebrew/lib", "/usr/local/lib") if p
    )

import cairosvg
from PIL import Image

HERE = Path(__file__).resolve().parent
OUTPUT = HERE.parent.parent / "crates" / "zt-tsf" / "logo.ico"
SIZES = (16, 20, 24, 32, 40, 48, 64, 256)


def render(size: int) -> Image.Image:
    png = cairosvg.svg2png(url=str(HERE / f"tip-{size}.svg"), output_width=size, output_height=size)
    image = Image.open(io.BytesIO(png)).convert("RGBA")
    if image.size != (size, size):
        raise ValueError(f"tip-{size}.svg rendered at {image.size}")
    return image


def dib(image: Image.Image) -> bytes:
    """A 32-bit icon frame: BITMAPINFOHEADER (double height), bottom-up BGRA rows, then an all-zero AND mask."""
    width, height = image.size
    header = struct.pack("<IiiHHIIiiII", 40, width, height * 2, 1, 32, 0, 0, 0, 0, 0, 0)
    pixels = image.tobytes("raw", "BGRA", 0, -1)
    mask_stride = (width + 31) // 32 * 4
    return header + pixels + bytes(mask_stride * height)


def frame(image: Image.Image) -> bytes:
    # 256 is stored as PNG (Vista and later); the small frames as DIBs, which every icon loader reads.
    if image.width >= 256:
        out = io.BytesIO()
        image.save(out, "PNG", optimize=True)
        return out.getvalue()
    return dib(image)


def ico(images: list[Image.Image]) -> bytes:
    data = [frame(image) for image in images]
    offset = 6 + 16 * len(images)
    entries = b""
    for image, blob in zip(images, data, strict=True):
        side = image.width % 256  # 0 means 256
        entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(blob), offset)
        offset += len(blob)
    return struct.pack("<HHH", 0, 1, len(images)) + entries + b"".join(data)


def main() -> int:
    images = [render(size) for size in SIZES]
    OUTPUT.write_bytes(ico(images))
    print(f"{OUTPUT} ({', '.join(str(size) for size in SIZES)} px)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
