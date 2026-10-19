#!/usr/bin/env python3
# Copyright 2026 Janghoon Lee (Redrob) and contributors
# SPDX-License-Identifier: Apache-2.0
"""Generate every branding asset for Redrob OS from the Redrob Group Design System.

Nothing here is drawn by hand. The mark and lockups are the design system's own
exports, copied verbatim into tools/branding/brand/ and pinned by SHA256SUMS;
every colour comes from the redrob-ui tokens.json next to them. This script only
resizes DOWN (never up), crops, composes grounds and writes config files:

  * hicolor icon theme `redrob-os`      favicon tiles (white symbol on Redrob Blue)
  * /usr/share/pixmaps                  KInfoCenter / Plymouth fallbacks
  * Plymouth `redrob` theme             primary-white lockup on redrob-black, a
                                        brand-gradient loader bar on the 40 deg rake
  * Plasma wallpaper `Redrob`           the "deep" ground: redrob-black crossed by
                                        the brand seam at 40 deg (85-social.md)
  * KDE colour schemes                  Redrob (light) + RedrobDark, from tokens
  * Plasma look-and-feel previews
  * Anaconda pixmaps + CSS colours

Usage:
    ~/.local/venv-pil/bin/python tools/branding/make_assets.py [--out image/branding]

Requires Pillow. Idempotent: re-running rewrites the same files.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import pathlib
import shutil

from PIL import Image, ImageDraw

HERE = pathlib.Path(__file__).resolve().parent
BRAND = HERE / "brand"

# ---- design-system inputs ----------------------------------------------------
# tools/branding/brand/README.md says where each file comes from.
LOCKUP_LIGHT = "redrob-logo-primary.png"  # gradient symbol, black wordmark (light grounds)
LOCKUP_DARK = "redrob-logo-primary-white.png"  # gradient symbol, white wordmark (dark grounds)
LOCKUP_MONO_WHITE = "redrob-logo-primary-solid-mono-white.png"  # flat white: on any gradient
SYMBOL_WHITE = "redrob-symbol-solid-white.png"  # flat white symbol, 320px
TILES = {16: "favicon-16x16.png", 32: "favicon-32x32.png", 48: "favicon-48x48.png",
         64: "favicon-64x64.png", 192: "icon-192.png", 512: "icon-512.png"}
TILE_SVG = "favicon.svg"
TOKENS = "tokens.json"

SS = 3  # supersampling for the ground geometry
RAKE_DEG = 40.0  # the one angle: gradient, seam, icon diagonals (10-logo.md)


# ---- pins ---------------------------------------------------------------------
def verify_brand() -> None:
    sums = (BRAND / "SHA256SUMS").read_text().split("\n")
    for line in sums:
        if not line.strip():
            continue
        want, name = line.split()
        got = hashlib.sha256((BRAND / name).read_bytes()).hexdigest()
        if got != want:
            raise SystemExit(f"brand/{name}: sha256 mismatch -- brand files are verbatim design-system exports")


# ---- tokens -------------------------------------------------------------------
class Tokens:
    def __init__(self, path: pathlib.Path):
        data = json.loads(path.read_text())
        self.by_name = {t["name"]: t for t in data["tokens"]}

    def hex(self, name: str, theme: str = "light") -> str:
        return self.by_name[name][theme]

    def rgb(self, name: str, theme: str = "light") -> tuple[int, int, int]:
        return hex_rgb(self.hex(name, theme))

    def kde(self, name: str, theme: str = "light") -> str:
        return ",".join(str(c) for c in self.rgb(name, theme))


def hex_rgb(h: str) -> tuple[int, int, int]:
    h = h.lstrip("#")
    return int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)


def lerp(a, b, t):
    return tuple(int(round(a[i] + (b[i] - a[i]) * t)) for i in range(3))


# ---- image helpers --------------------------------------------------------------
def load(name: str) -> Image.Image:
    return Image.open(BRAND / name).convert("RGBA")


def fit_width(img: Image.Image, width: int) -> Image.Image:
    """Downscale to `width` keeping aspect. Refuses to upscale (Logos/README.md)."""
    if width > img.width:
        raise SystemExit(f"refusing to enlarge {img.width}px artwork to {width}px")
    h = round(img.height * width / img.width)
    return img.resize((width, h), Image.LANCZOS)


def fit_square(img: Image.Image, size: int) -> Image.Image:
    if size > img.width:
        raise SystemExit(f"refusing to enlarge {img.width}px artwork to {size}px")
    return img.resize((size, size), Image.LANCZOS)


def tile(size: int) -> Image.Image:
    """Favicons/README.md: a shipped size is used as is; any other size is the next
    larger file resized down. Nothing is upscaled."""
    if size in TILES:
        return load(TILES[size])
    larger = min(s for s in TILES if s > size)
    return fit_square(load(TILES[larger]), size)


def save(img: Image.Image, path: pathlib.Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path, optimize=True)


def write_text(text: str, path: pathlib.Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


# ---- the deep ground ------------------------------------------------------------
def deep_ground(w: int, h: int, tk: Tokens, *, seam_x: float = 0.72) -> Image.Image:
    """85-social.md `deep`: Redrob Black, crossed by blue. One diagonal at 40 deg
    crossing the bottom right - the only shape on the field. The far side of the
    seam is lit in blue-10; the seam itself is gradient-brand (blue-6 -> teal-3)
    running up the rake, with one hairline beside it.

    `seam_x` is where the seam leaves the bottom edge, as a fraction of width."""
    W, H = w * SS, h * SS
    field = tk.rgb("redrob-black")
    beyond = tk.rgb("blue-10")
    img = Image.new("RGB", (W, H), field)
    d = ImageDraw.Draw(img)

    t = math.tan(math.radians(RAKE_DEG))
    x0 = seam_x * W  # seam at the bottom edge
    # the seam rises to the right at 40 deg: x = x0 + (H - y) / t
    def seam_x_at(y: float) -> float:
        return x0 + (H - y) / t

    # far side (right of the seam)
    d.polygon([(seam_x_at(H), H), (W, H), (W, 0), (seam_x_at(0), 0)], fill=beyond)

    # the seam: a band of gradient-brand drawn as thin slices along its length
    band = max(2, round(0.006 * W))
    c0, c1 = tk.rgb("blue-6"), tk.rgb("accent-teal-3")
    for y in range(0, H):
        x = seam_x_at(y)
        if x < -band or x > W + band:
            continue
        d.line([(x - band / 2, y), (x + band / 2, y)], fill=lerp(c0, c1, 1 - y / H), width=1)
    # hairline, 24 px (at 1080p) toward the field, blue-4 at low alpha
    hair = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    hd = ImageDraw.Draw(hair)
    off = round(24 * SS * (h / 1080))
    hb = tk.rgb("blue-4") + (int(255 * 0.30),)
    for y in range(0, H):
        x = seam_x_at(y) - off
        if 0 <= x < W:
            hd.point((x, y), fill=hb)
    img = Image.alpha_composite(img.convert("RGBA"), hair)
    return img.resize((w, h), Image.LANCZOS)


# ---- KDE colour scheme ------------------------------------------------------------
def kde_scheme(tk: Tokens, theme: str, name: str) -> str:
    """A Breeze-structured scheme whose every colour is a token. Semantic roles:
    surface-* for backgrounds, ink-* for text, surface-brand for selection,
    status-* for the negative/neutral/positive triplet, focus-ring for focus."""
    T = lambda n: tk.kde(n, theme)  # noqa: E731
    view_alt = "surface-sunken" if theme == "light" else "surface-raised"
    button_bg = "surface-sunken" if theme == "light" else "border-subtle"
    hover = "blue-5" if theme == "light" else "blue-4"
    visited = "accent-violet-4" if theme == "light" else "accent-violet-3"
    sel_alt = "blue-7" if theme == "light" else "blue-5"
    sel_inactive = "blue-2"

    def block(section: str, bg: str, bg_alt: str, fg: str = "ink-primary") -> str:
        return f"""[Colors:{section}]
