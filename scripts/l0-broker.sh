#!/usr/bin/env bash
# L0: run the credential broker directly on the dev host, no systemd.
#   scripts/l0-broker.sh build        # cargo release build
#   scripts/l0-broker.sh test         # cargo test (unit + integration, mock vendor)
#   scripts/l0-broker.sh start|stop|status|logs|audit
#   scripts/l0-broker.sh call '<json>' # POST /v1/call over the socket (curl --unix-socket)
# State lives under $L0_BROKER_ROOT (default ~/.local/state/redrob-broker). The egress proxy
# listens on 127.0.0.1:3128 as on the device, so l0-agent.sh's rendered config works unchanged.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
. "$ROOT/scripts/dev-env.sh"
L0_ROOT="${L0_BROKER_ROOT:-$HOME/.local/state/redrob-broker}"
CRATE="$ROOT/modules/credential-broker/broker"
BIN="$CRATE/target/release/redrob-broker"
SOCK="$L0_ROOT/broker.sock"
PIDFILE="$L0_ROOT/daemon.pid"
LOG="$L0_ROOT/logs/daemon.log"
export RUST_LOG="${RUST_LOG:-info}"

cmd="${1:-status}"
case "$cmd" in
  build) (cd "$CRATE" && cargo build --release) ;;
  test)  (cd "$CRATE" && cargo test) ;;
  start)
    [ -x "$BIN" ] || { echo "no binary, run: $0 build" >&2; exit 1; }
    mkdir -p "$L0_ROOT/logs" "$L0_ROOT/state" "$L0_ROOT/audit"
    if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
      echo "already running pid $(cat "$PIDFILE")"; exit 0
    fi
    # admin = this user (on the device: redrob-agent)
    setsid nohup "$BIN" --config "$ROOT/deploy/config/broker.toml" --socket "$SOCK" \
        --state-dir "$L0_ROOT/state" --audit-dir "$L0_ROOT/audit" --admin-uid "$(id -u)" \
        >>"$LOG" 2>&1 &
    echo $! > "$PIDFILE"
    echo "started pid $! socket $SOCK log $LOG"
    ;;
  stop)
    if [ -f "$PIDFILE" ]; then kill "$(cat "$PIDFILE")" 2>/dev/null || true; rm -f "$PIDFILE"; echo stopped; fi
    ;;
  status)
    if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
      echo "running pid $(cat "$PIDFILE")"; curl -s --unix-socket "$SOCK" http://broker/v1/health; echo
    else echo "not running"; fi
    ;;
  logs)  tail -n "${2:-50}" "$LOG" ;;
  audit) cat "$L0_ROOT"/audit/broker-*.jsonl 2>/dev/null | tail -n "${2:-50}" ;;
  call)  curl -s --unix-socket "$SOCK" -H 'content-type: application/json' -d "${2:?json body}" http://broker/v1/call; echo ;;
  *) echo "usage: $0 {build|test|start|stop|status|logs|audit|call}" >&2; exit 2 ;;
esac
