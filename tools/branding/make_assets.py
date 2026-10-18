#!/usr/bin/env python3
# Copyright 2026 Janghoon Lee (Redrob) and contributors
# SPDX-License-Identifier: Apache-2.0
"""Generate every raster branding asset for Redrob OS from one mark description.

The mark is ours and deliberately simple: a Redrob-red (#E5322D) disc with a
white inner ring. No upstream (Fedora / KDE / Universal Blue) geometry is used.

Usage:
    python3 tools/branding/make_assets.py [--out image/branding]

Requires Pillow only. Idempotent: re-running rewrites the same files.
"""
from __future__ import annotations

import argparse
import json
import pathlib

from PIL import Image, ImageDraw

# ---- colour tokens (keep in sync with docs/branding.md) ----------------------
RED = (0xE5, 0x32, 0x2D)  # Redrob red
BG = (0x12, 0x12, 0x12)  # dark background
BG_WARM = (0x2A, 0x0D, 0x0C)  # warm dark end of the wallpaper gradient
WHITE = (0xFC, 0xFC, 0xFC)

SS = 4  # supersampling factor for anti-aliasing

# ---- the mark ---------------------------------------------------------------
# Geometry in unit coordinates (0..1 of the mark's bounding box):
#   outer disc  radius 0.50
#   ring (white) outer 0.33, inner 0.21
# Rendered as: red disc, white ring, red centre.
MARK = {"disc": 0.50, "ring_outer": 0.33, "ring_inner": 0.21}


def draw_mark(size: int, *, disc=RED, ring=WHITE, centre=None) -> Image.Image:
    """Return an RGBA image of the mark, `size` px square, transparent outside."""
    centre = centre or disc
    s = size * SS
    img = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    c = s / 2

    def circle(r, fill):
        rr = r * s
        d.ellipse([c - rr, c - rr, c + rr, c + rr], fill=fill + (255,))

    circle(MARK["disc"], disc)
    circle(MARK["ring_outer"], ring)
    circle(MARK["ring_inner"], centre)
    return img.resize((size, size), Image.LANCZOS)


def mark_svg() -> str:
    """Hand-written SVG of the same mark (scalable icon)."""
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<!-- Copyright 2026 Janghoon Lee (Redrob) and contributors. SPDX-License-Identifier: Apache-2.0 -->
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">
  <circle cx="50" cy="50" r="{MARK['disc'] * 100:g}" fill="#E5322D"/>
  <circle cx="50" cy="50" r="{MARK['ring_outer'] * 100:g}" fill="#FCFCFC"/>
  <circle cx="50" cy="50" r="{MARK['ring_inner'] * 100:g}" fill="#E5322D"/>
