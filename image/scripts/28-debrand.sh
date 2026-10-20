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

# 1. Login session picker. Plasma Login Manager lists each session by its Name=
#    from /usr/share/{wayland-sessions,xsessions}/*.desktop ("Plasma (Wayland)"
#    etc.). Rewrite the human strings only (Name/GenericName/Comment, including
#    localized Name[xx]=) -- never Exec/TryExec/DesktopNames.
shopt -s nullglob
for d in /usr/share/wayland-sessions /usr/share/xsessions; do
  [ -d "$d" ] || continue
  for f in "$d"/plasma*.desktop; do
    sed -i -E '/^(Name|GenericName|Comment)(\[[^]]*\])?=/{ s/\bPlasma\b/Redrob/g; s/\bKDE\b/Redrob/g }' "$f"
  done
done

# 2. Console login banner: agetty prints /etc/issue on each VT; the stock file
#    expands \S to the system-release ("Fedora ..."). Ship a plain Redrob banner.
printf 'Redrob OS \\r (\\l)\n\n' > /etc/issue
printf 'Redrob OS \\r (\\l)\n\n' > /etc/issue.net

# 3. About-this-System distro logo already follows os-release LOGO=redrob-os.
#    The "KDE Plasma / Frameworks / Qt Version" lines in that panel come from the
#    kinfocenter kcm and are NOT removable without patching the package (which
#    would re-brand on every update); left as-is by design.

echo "28-debrand: login session names + console issue rebranded"
