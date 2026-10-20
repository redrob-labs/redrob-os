#!/usr/bin/bash
# 16-gaming.sh -- the Steam/Proton gaming layer (v0.5.0). Host tools a PC gamer
# expects, so Steam + Proton "just work" after the first-boot install. Steam
# itself is NOT baked into the image (it is a ~1-2 GB Flathub app pulled on first
# boot by redrob-steam.service, and a user who removes it stays removed); this
# script ships only the small host pieces Flatpak games cannot bring themselves:
#   * gamemode   -- CPU governor / niceness boost while a game runs (D-Bus activated).
#   * gamescope  -- micro-compositor for scaling / frame-limiting a game window.
#   * mangohud   -- in-game FPS / temp overlay (host build for native apps).
#   * game-devices-udev -- udev rules so Xbox/PS/Switch pads work without root.
# Proton ships inside Steam; ProtonUp-Qt (first boot) adds Proton-GE on top.
# League of Legends is NOT supported (Vanguard is a Windows kernel driver); the
# installer's dual-boot path is the only route for it.
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.

# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

# All four are in the Fedora 44 repos (no RPM Fusion needed), so this does not
# depend on the mirror hosts 15-hardware pins.
dnf5 -y install --setopt=install_weak_deps=False \
  gamemode \
  gamescope \
  mangohud \
  game-devices-udev

# Some Proton/Unity titles mmap far more regions than the old default allowed.
# Fedora 44 already raises this via systemd, but pin it so a game never dies with
# "Not enough memory" on a machine that inherited an older value.
install -D -m 0644 /dev/stdin /usr/lib/sysctl.d/99-redrob-gaming.conf <<'EOF'
# Redrob OS gaming layer: large mmap count for Proton/Unity games (Bazzite uses
# the same value).
vm.max_map_count = 2147483642
EOF

dnf5 clean all