BackgroundAlternate={T(bg_alt)}
BackgroundNormal={T(bg)}
DecorationFocus={T("focus-ring")}
DecorationHover={T(hover)}
ForegroundActive={T("ink-brand")}
ForegroundInactive={T("ink-muted")}
ForegroundLink={T("ink-brand")}
ForegroundNegative={T("status-danger")}
ForegroundNeutral={T("status-warning")}
ForegroundNormal={T(fg)}
ForegroundPositive={T("status-success")}
ForegroundVisited={T(visited)}
"""

    # Complementary is what the lock screen, logout and login greeter draw with:
    # always the dark tokens, whatever the scheme (it sits on the wallpaper).
    D = lambda n: tk.kde(n, "dark")  # noqa: E731
    complementary = f"""[Colors:Complementary]
BackgroundAlternate={D("surface-raised")}
BackgroundNormal={D("surface-base")}
DecorationFocus={D("focus-ring")}
DecorationHover={D("blue-4")}
ForegroundActive={D("ink-brand")}
ForegroundInactive={D("ink-muted")}
ForegroundLink={D("ink-brand")}
ForegroundNegative={D("status-danger")}
ForegroundNeutral={D("status-warning")}
ForegroundNormal={D("ink-primary")}
ForegroundPositive={D("status-success")}
ForegroundVisited={D("accent-violet-3")}
"""
    selection = f"""[Colors:Selection]
