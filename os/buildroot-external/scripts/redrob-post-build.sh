#!/bin/bash
# Redrob additions on top of post-build.sh (runs after it, same arguments).
set -e

BOARD_DIR=${2}
# shellcheck source=/dev/null
. "${BR2_EXTERNAL_HAOS_PATH}/meta"
# shellcheck source=/dev/null
. "${BOARD_DIR}/meta"

# os-release: vendor fields that post-build.sh hardcodes to upstream.
sed -i \
    -e 's|^CPE_NAME=cpe:2.3:o:home-assistant:|CPE_NAME=cpe:2.3:o:redrob:|' \
    -e 's|^HOME_URL=.*|HOME_URL=https://redrob.ai/|' \
    -e '/^SUPERVISOR_MACHINE=/d' \
    -e '/^SUPERVISOR_ARCH=/d' \
    "${TARGET_DIR}/usr/lib/os-release"
{
    echo "DOCUMENTATION_URL=https://github.com/redrob-labs/redrob-os"
    echo "LOGO=redrob"
} >> "${TARGET_DIR}/usr/lib/os-release"

# Supervisor-era units: the Redrob agent replaces them. Mask rather than
# delete so the upstream overlay stays untouched and the subtree still merges.
for unit in haos-supervisor.service haos-apparmor.service haos-bt-cache.timer; do
    if [ -e "${TARGET_DIR}/usr/lib/systemd/system/${unit}" ]; then
        "${HOST_DIR}/bin/systemctl" --root="${TARGET_DIR}" mask "${unit}" >/dev/null 2>&1 || true
    fi
done

# /etc/issue carries the product name; machine-info chassis stays from board meta.
printf 'Welcome to Redrob OS %s (%s)\n' "$(. "${BR2_EXTERNAL_HAOS_PATH}/scripts/name.sh"; haos_version)" "${BOARD_NAME}" \
    > "${TARGET_DIR}/etc/issue"
# Login MOTD from the upstream overlay still says Home Assistant; replace it
# here rather than in the overlay so the subtree keeps merging cleanly.
printf 'Welcome to \033[01;31mRedrob OS\033[00m.\n\nThe agent listens on :42617; `rauc status` shows the A/B slots.\n' \
    > "${TARGET_DIR}/etc/motd"

# RAUC caps `rauc install <url>` at 8 MiB by default; upstream never hits it
# because the Supervisor downloads the bundle to disk first. Our bundles are
# ~290 MB, so raise the cap (1 GiB) for direct-URL installs. The agent still
# prefers downloading to /mnt/data/redrob/ota and installing the local path.
if ! grep -q '^max-bundle-download-size=' "${TARGET_DIR}/etc/rauc/system.conf"; then
    sed -i '/^statusfile=/a max-bundle-download-size=1073741824' "${TARGET_DIR}/etc/rauc/system.conf"
fi

# Identity must never be in the image: fail the build if anything slipped in.
if [ -e "${TARGET_DIR}/mnt/data/redrob/identity" ] || find "${TARGET_DIR}" -name 'device.key' | grep -q .; then
    echo "redrob-post-build: device identity material found in rootfs" >&2
    exit 1
fi