</svg>
"""


# ---- helpers ----------------------------------------------------------------
def save(img: Image.Image, path: pathlib.Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path, optimize=True)
    print(f"wrote {path} ({img.size[0]}x{img.size[1]})")


def write_text(text: str, path: pathlib.Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    print(f"wrote {path}")


def vertical_gradient(w: int, h: int, top, bottom) -> Image.Image:
    """Diagonal-ish gradient: top-left `top` -> bottom-right `bottom`."""
    # Build a small gradient and scale up: cheap and smooth.
    g = Image.new("RGB", (64, 64))
    px = g.load()
    for y in range(64):
        for x in range(64):
            t = (x + y) / 126.0
            px[x, y] = tuple(round(a + (b - a) * t) for a, b in zip(top, bottom))
    return g.resize((w, h), Image.BICUBIC)


def wallpaper(w: int, h: int) -> Image.Image:
    img = vertical_gradient(w, h, BG, BG_WARM)
    # subtle mark bottom-right: 12% of height, low opacity
    m = int(h * 0.12)
    mark = draw_mark(m)
    alpha = mark.getchannel("A").point(lambda a: int(a * 0.28))
    mark.putalpha(alpha)
    margin = int(h * 0.05)
    img.paste(mark, (w - m - margin, h - m - margin), mark)
    return img


# ---- outputs ----------------------------------------------------------------
def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="image/branding", help="overlay root (default image/branding)")
    args = ap.parse_args()
    out = pathlib.Path(args.out)
    share = out / "usr/share"

    # 1. hicolor icon theme: redrob-os (also referenced by os-release LOGO=)
    for px in (16, 32, 48, 64, 128, 256):
        save(draw_mark(px), share / f"icons/hicolor/{px}x{px}/apps/redrob-os.png")
    write_text(mark_svg(), share / "icons/hicolor/scalable/apps/redrob-os.svg")

    # 2. pixmaps: system-logo-white.png is what Plymouth (spinner watermark),
    #    Anaconda and KInfoCenter fall back to. 252x252 matches fedora-logos.
    save(draw_mark(252), share / "pixmaps/system-logo-white.png")
    save(draw_mark(252), share / "pixmaps/redrob-os.png")

    # 3. Plymouth theme images (script theme reads from its ImageDir)
    ply = share / "plymouth/themes/redrob"
    save(draw_mark(128), ply / "logo.png")
    # spinner frames: 24 frames of a thin red arc rotating around the mark
    frames = 24
    for i in range(frames):
        s = 64 * SS
        f = Image.new("RGBA", (s, s), (0, 0, 0, 0))
        d = ImageDraw.Draw(f)
        pad = 4 * SS
        d.arc([pad, pad, s - pad, s - pad], start=i * 360 / frames, end=i * 360 / frames + 90,
              fill=RED + (255,), width=4 * SS)
        save(f.resize((64, 64), Image.LANCZOS), ply / f"throbber-{i:04d}.png")
    # password dialog bits
    box = Image.new("RGBA", (420, 80), (0x20, 0x20, 0x20, 230))
    save(box, ply / "box.png")
    entry = Image.new("RGBA", (300, 36), (0x12, 0x12, 0x12, 255))
    ImageDraw.Draw(entry).rectangle([0, 0, 299, 35], outline=RED + (255,), width=2)
    save(entry, ply / "entry.png")
    bullet = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
    ImageDraw.Draw(bullet).ellipse([4, 4, 11, 11], fill=WHITE + (255,))
    save(bullet, ply / "bullet.png")
    lock = Image.new("RGBA", (36, 36), (0, 0, 0, 0))
    ld = ImageDraw.Draw(lock)
    ld.rounded_rectangle([8, 16, 28, 32], radius=3, fill=WHITE + (255,))
    ld.arc([11, 4, 25, 22], start=180, end=360, fill=WHITE + (255,), width=3)
    save(lock, ply / "lock.png")

    # 4. KDE wallpaper package (Plasma/Wallpaper): contents/images/<WxH>.png
    wp = share / "wallpapers/Redrob"
    for w, h in ((1920, 1080), (2560, 1440), (3840, 2160)):
        save(wallpaper(w, h), wp / f"contents/images/{w}x{h}.png")
    # screenshot used by the wallpaper chooser
    save(wallpaper(1920, 1080).resize((400, 225), Image.LANCZOS), wp / "contents/screenshot.png")
    write_text(json.dumps({
        "KPlugin": {
            "Id": "Redrob",
            "Name": "Redrob",
            "Description": "Redrob OS default wallpaper",
            "Authors": [{"Name": "Redrob", "Email": "hello@redrob.ai"}],
            "License": "Apache-2.0",
            "Website": "https://redrob.ai",
        },
        "KPackageStructure": "Plasma/Wallpaper",
        "X-KDE-PlasmaImageWallpaper-AccentColor": "#E5322D",
    }, indent=4) + "\n", wp / "metadata.json")

    # 5. Plasma look-and-feel previews
    lnf = share / "plasma/look-and-feel/ai.redrob.desktop/contents/previews"
    save(wallpaper(1920, 1080).resize((640, 360), Image.LANCZOS), lnf / "preview.png")
    save(wallpaper(1920, 1080).resize((1280, 720), Image.LANCZOS), lnf / "fullscreenpreview.jpg")

    # 6. Anaconda pixmaps (sizes match fedora-logos so the stock CSS layout holds)
    ana = share / "anaconda/pixmaps"
    sidebar_logo = Image.new("RGBA", (150, 69), (0, 0, 0, 0))
    sidebar_logo.paste(draw_mark(60), (45, 4), draw_mark(60))
    save(sidebar_logo, ana / "sidebar-logo.png")
    save(vertical_gradient(406, 767, BG, BG_WARM).convert("RGBA"), ana / "sidebar-bg.png")
    save(Image.new("RGBA", (1040, 132), BG + (255,)), ana / "topbar-bg.png")

    # 7. SDDM: breeze theme background is the 1080p wallpaper (theme.conf.user points at it)
    print("done")


if __name__ == "__main__":
    main()
