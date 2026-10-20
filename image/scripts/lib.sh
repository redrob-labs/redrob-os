#!/usr/bin/bash
# Shared helpers for the Redrob OS image build scripts.
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.
#
# Sourced by 10-pins.sh, 20-launcher.sh, 30-cleanup.sh. Not executable on purpose.

set -euo pipefail

CTX="${CTX:-/ctx}"
PINS_FILE="${PINS_FILE:-${CTX}/pins.env}"

# Target architecture as the upstream download URLs spell it (x86_64 | aarch64).
# `uname -m` reports the *target* under qemu-user emulation too, so this is right
# for cross builds; TARGETARCH (amd64|arm64) is honoured when podman passes it.
case "${TARGETARCH:-}" in
  amd64) ARCH=x86_64 ;;
  arm64) ARCH=aarch64 ;;
  "")    ARCH="$(uname -m)" ;;
  *)     echo "unsupported TARGETARCH=${TARGETARCH}" >&2; exit 1 ;;
esac
export ARCH
ARCH_UPPER="$(echo "${ARCH}" | tr '[:lower:]' '[:upper:]')"

load_pins() {
  [ -r "${PINS_FILE}" ] || { echo "pins file not found: ${PINS_FILE}" >&2; exit 1; }
  # pins.env is plain KEY=VALUE shell; ${ARCH} inside URLs expands here.
  set -a
  # shellcheck disable=SC1090
  . "${PINS_FILE}"
  set +a
}

# pin_value NAME FIELD  -> value of NAME_FIELD, preferring the per-arch variant
# (NAME_FIELD_X86_64 / NAME_FIELD_AARCH64) when it exists.
pin_value() {
  local name="$1" field="$2" v
  v="$(eval "printf '%s' \"\${${name}_${field}_${ARCH_UPPER}:-}\"")"
  [ -n "${v}" ] || v="$(eval "printf '%s' \"\${${name}_${field}:-}\"")"
  printf '%s' "${v}"
}

# fetch_pinned NAME DEST  -> downloads NAME_URL to DEST and verifies the pinned
# sha256/sha512. Fails the build on a missing pin, a TODO-hash, or a mismatch.
fetch_pinned() {
  local name="$1" dest="$2" url sha256 sha512 actual
  url="$(pin_value "${name}" URL)"
  sha256="$(pin_value "${name}" SHA256)"
  sha512="$(pin_value "${name}" SHA512)"
  [ -n "${url}" ] || { echo "${name}_URL missing in ${PINS_FILE}" >&2; exit 1; }
  if [ -z "${sha256}${sha512}" ] || [ "${sha256}" = "TODO-hash" ] || [ "${sha512}" = "TODO-hash" ]; then
    echo "${name}: no usable checksum pinned (TODO-hash?) -- refusing to install" >&2
    exit 1
  fi
  echo "fetching ${name} ${ARCH}: ${url}"
  curl --fail --silent --show-error --location --retry 3 --retry-delay 5 \
       --proto '=https' --tlsv1.2 -o "${dest}" "${url}"
  if [ -n "${sha512}" ]; then
    actual="$(sha512sum "${dest}" | cut -d' ' -f1)"
    [ "${actual}" = "${sha512}" ] || { echo "${name}: sha512 mismatch (got ${actual}, want ${sha512})" >&2; exit 1; }
  else
    actual="$(sha256sum "${dest}" | cut -d' ' -f1)"
    [ "${actual}" = "${sha256}" ] || { echo "${name}: sha256 mismatch (got ${actual}, want ${sha256})" >&2; exit 1; }
  fi
  echo "${name}: checksum OK"
}

# fetch_extract NAME DESTDIR  -- fetch_pinned NAME into a temp tarball and extract
# it into DESTDIR (created), stripping the archive's top directory. Echoes DESTDIR.
fetch_extract() {
  local name="$1" dest="$2" tgz
  tgz="$(mktemp)"
  fetch_pinned "${name}" "${tgz}"
  mkdir -p "${dest}"
  tar -xzf "${tgz}" -C "${dest}" --strip-components=1
  rm -f "${tgz}"
}

# theme_copy_look SRC  -- copy the "look" assets of a WhiteSur/Win11-style KDE
# theme tree (colour schemes, Kvantum, Plasma desktop theme + look-and-feel) into
# /usr/share. Deliberately NOT wallpaper/ or sddm*/ -- those are branding Redrob
# OS replaces with its own. Aurorae decorations are copied by the caller (their
# layout differs between themes).
theme_copy_look() {
  local src="$1"
  install -d /usr/share/color-schemes /usr/share/Kvantum \
            /usr/share/plasma/desktoptheme /usr/share/plasma/look-and-feel
  cp -a "${src}/color-schemes/." /usr/share/color-schemes/
  cp -a "${src}/Kvantum/." /usr/share/Kvantum/
  cp -a "${src}/plasma/desktoptheme/." /usr/share/plasma/desktoptheme/
  cp -a "${src}/plasma/look-and-feel/." /usr/share/plasma/look-and-feel/
}

# install_icon_theme SRC  -- run an icon theme's own install.sh against
# /usr/share/icons, then refresh caches for the named theme dirs ($@ after SRC).
install_icon_theme() {
  local src="$1"; shift
  ( cd "${src}" && bash ./install.sh -d /usr/share/icons ) \
    || echo "warning: icon-theme install returned nonzero" >&2
  local d
  for d in "$@"; do
    [ -d "/usr/share/icons/${d}" ] && gtk-update-icon-cache -q -f "/usr/share/icons/${d}" 2>/dev/null || true
  done
}
