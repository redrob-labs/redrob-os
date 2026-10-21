#!/usr/bin/bash
# 30-cleanup.sh -- apply systemd presets, then leave the tree in the shape
# `bootc container lint` and `ostree container commit` accept.
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.
#
# Runs after every overlay has been COPYed, so it sees the branding/agent/
# installer units too.

# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

# 1. Presets -> enablement symlinks. /usr/lib/systemd/system-preset/*.preset is
#    honoured on first boot, but devices already installed only pick up new
#    enablements if the symlinks are in the image's /etc, so apply them now.
#    (`systemctl preset-all` would also *disable* everything the base presets
#    omit; we only preset the units we name.)
preset_units() {
  local scope="$1"; shift
  local u
  for u in "$@"; do
    if [ "${scope}" = system ]; then
      systemctl preset "${u}" || echo "warning: system preset ${u} failed (unit absent?)" >&2
    else
      systemctl --global preset "${u}" || echo "warning: user preset ${u} failed (unit absent?)" >&2
    fi
  done
}
#    Read every redrob-owned preset file (base OS 90-redrob, agent 80-redrob-agent,
#    installer 85-redrob-initial-setup...): on the v0.2.6 QEMU install the agent
#    stack stayed disabled because only 90-redrob.preset was applied here.
mapfile -t system_units < <(cat /usr/lib/systemd/system-preset/*redrob*.preset 2>/dev/null | sed -n 's/^enable[[:space:]]\+//p')
mapfile -t user_units   < <(cat /usr/lib/systemd/user-preset/*redrob*.preset 2>/dev/null | sed -n 's/^enable[[:space:]]\+//p')
echo "presets: system=${system_units[*]}"
echo "presets: user=${user_units[*]}"
preset_units system "${system_units[@]}"
preset_units user   "${user_units[@]}"

# 1b. dconf system defaults (GTK fonts and theme, installer overlay) -> binary db. dconf
#     only reads the compiled /etc/dconf/db/local, never the .d sources.
dconf update
[ -e /etc/dconf/db/local ] || { echo "error: dconf update produced no /etc/dconf/db/local" >&2; exit 1; }

# Fedora Flatpaks must not be a remote on Redrob OS: Flathub is the only app source.
# (Same move Universal Blue makes; the unit exists on Fedora Atomic bases.)
if [ -e /usr/lib/systemd/system/flatpak-add-fedora-repos.service ]; then
  systemctl disable flatpak-add-fedora-repos.service || true
  systemctl mask flatpak-add-fedora-repos.service || true
fi

# 2. Package manager leftovers.
dnf5 -y clean all || true
rm -rf /var/cache/* /var/log/* /var/lib/dnf /var/lib/rpm-state 2>/dev/null || true

# 3. /var must be empty in the image: it is machine state, created on first boot
#    (tmpfiles.d / StateDirectory=). Anything we need pre-created is declared in
#    /usr/lib/tmpfiles.d/redrob.conf instead.
find /var/* -maxdepth 0 -exec rm -rf {} + 2>/dev/null || true
install -d -m 1777 /var/tmp

# 4. /opt and /usr/local follow the ostree layout (writable via /var).
if [ -d /opt ] && [ ! -L /opt ]; then
  if [ -n "$(ls -A /opt 2>/dev/null)" ]; then
    echo "error: /opt has content; bootc images keep /opt read-only. Move it under /usr or /var." >&2
    ls -la /opt >&2; exit 1
  fi
  rmdir /opt && ln -s var/opt /opt
fi

# 5. Stray build artefacts bootc lint rejects.
rm -rf /usr/etc /tmp/* 2>/dev/null || true
# shellcheck disable=SC2114  # container build root, not a live system
rm -rf /boot && mkdir -p /boot
rm -f /.gitkeep /.dockerenv 2>/dev/null || true

# 5b. Overlay files are COPYed with git's modes; scripts greenboot runs must be executable.
find /etc/greenboot/check -type f -name '*.sh' -exec chmod 0755 {} +

# 6. Sanity: the things later stages rely on must be present. greenboot's binary path
#    moved between releases (/usr/bin -> /usr/libexec), so check its unit instead.
for f in /usr/bin/redrob-launcher /usr/libexec/gvisor/runsc /usr/bin/bootc \
         /usr/lib/systemd/system/greenboot-healthcheck.service \
         /usr/bin/redrob-agent /usr/bin/redrob-broker /usr/bin/redrob-usb-broker /usr/bin/redrob-pairing \
         /etc/containers/policy.json /etc/containers/containers.conf.d/50-redrob-runsc.conf \
         /usr/bin/redrob-comp /usr/bin/redrob-session /etc/greetd/config.toml \
         /usr/share/wallpapers/Redrob/metadata.json; do
  [ -e "${f}" ] || { echo "error: expected ${f} missing after build" >&2; exit 1; }
done
# The desktop: greetd runs redrob-comp's login screen. Plasma must not come back
# in through a dependency.
rpm -q greetd >/dev/null || { echo "error: greetd not installed" >&2; exit 1; }
rpm -q generic-logos >/dev/null || { echo "error: generic-logos not installed" >&2; exit 1; }
for p in plasma-workspace kwin plasma-login-manager; do
  if rpm -q "${p}" >/dev/null 2>&1; then echo "error: ${p} is installed (Plasma came back)" >&2; exit 1; fi
done
if rpm -q fedora-logos >/dev/null 2>&1; then echo "error: fedora-logos still installed" >&2; exit 1; fi

echo "cleanup done"
