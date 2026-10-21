#!/usr/bin/bash
# 15-hardware.sh -- the parts Fedora does not ship but a PC user expects.
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.
#
# * RPM Fusion free + nonfree repositories (same source Bazzite/Universal Blue use).
# * Full ffmpeg + VA-API/VDPAU freeworld drivers: H.264/HEVC/AAC playback and
#   hardware video decode in browsers and mpv.
# * NVIDIA proprietary userspace (x86_64 only); the kernel module comes from the
#   `akmods` stage in image/Containerfile and is installed by 16-nvidia-kmod.sh.
# * Fingerprint login (fprintd + PAM).
#
# Runs after the base dnf transaction and before our overlays.

# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

FEDORA="$(. /usr/lib/os-release && echo "${VERSION_ID}")"
: "${FEDORA:?VERSION_ID missing from /usr/lib/os-release}"

# The release RPM comes from RPM Fusion's own hosts (download1/download2), not the
# `mirrors.rpmfusion.org` redirector: that redirector load-balances to third-party
# mirrors, and a dead one (repos.eggycrew.com, DNS NXDOMAIN) failed the image build
# repeatedly. download1 is authoritative; download2 is the fallback.
rpmfusion_release() {
  local repo="$1" # free | nonfree
  for host in download1.rpmfusion.org download2.rpmfusion.org; do
    if dnf5 -y install --setopt=install_weak_deps=False \
      "https://${host}/${repo}/fedora/rpmfusion-${repo}-release-${FEDORA}.noarch.rpm"; then
      return 0
    fi
    echo "rpmfusion ${repo}: ${host} failed, trying next host" >&2
  done
  return 1
}
rpmfusion_release free
rpmfusion_release nonfree

# Codecs. Fedora's ffmpeg-free stays (apps link against it; a `swap` to the full
# ffmpeg with --allowerasing tore out the whole desktop in CI). libavcodec-freeworld
# adds the patented decoders (H.264/HEVC/AAC...) on top of ffmpeg-free, which is the
# path Fedora itself documents. mesa-va-drivers-freeworld is NOT swapped in: it is
# version-locked to Mesa and RPM Fusion lags the Fedora updates repo, so the swap
# fails whenever Mesa moved (it did in CI). AMD hardware decode of patented codecs
# therefore falls back to software; Intel (intel-media-driver) and NVIDIA
# (libva-nvidia-driver) decode in hardware.
dnf5 -y install --setopt=install_weak_deps=False \
  libavcodec-freeworld \
  gstreamer1-plugins-ugly \
  gstreamer1-plugins-bad-free-extras \
  gstreamer1-plugin-libav \
  libva-utils

# Fingerprint login, phone integration.
dnf5 -y install --setopt=install_weak_deps=False \
  fprintd fprintd-pam \

# NVIDIA (x86_64 only -- RPM Fusion ships no aarch64 build). The kernel module rpm
# built by the `akmods` stage is bind-mounted at /akmods; install it in the SAME
# transaction as the userspace so `nvidia-kmod` is satisfied by it and dnf does not
# pull akmod-nvidia (whose %post tries to build as root and fails).
if [ "${ARCH}" = "x86_64" ]; then
  kmods=()
  for f in /akmods/kmod-nvidia-*.rpm; do [ -e "$f" ] && kmods+=("$f"); done
  [ "${#kmods[@]}" -gt 0 ] || { echo "error: no kmod-nvidia rpm from the akmods stage" >&2; exit 1; }
  dnf5 -y install --setopt=install_weak_deps=False --exclude=akmod-nvidia \
    "${kmods[@]}" \
    intel-media-driver \
    xorg-x11-drv-nvidia \
    xorg-x11-drv-nvidia-libs \
    xorg-x11-drv-nvidia-power \
    libva-nvidia-driver \
    nvidia-settings
  # Keep nouveau off the GPU when the proprietary module is present; harmless
  # on machines without NVIDIA hardware (bootc reads kargs.d at install/upgrade).
  install -D -m 0644 /dev/stdin /usr/lib/bootc/kargs.d/10-nvidia.toml <<'EOF'
# NVIDIA proprietary driver (RPM Fusion akmod built into this image).
kargs = ["rd.driver.blacklist=nouveau", "modprobe.blacklist=nouveau", "nvidia-drm.modeset=1"]
match-architectures = ["x86_64"]
EOF
  systemctl enable nvidia-powerd.service 2>/dev/null || true
fi

dnf5 clean all
