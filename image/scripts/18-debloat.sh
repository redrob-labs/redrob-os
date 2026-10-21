#!/usr/bin/bash
# 18-debloat.sh -- remove base apps Redrob OS replaces with its own, and the
# upstream first-run branding the user should never see.
# * firefox: Redrob Browser is the browser (installed via the Launcher on first
#   login, set default by apps-firstrun), so the base Firefox is removed.
# * plasma-welcome: the KDE Welcome Center auto-launches on first login with
#   "Welcome to KDE Plasma" / upstream branding -- removed so no KDE/Welcome
#   screen appears. Redrob's own first-run is the style chooser + welcome.
# Each removal is a no-op when the base image does not ship the package.
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.

# shellcheck source=lib.sh
. "$(dirname "$0")/lib.sh"

for pkg in firefox firefox-langpacks plasma-welcome; do
  if rpm -q "${pkg}" >/dev/null 2>&1; then
    dnf5 -y remove "${pkg}" || echo "warning: could not remove ${pkg}" >&2
  fi
done

dnf5 clean all
