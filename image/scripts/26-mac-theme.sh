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
# WhiteSur ships its aurorae decoration VARIANTS under aurorae/main/<variant>/
# (WhiteSur, WhiteSur-dark, WhiteSur-dark_x1.5, ...). Install each as a TOP-LEVEL
# aurorae theme so kwin's __aurorae__svg__<variant> resolves (nesting them under
# one WhiteSur/ dir makes kwin fall back to Breeze -> no traffic-light buttons).
install -d /usr/share/aurorae/themes
cp -a "${work}/kde/aurorae/main/." /usr/share/aurorae/themes/
[ -d "${work}/kde/aurorae/common" ] && cp -a "${work}/kde/aurorae/common" /usr/share/aurorae/themes/

fetch_extract WHITESUR_ICON "${work}/icon"
install_icon_theme "${work}/icon" WhiteSur WhiteSur-dark

echo "26-mac-theme: WhiteSur KDE theme + icons installed"