BackgroundAlternate={T(sel_alt)}
BackgroundNormal={T("surface-brand")}
DecorationFocus={T("focus-ring")}
DecorationHover={T(hover)}
ForegroundActive={T("ink-on-brand")}
ForegroundInactive={T(sel_inactive)}
ForegroundLink={T("accent-teal-2")}
ForegroundNegative={T("accent-red-2")}
ForegroundNeutral={T("accent-orange-2")}
ForegroundNormal={T("ink-on-brand")}
ForegroundPositive={T("accent-green-2")}
ForegroundVisited={T("accent-violet-2")}
"""
    effects = """[ColorEffects:Disabled]
Color=56,56,56
ColorAmount=0
ColorEffect=0
ContrastAmount=0.65
ContrastEffect=1
IntensityAmount=0.1
IntensityEffect=2

[ColorEffects:Inactive]
ChangeSelectionColor=true
Color=112,111,110
ColorAmount=0.025
ColorEffect=2
ContrastAmount=0.1
ContrastEffect=2
Enable=false
IntensityAmount=0
IntensityEffect=0
"""
    header = f"""# Redrob OS colour scheme ({theme}). Generated by tools/branding/make_assets.py
# from the Redrob Group Design System tokens (redrob-ui tokens.json): do not edit.
# Copyright 2026 Janghoon Lee (Redrob) and contributors
# SPDX-License-Identifier: Apache-2.0
# Structure follows KDE's Breeze schemes (LGPL-2.0-or-later); every value is a token.

{effects}
{block("Button", button_bg, "border-subtle")}
{complementary}
{block("Header", "surface-raised", "surface-sunken")}
{selection}
{block("Tooltip", "surface-raised", "surface-sunken")}
{block("View", "surface-base", view_alt)}
{block("Window", "surface-raised", "surface-sunken")}
[General]
ColorScheme={name}
Name={name}
TitleBarColor={T("surface-raised")}
accentColorFromWallpaper=false
shadeSortColumn=true

[KDE]
contrast=4

[WM]
activeBackground={T("surface-raised")}
activeBlend={T("ink-primary")}
activeForeground={T("ink-primary")}
inactiveBackground={T("surface-raised")}
inactiveBlend={T("ink-muted")}
inactiveForeground={T("ink-muted")}
"""
    return header


# ---- Anaconda CSS -----------------------------------------------------------------
def anaconda_css(tk: Tokens) -> str:
    return f"""/* Anaconda GTK style overrides for Redrob OS.
 * Copyright 2026 Janghoon Lee (Redrob) and contributors
 * SPDX-License-Identifier: Apache-2.0
 *
 * Generated by tools/branding/make_assets.py from the design-system tokens.
 * Loaded via [User Interface] custom_stylesheet= in the redrob-os Anaconda
 * profile (image/installer). Pixmaps match the fedora-logos sizes so the stock
 * sidebar/topbar layout holds; the sidebar carries the primary-white lockup on
 * the deep ground, and every control that shows a vendor colour is Redrob Blue.
 */

