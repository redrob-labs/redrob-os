#!/usr/bin/bash
# 26-mac-theme.sh -- bake the WhiteSur macOS-style theme (vinceliuice, GPL-3.0) so
# the first-login "macOS" style is a REAL Mac look: traffic-light window buttons
# (Aurorae), translucent Plasma theme, Big Sur icons. Applied ONLY when the user
# picks macOS in the style chooser, which tints it to Redrob (accent #2B52FF) and
# forces Redrob branding (wallpaper, splash, logo, app icons). Only the "look"
# comes from upstream -- none of its wallpaper/sddm/splash is installed.
# Upstream: github.com/vinceliuice/WhiteSur-kde , /WhiteSur-icon-theme (pinned).
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0 (our glue only).
set -euo pipefail

# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"
load_pins

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

fetch_extract WHITESUR_KDE "${work}/kde"
theme_copy_look "${work}/kde"
# WhiteSur ships one aurorae decoration under aurorae/main -> themes/WhiteSur.
install -d /usr/share/aurorae/themes/WhiteSur
cp -a "${work}/kde/aurorae/main/." /usr/share/aurorae/themes/WhiteSur/
[ -f "${work}/kde/aurorae/metadata.desktop" ] && \
  cp -a "${work}/kde/aurorae/metadata.desktop" /usr/share/aurorae/themes/WhiteSur/metadata.desktop

fetch_extract WHITESUR_ICON "${work}/icon"
install_icon_theme "${work}/icon" WhiteSur WhiteSur-dark

echo "26-mac-theme: WhiteSur KDE theme + icons installed"
