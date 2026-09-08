#!/usr/bin/env bash
set -euo pipefail

NODE_DIR="${1:-${VDAG_NODE_DIR:-$HOME/velodag-test/node}}"
PID_FILE="${VDAG_PID_FILE:-$NODE_DIR/node.pid}"

if [ -f "$PID_FILE" ]; then
  PID="$(cat "$PID_FILE" 2>/dev/null || true)"
  if [ -n "${PID}" ] && kill -0 "$PID" 2>/dev/null; then
    kill "$PID"
    echo "Stopped node PID=$PID"
  fi
fi

if command -v pgrep >/dev/null 2>&1; then
  PIDS=$(pgrep -f "$NODE_DIR/vdag-node" || true)
  if [ -n "$PIDS" ]; then
    echo "$PIDS" | xargs -r kill
    echo "Stopped testnet node processes from $NODE_DIR"
  fi
fi

rm -f "$PID_FILE"
