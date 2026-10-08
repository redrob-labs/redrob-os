#!/usr/bin/env bash
# Verify deploy/systemd/*.service with systemd-analyze in a throwaway root.
# The real binaries and the /mnt/data mount do not exist on a dev host, so
# stubs stand in for them; only the unit files themselves are under test.
set -euo pipefail

cd "$(dirname "$0")/.."
root="$(mktemp -d "${TMPDIR:-/tmp}/redrob-units.XXXXXX")"
trap 'rm -rf "$root"' EXIT

mkdir -p "$root/etc/systemd/system" "$root/usr/bin" "$root/usr/libexec"
cp deploy/systemd/*.service "$root/etc/systemd/system/"

printf '[Unit]\nDescription=stub\n[Mount]\nWhat=/dev/null\nWhere=/mnt/data\nType=ext4\n' \
  > "$root/etc/systemd/system/mnt-data.mount"
for t in sysinit.target basic.target multi-user.target network-online.target \
         network.target local-fs.target shutdown.target; do
  printf '[Unit]\nDescription=stub\n' > "$root/etc/systemd/system/$t"
done
install -m 755 /bin/true "$root/usr/bin/redrob-agent"
install -m 755 /bin/true "$root/usr/libexec/redrob-firstboot"
install -m 755 /bin/true "$root/usr/bin/rauc"
printf "[Unit]\\nDescription=stub\\n[Service]\\nExecStart=/usr/bin/rauc\\n" > "$root/etc/systemd/system/rauc.service"
mkdir -p "$root/bin" && install -m 755 /bin/true "$root/bin/sh"

units=()
for f in deploy/systemd/*.service; do units+=("$(basename "$f")"); done

# Any warning (unknown key, bad value) is a failure: a key in the wrong
# section is silently ignored at runtime, which is exactly the bug we want
# to catch.
out="$(systemd-analyze verify --root="$root" --man=no "${units[@]}" 2>&1)" || { echo "$out"; exit 1; }
if [[ -n "$out" ]]; then echo "$out"; exit 1; fi
echo "${#units[@]} units verified"
