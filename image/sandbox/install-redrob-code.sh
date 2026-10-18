#!/usr/bin/bash
# install-redrob-code.sh -- put Redrob Code (headless) into the task sandbox image.
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.
#
# Pinned in image/pins.env (REDROB_CODE_*). The Linux tarball from
# github.com/redrob-labs/redrob-code releases unpacks to
#   redrob-code-<ver>-linux-<arch>/bin/{redrob,redrob-code}
# Both land in /usr/local/bin. Needs curl (sandbox base has it) and tar.

# shellcheck source=../scripts/lib.sh
. "$(dirname "$0")/lib.sh"
load_pins

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

fetch_pinned REDROB_CODE "${work}/redrob-code.tar.gz"
tar -xzf "${work}/redrob-code.tar.gz" --directory="${work}" --no-same-owner
src="${work}/redrob-code-${REDROB_CODE_VERSION}-linux-${ARCH}"
[ -x "${src}/bin/redrob-code" ] || { echo "unexpected tarball layout under ${src}" >&2; ls -R "${work}" >&2; exit 1; }

install -D -m 0755 "${src}/bin/redrob-code" /usr/local/bin/redrob-code
[ -x "${src}/bin/redrob" ] && install -D -m 0755 "${src}/bin/redrob" /usr/local/bin/redrob
echo "redrob-code ${REDROB_CODE_VERSION} (${ARCH}) installed"