@define-color redrob_blue {tk.hex("blue-6")};          /* surface-brand */
@define-color redrob_blue_deep {tk.hex("blue-7")};     /* pressed / hover on light */
@define-color redrob_black {tk.hex("redrob-black")};   /* dark ground */
@define-color redrob_ink {tk.hex("ink-primary", "dark")};

.logo-sidebar {{
    background-image: url('/usr/share/anaconda/pixmaps/sidebar-bg.png');
    background-color: @redrob_black;
    background-repeat: no-repeat;
}}

.logo {{
    background-image: url('/usr/share/anaconda/pixmaps/sidebar-logo.png');
    background-position: 50% 20px;
    background-repeat: no-repeat;
    background-color: transparent;
}}

.product-logo {{
    background-image: none;
    background-color: transparent;
}}

AnacondaSpokeWindow #nav-box {{
    background-color: @redrob_black;
    background-image: url('/usr/share/anaconda/pixmaps/topbar-bg.png');
    background-repeat: repeat;
    color: @redrob_ink;
}}

/* Blue acts (20-color.md): progress, switches, checks, radios, links, focus. */
progressbar progress,
switch:checked,
checkbutton check:checked,
radiobutton radio:checked {{
    background-color: @redrob_blue;
    border-color: @redrob_blue;
}}

button.suggested-action {{
    background-image: none;
    background-color: @redrob_blue;
    border-color: @redrob_blue;
    color: white;
}}

button.suggested-action:hover,
button.suggested-action:active {{
    background-color: @redrob_blue_deep;
    border-color: @redrob_blue_deep;
}}

*:link,
label.link {{
    color: @redrob_blue_deep;
}}

