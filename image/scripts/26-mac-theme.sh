#!/usr/bin/bash
# 26-mac-theme.sh -- bake the WhiteSur macOS-style theme (vinceliuice, GPL-3.0) so
# the first-login "macOS" style is a REAL Mac look: traffic-light window buttons
# (Aurorae), translucent Plasma theme, Big Sur icons. Applied ONLY when the user
# picks macOS in the style chooser, which applies it STOCK; the only Redrob change
# is the logo (start/OS logos -> Redrob OS mark, Redrob splash).
#
# We run WhiteSur's OWN installer: its aurorae themes need a per-variant
# metadata.desktop + shared button svgs that are NOT inside the raw
# aurorae/main/<variant>/ dir, so hand-copying them makes kwin fall back to Breeze
# (no traffic-light buttons). The installer assembles them correctly.
# Upstream: github.com/vinceliuice/WhiteSur-kde , /WhiteSur-icon-theme (pinned).
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0 (our glue only).
set -euo pipefail

# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"
load_pins

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

# WhiteSur KDE theme, every variant, exactly as upstream ships it (global theme,
# Aurorae + metadata, colours, Kvantum, Plasma theme, wallpaper) -- its own
# installer, so aurorae loads. The style chooser applies the light one stock.
fetch_extract WHITESUR_KDE "${work}/kde"
( cd "${work}/kde" && bash ./install.sh ) \
  || { echo "error: WhiteSur-kde install.sh failed" >&2; exit 1; }
rm -rf /usr/share/sddm/themes/WhiteSur* 2>/dev/null || true   # greeter stays Redrob

# WhiteSur icon theme (Big Sur icons).
fetch_extract WHITESUR_ICON "${work}/icon"
install_icon_theme "${work}/icon" WhiteSur WhiteSur-dark

# Rebrand only: OS/start logos -> Redrob OS mark.
rebrand_logos /usr/share/plasma/desktoptheme/WhiteSur* /usr/share/icons/WhiteSur*

echo "26-mac-theme: WhiteSur (stock) + icons installed, logos rebranded"
