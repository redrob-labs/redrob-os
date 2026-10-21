#!/usr/bin/bash
# 27-win-theme.sh -- bake the Win11OS KDE theme (yeyushengfan258, GPL-3.0) so the
# first-login "Windows" style is a REAL Win11 look: pill window buttons (Aurorae),
# Fluent panels, Win11 icons. Applied ONLY when the user picks Windows in the
# style chooser, which applies it STOCK (light, its wallpaper); the only Redrob
# change is the logo (start/OS logos -> Redrob OS mark, Redrob splash). Its
# sddm themes are not installed -- the greeter stays Redrob.
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

install -d /usr/share/wallpapers
cp -a "${work}/kde/wallpaper/." /usr/share/wallpapers/   # stock Win11OS wallpaper

fetch_extract WIN11_ICON "${work}/icon"
install_icon_theme "${work}/icon" Win11 Win11-dark

# Rebrand only: OS/start logos -> Redrob OS mark.
rebrand_logos /usr/share/plasma/desktoptheme/Win11OS* /usr/share/icons/Win11*

echo "27-win-theme: Win11OS (stock) + icons installed, logos rebranded"
