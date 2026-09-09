#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/../.." && pwd)"

NODE_DIR="${1:-${VDAG_NODE_DIR:-$HOME/velodag-test/node}}"
BINARY="${VDAG_BINARY:-$REPO_ROOT/target/release/vdag-node}"
LOG_FILE="${VDAG_LOG_FILE:-$NODE_DIR/node.log}"
PID_FILE="${VDAG_PID_FILE:-$NODE_DIR/node.pid}"
BOOTSTRAP_FILE="${VDAG_BOOTSTRAP_FILE:-$NODE_DIR/bootstrap_peers.txt}"
VDAG_NETWORK_VALUE="${VDAG_NETWORK:-devnet}"
BOOTSTRAP_CONFIG_PATH="${VDAG_BOOTSTRAP_CONFIG:-$REPO_ROOT/bootstrap.${VDAG_NETWORK_VALUE}.json}"
RUST_LOG_VALUE="${RUST_LOG:-info}"

mkdir -p "$NODE_DIR"

if [ ! -x "$BINARY" ]; then
  echo "ERROR: node binary not found or not executable: $BINARY" >&2
  echo "Build it first with: cargo build --release" >&2
  exit 1
fi

if [ -f "$PID_FILE" ]; then
  PID="$(cat "$PID_FILE" 2>/dev/null || true)"
  if [ -n "${PID}" ] && kill -0 "$PID" 2>/dev/null; then
    echo "Node is already running with PID $PID"
    exit 0
  fi
fi

if [ ! -x "$NODE_DIR/vdag-node" ] || ! cmp -s "$BINARY" "$NODE_DIR/vdag-node" 2>/dev/null; then
  cp "$BINARY" "$NODE_DIR/vdag-node"
fi

chmod 700 "$NODE_DIR"
chmod 600 "$NODE_DIR/node_identity.key" 2>/dev/null || true

if [ -f "$BOOTSTRAP_FILE" ]; then
  chmod 600 "$BOOTSTRAP_FILE" 2>/dev/null || true
fi

cd "$NODE_DIR"
export VDAG_NETWORK="$VDAG_NETWORK_VALUE"
export VDAG_BOOTSTRAP_CONFIG="$BOOTSTRAP_CONFIG_PATH"
nohup env RUST_LOG="$RUST_LOG_VALUE" VDAG_NETWORK="$VDAG_NETWORK_VALUE" VDAG_BOOTSTRAP_CONFIG="$BOOTSTRAP_CONFIG_PATH" ./vdag-node > "$LOG_FILE" 2>&1 &
NEW_PID=$!
echo "$NEW_PID" > "$PID_FILE"

sleep 1
printf 'Started node PID=%s in %s\n' "$NEW_PID" "$NODE_DIR"
if [ -f "$LOG_FILE" ]; then
  tail -n 20 "$LOG_FILE"
fi
