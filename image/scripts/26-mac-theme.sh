#!/usr/bin/bash
# 26-mac-theme.sh -- bake the WhiteSur macOS-style theme (vinceliuice, GPL-3.0) so
# the first-login "macOS" style is a REAL Mac look: traffic-light window buttons
# (Aurorae), translucent Plasma theme, Big Sur icons. Applied ONLY when the user
# picks macOS in the style chooser, which tints it to Redrob (accent #2B52FF) and
# forces Redrob branding (wallpaper, splash, logo, app icons).
#
# We run WhiteSur's OWN installer for the dark variant: its aurorae themes need a
# per-variant metadata.desktop + shared button svgs that are NOT inside the raw
# aurorae/main/<variant>/ dir, so hand-copying them makes kwin fall back to Breeze
# (no traffic-light buttons). The installer assembles them correctly. We then
# delete the upstream wallpaper/sddm it drops -- that is branding Redrob replaces.
# Upstream: github.com/vinceliuice/WhiteSur-kde , /WhiteSur-icon-theme (pinned).
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0 (our glue only).
set -euo pipefail

# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"
load_pins

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

# WhiteSur KDE theme (dark variant): global theme, Aurorae decoration (+metadata),
# colour schemes, Kvantum -- via its own installer so aurorae loads.
fetch_extract WHITESUR_KDE "${work}/kde"
( cd "${work}/kde" && bash ./install.sh -c dark ) \
  || { echo "error: WhiteSur-kde install.sh failed" >&2; exit 1; }

# Strip upstream BRANDING -- Redrob OS ships its own wallpaper, splash and greeter.
rm -rf /usr/share/wallpapers/WhiteSur* 2>/dev/null || true
rm -rf /usr/share/sddm/themes/WhiteSur* 2>/dev/null || true

# WhiteSur icon theme (Big Sur icons). Redrob brand marks still win via hicolor.
fetch_extract WHITESUR_ICON "${work}/icon"
install_icon_theme "${work}/icon" WhiteSur WhiteSur-dark

echo "26-mac-theme: WhiteSur KDE theme (installer) + icons installed"
