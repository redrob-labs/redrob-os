#!/usr/bin/bash
# 26-mac-theme.sh -- bake the WhiteSur macOS-style theme (vinceliuice, GPL-3.0) so
# the first-login "macOS" style is a REAL Mac look: traffic-light window buttons
# (Aurorae), translucent Plasma theme, Big Sur icons. It is applied ONLY when the
# user picks macOS in the style chooser; the Windows style keeps stock Breeze.
# style-chooser tints it to Redrob (accent #2B52FF + Redrob wallpaper), so it is a
# Redrob-flavoured macOS, not plain WhiteSur.
# Upstream: github.com/vinceliuice/WhiteSur-kde , /WhiteSur-icon-theme (pinned).
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0 (our glue only).
set -euo pipefail

# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"
load_pins

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

# --- WhiteSur KDE theme: global theme, Aurorae decoration, colour schemes, Kvantum.
fetch_pinned WHITESUR_KDE "${work}/kde.tar.gz"
mkdir -p "${work}/kde"
tar -xzf "${work}/kde.tar.gz" -C "${work}/kde" --strip-components=1
src="${work}/kde"

install -d /usr/share/color-schemes /usr/share/Kvantum \
          /usr/share/aurorae/themes/WhiteSur \
          /usr/share/plasma/desktoptheme /usr/share/plasma/look-and-feel

cp -a "${src}/color-schemes/." /usr/share/color-schemes/
cp -a "${src}/Kvantum/." /usr/share/Kvantum/
cp -a "${src}/aurorae/main/." /usr/share/aurorae/themes/WhiteSur/
[ -f "${src}/aurorae/metadata.desktop" ] && \
  cp -a "${src}/aurorae/metadata.desktop" /usr/share/aurorae/themes/WhiteSur/metadata.desktop
cp -a "${src}/plasma/desktoptheme/." /usr/share/plasma/desktoptheme/
cp -a "${src}/plasma/look-and-feel/." /usr/share/plasma/look-and-feel/

# --- WhiteSur icon theme (Big Sur icons). Their installer copies the generated
#     theme tree into the target dir; run it against /usr/share/icons.
fetch_pinned WHITESUR_ICON "${work}/icon.tar.gz"
mkdir -p "${work}/icon"
tar -xzf "${work}/icon.tar.gz" -C "${work}/icon" --strip-components=1
( cd "${work}/icon" && bash ./install.sh -d /usr/share/icons ) \
  || echo "warning: WhiteSur icon-theme install returned nonzero" >&2

# Refresh icon caches where present (non-fatal).
for d in /usr/share/icons/WhiteSur /usr/share/icons/WhiteSur-dark; do
  [ -d "${d}" ] && gtk-update-icon-cache -q -f "${d}" 2>/dev/null || true
done

echo "26-mac-theme: WhiteSur KDE theme + icons installed"
