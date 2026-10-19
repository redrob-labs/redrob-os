#!/bin/bash
# Sandbox entrypoint: bridge the broker's per-task egress socket to loopback TCP.
# Copyright 2026 Janghoon Lee (Redrob) and contributors. Apache-2.0.
#
# The task container runs with --network none (loopback only). When the agent
# mounts a broker socket at /run/redrob/egress.sock it also sets HTTP(S)_PROXY to
# http://127.0.0.1:3128; this makes that address real. No socket = no network,
# and the command runs exactly as before.
sock=/run/redrob/egress.sock
if [ -S "$sock" ]; then
  socat -T 300 TCP-LISTEN:3128,bind=127.0.0.1,reuseaddr,fork UNIX-CONNECT:"$sock" &
  # Wait briefly for the listener so the first request does not race it.
  i=0
  while [ "$i" -lt 20 ] && ! (exec 3<>/dev/tcp/127.0.0.1/3128) 2>/dev/null; do
    i=$((i + 1))
    sleep 0.05
  done
fi
exec "$@"
