#!/usr/bin/bash
# 18-debloat.sh -- remove base apps Redrob OS replaces with its own.
# Firefox: Redrob Browser is the browser (installed via the Launcher on first
# login, set default by browser-firstrun), so the base Firefox is removed. The
# removal is a no-op when the base image does not ship it.
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.

# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

for pkg in firefox firefox-langpacks; do
  if rpm -q "${pkg}" >/dev/null 2>&1; then
    dnf5 -y remove "${pkg}" || echo "warning: could not remove ${pkg}" >&2
  fi
done

dnf5 clean all
