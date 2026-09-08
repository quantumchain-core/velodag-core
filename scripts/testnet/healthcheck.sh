#!/usr/bin/env bash
set -euo pipefail

NODE_DIR="${1:-${VDAG_NODE_DIR:-$HOME/velodag-test/node}}"
PID_FILE="${VDAG_PID_FILE:-$NODE_DIR/node.pid}"
LOG_FILE="${VDAG_LOG_FILE:-$NODE_DIR/node.log}"

if [ ! -f "$PID_FILE" ]; then
  echo "NODE_DOWN: no PID file at $PID_FILE"
  exit 2
fi

PID="$(cat "$PID_FILE" 2>/dev/null || true)"
if [ -z "${PID}" ] || ! kill -0 "$PID" 2>/dev/null; then
  echo "NODE_DOWN: PID $PID is not running"
  exit 3
fi

if [ -f "$LOG_FILE" ]; then
  echo "NODE_UP: PID=$PID"
  tail -n 20 "$LOG_FILE"
else
  echo "NODE_UP: PID=$PID but no log file found yet"
fi
