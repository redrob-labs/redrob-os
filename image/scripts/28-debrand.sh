#!/usr/bin/bash
# 28-debrand.sh -- strip the visible KDE/Plasma and Fedora WORDMARKS that base
# packages bake into rpm-owned text files, so no on-screen string names an
# upstream project. Logos, colours, wallpaper, splash and the start-menu mark are
# already Redrob (image/branding overlay); this only rewrites display strings.
# Functional identifiers (DesktopNames=KDE, Exec=, ID_LIKE=fedora, the
# org.kde.plasma.* applet ids) are deliberately left alone -- they are API the
# desktop needs, not branding the user reads.
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.
set -euo pipefail

# 1. Login session picker: the Redrob session (redrob-comp) is the desktop.
#    The base's Plasma sessions are taken off the list. Their packages stay,
#    because the login manager and the Qt apps are built on them.
shopt -s nullglob
for d in /usr/share/wayland-sessions /usr/share/xsessions; do
  [ -d "$d" ] || continue
  for f in "$d"/*.desktop; do
    [ "$(basename "$f")" = redrob.desktop ] || rm -f "$f"
  done
done
[ -e /usr/share/wayland-sessions/redrob.desktop ] || { echo "error: redrob session missing" >&2; exit 1; }

# 2. Console login banner: agetty prints /etc/issue on each VT; the stock file
#    expands \S to the system-release ("Fedora ..."). Ship a plain Redrob banner.
printf 'Redrob OS \\r (\\l)\n\n' > /etc/issue
printf 'Redrob OS \\r (\\l)\n\n' > /etc/issue.net

# 3. About-this-System distro logo already follows os-release LOGO=redrob-os.
#    The "KDE Plasma / Frameworks / Qt Version" lines in that panel come from the
#    kinfocenter kcm and are NOT removable without patching the package (which
#    would re-brand on every update); left as-is by design.

echo "28-debrand: login session names + console issue rebranded"
