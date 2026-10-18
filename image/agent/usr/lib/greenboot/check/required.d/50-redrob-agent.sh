#!/bin/bash
# greenboot required health check: the Redrob agent must come up on every boot.
# Copyright 2026 Janghoon Lee (Redrob) and contributors
# SPDX-License-Identifier: Apache-2.0
#
# Replaces Buildroot's redrob-mark-good.service (rauc mark-good). On bootc the
# boot counter / rollback is greenboot's: a non-zero exit from any script in
# /usr/lib/greenboot/check/required.d marks the boot RED, redboot-auto-reboot
# retries, and after GREENBOOT_MAX_BOOT_ATTEMPTS bootc rolls back to the previous
# deployment. A boot where the agent never answers /health is a boot that must
# NOT be marked good -- that is the one thing an OS update can break for us.
set -u

UNIT=redrob-agent.service
HEALTH=http://127.0.0.1:42617/health
# Up to 5 min: first boot renders config and keys; small CPUs are slow.
ATTEMPTS=150
SLEEP=2

# The user disabled the agent on purpose (systemctl disable). Not a bad boot.
state=$(systemctl is-enabled "$UNIT" 2>/dev/null || true)
case "$state" in
    enabled|enabled-runtime|static) ;;
    *)
        echo "redrob-agent: $UNIT is $state; skipping health check"
        exit 0
        ;;
esac

for ((i = 1; i <= ATTEMPTS; i++)); do
    if curl -sf -o /dev/null --max-time 3 "$HEALTH"; then
        echo "redrob-agent: healthy ($HEALTH)"
        exit 0
    fi
    # Fail fast when the unit is wedged instead of waiting the full window.
    if [[ "$(systemctl is-failed "$UNIT" 2>/dev/null)" == "failed" ]]; then
        echo "redrob-agent: $UNIT failed" >&2
        systemctl status --no-pager -l "$UNIT" >&2 || true
        exit 1
    fi
    sleep "$SLEEP"
done

echo "redrob-agent: no answer from $HEALTH after $((ATTEMPTS * SLEEP))s; boot is not healthy" >&2
journalctl -u "$UNIT" -b --no-pager -n 30 >&2 || true
exit 1
