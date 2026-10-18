#!/usr/bin/bash
# greenboot required check: the Redrob OS base pieces this deployment promises
# are present and the OS can still talk to its update source. A non-zero exit
# marks the boot red; after the configured number of red boots greenboot rolls
# back to the previous deployment.
set -euo pipefail

fail() { echo "redrob-base: $*" >&2; exit 1; }

for f in /usr/bin/redrob-launcher /usr/libexec/gvisor/runsc /usr/bin/podman /usr/bin/flatpak \
         /etc/containers/policy.json /usr/share/applications/ai.redrob.launcher.desktop; do
  [ -e "$f" ] || fail "missing $f"
done

# bootc must be able to describe the booted deployment (a broken ostree/bootc
# stack would also break the next update, so fail early).
bootc status --format=json >/dev/null 2>&1 || fail "bootc status failed"

# The sandbox runtime starts and reports its version.
/usr/libexec/gvisor/runsc --version >/dev/null 2>&1 || fail "runsc does not run"

echo "redrob-base: OK"
