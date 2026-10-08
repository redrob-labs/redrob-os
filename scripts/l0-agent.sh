#!/usr/bin/env bash
# L0: run the agent directly on the x86 dev host, no container, no systemd.
#   scripts/l0-agent.sh build      # cargo release build (x86_64)
#   scripts/l0-agent.sh init       # render deploy/config/agent.toml into $L0_ROOT
#   scripts/l0-agent.sh validate   # config validate
#   scripts/l0-agent.sh start|stop|status|logs
# State lives under $L0_ROOT (default ~/.local/state/redrob-agent).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
. "$ROOT/scripts/dev-env.sh"
L0_ROOT="${L0_ROOT:-$HOME/.local/state/redrob-agent}"
BIN="$ROOT/agent/target/release/zeroclaw"
PIDFILE="$L0_ROOT/daemon.pid"
LOG="$L0_ROOT/logs/daemon.log"

export ZEROCLAW_CONFIG_DIR="$L0_ROOT"
export ZEROCLAW_DATA_DIR="$L0_ROOT/data"
export ZEROCLAW_WORKSPACE="$L0_ROOT/workspace"
export RUST_LOG="${RUST_LOG:-info}"

cmd="${1:-status}"
case "$cmd" in
  build)
    (cd "$ROOT/agent" && cargo build --release --bin zeroclaw --features channel-slack)
    ;;
  init)
    mkdir -p "$L0_ROOT/workspace" "$L0_ROOT/data" "$L0_ROOT/logs"
    # L0 has no Podman/gVisor on this host: fall back to the native runtime with
    # the in-process sandbox (landlock/bwrap/firejail by auto-detection).
    sed -e "s|@@DATA@@|$L0_ROOT|g" \
        -e 's|^kind = "docker"|kind = "native"|' \
        "$ROOT/deploy/config/agent.toml" > "$L0_ROOT/config.toml"
    echo "rendered $L0_ROOT/config.toml"
    ;;
  validate)
    "$BIN" doctor
    ;;
  start)
    [ -x "$BIN" ] || { echo "no binary, run: $0 build" >&2; exit 1; }
    [ -f "$L0_ROOT/config.toml" ] || { echo "no config, run: $0 init" >&2; exit 1; }
    if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
      echo "already running pid $(cat "$PIDFILE")"; exit 0
    fi
    mkdir -p "$L0_ROOT/logs"
    setsid nohup "$BIN" daemon >>"$LOG" 2>&1 &
    echo $! > "$PIDFILE"
    echo "started pid $! log $LOG"
    ;;
  stop)
    if [ -f "$PIDFILE" ]; then
      kill "$(cat "$PIDFILE")" 2>/dev/null || true
      rm -f "$PIDFILE"; echo stopped
    fi
    ;;
  status)
    if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
      echo "running pid $(cat "$PIDFILE")"
    else
      echo "not running"
    fi
    ;;
  logs)
    tail -n "${2:-50}" "$LOG"
    ;;
  *)
    echo "usage: $0 {build|init|validate|start|stop|status|logs}" >&2; exit 2
    ;;
esac
