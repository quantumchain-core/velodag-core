#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/../.." && pwd)"

BASE_DIR="${1:-$HOME/velodag-test}"
NODE_A_DIR="$BASE_DIR/node-a"
NODE_B_DIR="$BASE_DIR/node-b"
BINARY="${VDAG_BINARY:-$REPO_ROOT/target/release/vdag-node}"
VDAG_NETWORK_VALUE="${VDAG_NETWORK:-devnet}"

strip_ansi() {
  python3 - "$1" <<'PY'
import re, sys
path = sys.argv[1]
with open(path, 'r', encoding='utf-8', errors='replace') as f:
    text = f.read()
print(re.sub(r'\x1b\[[0-9;]*[A-Za-z]', '', text), end='')
PY
}

mkdir -p "$BASE_DIR"

if [ ! -x "$BINARY" ]; then
  echo "ERROR: release binary not found: $BINARY" >&2
  echo "Build it first with: cargo build --release" >&2
  exit 1
fi

if command -v pgrep >/dev/null 2>&1; then
  pgrep -af 'vdag-node' | awk '{print $1}' | xargs -r kill -TERM 2>/dev/null || true
fi
rm -rf "$NODE_A_DIR" "$NODE_B_DIR"
mkdir -p "$NODE_A_DIR" "$NODE_B_DIR"
cp "$BINARY" "$NODE_A_DIR/vdag-node"
cp "$BINARY" "$NODE_B_DIR/vdag-node"

echo "[1/2] Starting seed node in $NODE_A_DIR"
VDAG_NETWORK="$VDAG_NETWORK_VALUE" VDAG_BOOTSTRAP_CONFIG="$REPO_ROOT/bootstrap.${VDAG_NETWORK_VALUE}.json" ./scripts/testnet/start-node.sh "$NODE_A_DIR"

for _ in $(seq 1 30); do
  if [ -f "$NODE_A_DIR/node.log" ]; then
    CLEAN_LOG=$(strip_ansi "$NODE_A_DIR/node.log")
    if printf '%s' "$CLEAN_LOG" | grep -q 'Local P2P Node Peer ID' && \
       printf '%s' "$CLEAN_LOG" | grep -q 'Local node listening address=/ip4/127.0.0.1/tcp/'; then
      break
    fi
  fi
  sleep 1
done

CLEAN_LOG=$(strip_ansi "$NODE_A_DIR/node.log")
PEER_ID=$(printf '%s' "$CLEAN_LOG" | grep -o 'local_peer_id=[^ ]*' | head -n 1 | cut -d= -f2 || true)
LISTEN_ADDR=$(printf '%s' "$CLEAN_LOG" | grep -oE '/ip4/127\.0\.0\.1/tcp/[0-9]+' | head -n 1 || true)

if [ -z "$PEER_ID" ] || [ -z "$LISTEN_ADDR" ]; then
  echo "ERROR: could not determine seed node Peer ID or listen port from $NODE_A_DIR/node.log" >&2
  echo "--- last log lines ---" >&2
  tail -n 40 "$NODE_A_DIR/node.log" >&2 || true
  exit 1
fi

BOOTSTRAP_ADDR="${LISTEN_ADDR}/p2p/${PEER_ID}"
printf '%s\n' "$BOOTSTRAP_ADDR" > "$NODE_B_DIR/bootstrap_peers.txt"
chmod 600 "$NODE_B_DIR/bootstrap_peers.txt" 2>/dev/null || true

echo "Seed peer: $BOOTSTRAP_ADDR"

echo "[2/2] Starting second node in $NODE_B_DIR"
VDAG_NETWORK="$VDAG_NETWORK_VALUE" VDAG_BOOTSTRAP_CONFIG="$REPO_ROOT/bootstrap.${VDAG_NETWORK_VALUE}.json" ./scripts/testnet/start-node.sh "$NODE_B_DIR"

echo "--- node-a status ---"
./scripts/testnet/healthcheck.sh "$NODE_A_DIR" | tail -n 20

echo "--- node-b status ---"
./scripts/testnet/healthcheck.sh "$NODE_B_DIR" | tail -n 20