*:focus {{
    outline-color: @redrob_blue;
}}
"""


# ---- outputs ----------------------------------------------------------------------
def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="image/branding", help="overlay root (default image/branding)")
    args = ap.parse_args()
    out = pathlib.Path(args.out)
    share = out / "usr/share"

    verify_brand()
    tk = Tokens(BRAND / TOKENS)
    black = tk.rgb("redrob-black")

    # 1. hicolor icon theme `redrob-os` (os-release LOGO=, kickoff button, About).
    #    Favicons/README.md: white symbol on a Redrob Blue tile, drawn per size.
    for px in (16, 32, 48, 64, 128, 256):
        save(tile(px), share / f"icons/hicolor/{px}x{px}/apps/redrob-os.png")
    shutil.copyfile(BRAND / TILE_SVG, share / "icons/hicolor/scalable/apps/redrob-os.svg")

    # 2. pixmaps: 252x252 like fedora-logos. system-logo-white is the symbol alone
    #    (dark grounds that are not Redrob Blue: KInfoCenter header, Plymouth
    #    fallback); redrob-os.png is the tile.
    save(fit_square(load(SYMBOL_WHITE), 252), share / "pixmaps/system-logo-white.png")
    save(tile(252), share / "pixmaps/redrob-os.png")

    # 3. Plymouth: primary-white lockup (never the symbol alone when the lockup fits),
    #    420 px wide, and a loader on the rake: 24 frames of a brand-gradient segment
    #    travelling a border-subtle track. Dialogue bits on redrob-black.
    ply = share / "plymouth/themes/redrob"
    save(fit_width(load(LOCKUP_DARK), 420), ply / "logo.png")
    frames, bw, bh = 24, 300, 4
    track = tk.rgb("gray-8") + (255,)
    for i in range(frames):
        f = Image.new("RGBA", (bw * SS, bh * SS), (0, 0, 0, 0))
        d = ImageDraw.Draw(f)
        d.rectangle([0, 0, bw * SS, bh * SS], fill=track)
        seg = int(bw * SS * 0.28)
        x = int((bw * SS + seg) * i / frames) - seg
        for k in range(seg):
            xx = x + k
            if 0 <= xx < bw * SS:
                d.line([(xx, 0), (xx, bh * SS)], fill=lerp(tk.rgb("blue-6"), tk.rgb("accent-teal-3"), k / seg) + (255,))
        save(f.resize((bw, bh), Image.LANCZOS), ply / f"throbber-{i:04d}.png")
    box = Image.new("RGBA", (420, 80), tk.rgb("surface-raised", "dark") + (235,))
    save(box, ply / "box.png")
    entry = Image.new("RGBA", (300, 36), black + (255,))
    ImageDraw.Draw(entry).rectangle([0, 0, 299, 35], outline=tk.rgb("blue-6") + (255,), width=2)
    save(entry, ply / "entry.png")
    bullet = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
    ImageDraw.Draw(bullet).ellipse([4, 4, 11, 11], fill=tk.rgb("ink-primary", "dark") + (255,))
    save(bullet, ply / "bullet.png")
    lock = Image.new("RGBA", (36, 36), (0, 0, 0, 0))
    ld = ImageDraw.Draw(lock)
    ink = tk.rgb("ink-primary", "dark") + (255,)
    ld.rounded_rectangle([8, 16, 28, 32], radius=3, fill=ink)
    ld.arc([11, 4, 25, 22], start=180, end=360, fill=ink, width=3)
    save(lock, ply / "lock.png")

    # 4. Wallpaper package `Redrob`: the deep ground at three sizes.
    wp = share / "wallpapers/Redrob"
    full = deep_ground(3840, 2160, tk)
    for w, h in ((1920, 1080), (2560, 1440), (3840, 2160)):
        img = full if (w, h) == (3840, 2160) else full.resize((w, h), Image.LANCZOS)
        save(img.convert("RGB"), wp / f"contents/images/{w}x{h}.png")
    save(full.resize((400, 225), Image.LANCZOS).convert("RGB"), wp / "contents/screenshot.png")
    write_text(json.dumps({
        "KPlugin": {
            "Id": "Redrob",
            "Name": "Redrob",
            "Description": "Redrob OS default wallpaper: the deep ground, Redrob Black crossed by the brand seam",
            "Authors": [{"Name": "Redrob", "Email": "hello@redrob.ai"}],
            "License": "Apache-2.0",
            "Website": "https://redrob.ai",
        },
        "KPackageStructure": "Plasma/Wallpaper",
        "X-KDE-PlasmaImageWallpaper-AccentColor": tk.hex("blue-6").upper(),
    }, indent=4) + "\n", wp / "metadata.json")

    # 5. Plasma look-and-feel previews
    lnf = share / "plasma/look-and-feel/ai.redrob.desktop/contents/previews"
    save(full.resize((640, 360), Image.LANCZOS).convert("RGB"), lnf / "preview.png")
    full.resize((1280, 720), Image.LANCZOS).convert("RGB").save(lnf / "fullscreenpreview.jpg", quality=88)

    # 6. KDE colour schemes from tokens: Redrob (light, default) and RedrobDark.
    write_text(kde_scheme(tk, "light", "Redrob"), share / "color-schemes/Redrob.colors")
    write_text(kde_scheme(tk, "dark", "RedrobDark"), share / "color-schemes/RedrobDark.colors")
    # the look-and-feel package's own copy of the default scheme
    write_text(kde_scheme(tk, "light", "Redrob"), share / "plasma/look-and-feel/ai.redrob.desktop/contents/colors")

    # 7. Anaconda pixmaps (sizes match fedora-logos) + CSS
    ana = share / "anaconda/pixmaps"
    lockup = fit_width(load(LOCKUP_DARK), 130)  # 130x26 inside the 150x69 slot: clear space holds
    sidebar_logo = Image.new("RGBA", (150, 69), (0, 0, 0, 0))
    sidebar_logo.paste(lockup, ((150 - lockup.width) // 2, (69 - lockup.height) // 2), lockup)
    save(sidebar_logo, ana / "sidebar-logo.png")
    # sidebar: the deep ground, seam low so the spoke list stays on plain black
    save(deep_ground(406, 767, tk, seam_x=0.55).convert("RGBA"), ana / "sidebar-bg.png")
    save(Image.new("RGBA", (1040, 132), black + (255,)), ana / "topbar-bg.png")
    write_text(anaconda_css(tk), ana / "redrob-os.css")

    print("done")


if __name__ == "__main__":
    main()
