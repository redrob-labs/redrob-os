#!/usr/bin/bash
# 25-fonts.sh -- bake the Redrob Group Design System UI typefaces into the image.
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.
#
# 30-typography.md: Pretendard is the Latin system face, Wanted Sans the Korean
# one (Nanum Myeongjo, the Korean voice face, comes from Fedora's
# naver-nanum-fonts). Both are SIL OFL 1.1, so shipping them in the OS image is
# permitted; both come from their upstream GitHub releases, pinned in
# image/pins.env (PRETENDARD_*, WANTED_SANS_*). We install the static OTFs only
# (no variable fonts: Qt6/KDE pick weights more reliably from static faces) to
# /usr/share/fonts/redrob/<family>/ with the licence beside them.
# /etc/fonts/conf.d/60-redrob.conf (image/files) makes Pretendard the default
# sans and lets it fall through to Wanted Sans for Hangul.

# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"
load_pins

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

dest=/usr/share/fonts/redrob
install -d -m 0755 "${dest}/pretendard" "${dest}/wanted-sans"

fetch_pinned PRETENDARD "${work}/pretendard.zip"
unzip -q "${work}/pretendard.zip" -d "${work}/pretendard"
# zip layout: public/static/Pretendard-<Weight>.otf (+ alternative/*.ttf, variable/, web/)
n=0
for w in Light Regular Medium SemiBold Bold ExtraBold; do
  f="${work}/pretendard/public/static/Pretendard-${w}.otf"
  [ -f "${f}" ] || { echo "Pretendard-${w}.otf missing from the release zip" >&2; ls -R "${work}/pretendard" >&2; exit 1; }
  install -m 0644 "${f}" "${dest}/pretendard/"
  n=$((n + 1))
done
lic="$(find "${work}/pretendard" -maxdepth 2 -iname 'LICENSE*' -print -quit)"
[ -n "${lic}" ] && install -m 0644 "${lic}" "${dest}/pretendard/LICENSE.txt"
echo "Pretendard ${PRETENDARD_VERSION}: ${n} faces"

fetch_pinned WANTED_SANS "${work}/wanted-sans.zip"
unzip -q "${work}/wanted-sans.zip" -d "${work}/wanted-sans"
# zip layout: otf/WantedSans-<Weight>.otf, ttf/, variable/
n=0
for w in Regular Medium SemiBold Bold ExtraBold; do
  f="$(find "${work}/wanted-sans" -path '*/otf/WantedSans-'"${w}"'.otf' -print -quit)"
  [ -n "${f}" ] || { echo "WantedSans-${w}.otf missing from the release zip" >&2; ls -R "${work}/wanted-sans" >&2; exit 1; }
  install -m 0644 "${f}" "${dest}/wanted-sans/"
  n=$((n + 1))
done
lic="$(find "${work}/wanted-sans" -maxdepth 2 \( -iname 'LICENSE*' -o -iname 'OFL*' \) -print -quit)"
[ -n "${lic}" ] && install -m 0644 "${lic}" "${dest}/wanted-sans/LICENSE.txt"
echo "Wanted Sans ${WANTED_SANS_VERSION}: ${n} faces"

fc-cache -f "${dest}" >/dev/null
fc-list | grep -q 'Pretendard' || { echo "fontconfig does not see Pretendard after install" >&2; exit 1; }
fc-list | grep -q 'Wanted Sans' || { echo "fontconfig does not see Wanted Sans after install" >&2; exit 1; }

install -d -m 0755 /usr/share/redrob
{
  printf 'PRETENDARD_VERSION=%s\n' "${PRETENDARD_VERSION}"
  printf 'WANTED_SANS_VERSION=%s\n' "${WANTED_SANS_VERSION}"
} >> /usr/share/redrob/pins-installed.env
