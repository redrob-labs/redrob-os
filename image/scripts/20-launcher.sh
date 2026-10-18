#!/usr/bin/bash
# 20-launcher.sh -- bake Redrob Launcher into the image.
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.
#
# Pinned in image/pins.env (REDROB_LAUNCHER_*). The Linux tarball from
# github.com/redrob-labs/redrob-launcher releases unpacks to
#   redrob-launcher-<ver>-linux-<arch>/
#     bin/redrob-launcher                              (static-ish ELF: libc/libm/libgcc only)
#     share/applications/ai.redrob.launcher.desktop
#     share/icons/hicolor/{16x16,...,512x512,scalable}/apps/ai.redrob.launcher.{png,svg}
#     share/metainfo/ai.redrob.launcher.metainfo.xml
#     share/doc/redrob-launcher/{LICENSE-APACHE,LICENSE-MIT,NOTICE,ATTRIBUTION.md,README.md}
# We install share/ as-is under /usr/share and the binary to /usr/bin, then add an
# XDG autostart entry so the launcher (tray icon + app updater) starts at login.

# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"
load_pins

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

fetch_pinned REDROB_LAUNCHER "${work}/launcher.tar.gz"
tar -xzf "${work}/launcher.tar.gz" --directory="${work}" --no-same-owner
src="${work}/redrob-launcher-${REDROB_LAUNCHER_VERSION}-linux-${ARCH}"
[ -x "${src}/bin/redrob-launcher" ] || { echo "unexpected tarball layout under ${src}" >&2; ls -R "${work}" >&2; exit 1; }

install -D -m 0755 "${src}/bin/redrob-launcher" /usr/bin/redrob-launcher
# share/: desktop entry, icons, AppStream metainfo, licences -- same relative layout.
cp -a "${src}/share/." /usr/share/
chmod -R u=rwX,go=rX /usr/share/doc/redrob-launcher /usr/share/metainfo/ai.redrob.launcher.metainfo.xml \
    /usr/share/applications/ai.redrob.launcher.desktop
find /usr/share/icons/hicolor -path '*ai.redrob.launcher*' -exec chmod 0644 {} +

# Autostart at login (all desktops; KDE honours /etc/xdg/autostart). Hidden=false
# is the default; users can turn it off in System Settings > Autostart.
install -d -m 0755 /etc/xdg/autostart
{
  cat /usr/share/applications/ai.redrob.launcher.desktop
  printf 'X-KDE-autostart-phase=2\n'
} > /etc/xdg/autostart/ai.redrob.launcher.desktop
chmod 0644 /etc/xdg/autostart/ai.redrob.launcher.desktop

install -d -m 0755 /usr/share/redrob
printf 'REDROB_LAUNCHER_VERSION=%s\n' "${REDROB_LAUNCHER_VERSION}" >> /usr/share/redrob/pins-installed.env

echo "Redrob Launcher ${REDROB_LAUNCHER_VERSION} (${ARCH}) installed to /usr/bin/redrob-launcher"
