#!/usr/bin/bash
# 27-win-theme.sh -- bake the Win11OS KDE theme (yeyushengfan258, GPL-3.0) so the
# first-login "Windows" style is a REAL Win11 look: pill window buttons (Aurorae),
# Fluent panels, Win11 icons. Applied ONLY when the user picks Windows in the
# style chooser, which tints it to Redrob and forces Redrob branding (wallpaper,
# splash, logo, app icons). Only the "look" comes from upstream -- none of its
# wallpaper/sddm/splash is installed.
# Upstream: github.com/yeyushengfan258/Win11OS-kde , /Win11-icon-theme (pinned by sha).
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0 (our glue only).
set -euo pipefail

# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"
load_pins

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

fetch_extract WIN11_KDE "${work}/kde"
theme_copy_look "${work}/kde"
# Win11OS ships aurorae decorations already as theme dirs (Win11OS-dark/-light).
cp -a "${work}/kde/aurorae/." /usr/share/aurorae/themes/

fetch_extract WIN11_ICON "${work}/icon"
install_icon_theme "${work}/icon" Win11 Win11-dark

echo "27-win-theme: Win11OS KDE theme + icons installed"
