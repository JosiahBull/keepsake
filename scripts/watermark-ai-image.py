# /// script
# requires-python = ">=3.10"
# dependencies = ["pillow>=10.1"]
# ///
"""Stamp a visible "AI-generated" label on an image before it joins a default list.

Usage: uv run scripts/watermark-ai-image.py IN OUT

The label sits in the lower-left corner, on a dark band sized to the image, so it stays
legible on the phone-sized thumbnails matching shows. OUT is written as WebP, still,
without metadata. Also set "ai_generated": true on the file in its list.json: the label
in the app comes from that flag, not from this stamp.
"""

import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

LABEL = "AI-generated"
# Common bold sans-serif fonts; Pillow's own font is the fallback.
FONTS = [
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
]


def font(size: int) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    for path in FONTS:
        if Path(path).exists():
            return ImageFont.truetype(path, size)
    return ImageFont.load_default(size=size)


def stamp(image: Image.Image) -> Image.Image:
    image = image.convert("RGB")
    # Matching shows images about 6rem tall, so size the label to the shorter side.
    size = max(14, round(min(image.size) * 0.06))
    face = font(size)
    draw = ImageDraw.Draw(image, "RGBA")
    left, top, right, bottom = draw.textbbox((0, 0), LABEL, font=face)
    pad = round(size * 0.45)
    margin = round(size * 0.5)
    box = (
        margin,
        image.height - margin - (bottom - top) - 2 * pad,
        margin + (right - left) + 2 * pad,
        image.height - margin,
    )
    draw.rounded_rectangle(box, radius=pad, fill=(0, 0, 0, 190))
    draw.text((box[0] + pad - left, box[1] + pad - top), LABEL, font=face, fill=(255, 255, 255, 255))
    return image


def main() -> None:
    if len(sys.argv) != 3:
        sys.exit("Usage: uv run scripts/watermark-ai-image.py IN OUT")
    source, target = map(Path, sys.argv[1:])
    with Image.open(source) as image:
        stamp(image).save(target, "WEBP", quality=82)


if __name__ == "__main__":
    main()
