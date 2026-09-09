# Public Testnet Operational Hardening

This repository is now code-complete for a local/devnet proof and has the final operational layer needed for a public testnet rollout.

## What is already in place

- Rust workspace builds cleanly with `cargo check --workspace` and `cargo test --workspace`
- The node persists its libp2p identity in `node_identity.key` so Peer IDs stay stable
- Bootstrap peers can be supplied from `bootstrap_peers.txt`
- Signed environment bootstrap files such as `bootstrap.devnet.json` and `bootstrap.testnet.json` are supported
- Gossipsub mesh settings are valid and no longer fail on startup
- Sync gating prevents local mining while the node is still catching up
- Difficulty targets are committed into each block header and validated consistently
- The repo ignores runtime and secret artifacts such as `node_identity.key`, `velodag_ledger_data/`, `*.log`, and `*.pid`
- The runbook documents how to build, run, reconnect, and stop nodes

## Final operations layer for a public testnet

### 1. Identity and secret controls

- Keep `node_identity.key` outside the repository and restrict access with `chmod 600`
- Treat the file as a credential equivalent to a server private key
- Rotate it if a node is rebuilt or moved between environments
- Store ledger backups encrypted at rest and never commit them to Git

### 2. Bootstrap policy

- Use a fixed set of bootstrap nodes with stable IPs and maintained peer IDs
- Store bootstrap metadata in signed JSON files like `bootstrap.devnet.json` and `bootstrap.testnet.json`
- Set `VDAG_NETWORK=devnet|testnet|mainnet` to select the correct signed bootstrap set for the node
- Do not allow arbitrary dial-in peers without a allowlist or operator review
- Prefer a curated seed list for the first public testnet wave
- Require peer address validation before adding a new seed or relay node

### 3. Public ingress and firewall posture

- Only expose the node's TCP port through a controlled tunnel or firewall rule
- Keep the node on a dedicated host or VM with restricted egress and minimal service exposure
- Use a reverse tunnel only for testnet access; do not expose the node without validating your network policy
- Log every inbound dial and store a recent peer-address allowlist

### 4. Lifecycle automation

- Do not run nodes ad hoc in a production testnet environment
- Use a supervisor such as `systemd`, `tmux`, or a small container orchestrator
- Maintain PID files and log rotation to prevent stale or unmonitored processes
- Use the supplied scripts under `scripts/testnet/` for standard start/health/stop operations

### 5. Monitoring, alerting, and evidence

- Capture node startup logs, peer connection events, block acceptance, and sync lag
- Alert when a seed node is down, the sync queue stays pending too long, or the node is isolated from the mesh
- Track whether a peer receives blocks consistently after the bootstrap phase

## Runtime helper scripts

The repo now includes these operational helpers:

Run them from the repository root so the scripts can resolve the local binary automatically:

```bash
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
export VDAG_BINARY="$REPO_ROOT/target/release/vdag-node"
chmod +x scripts/testnet/*.sh
```

Then execute:

- `scripts/testnet/start-node.sh` — launch a node with the correct runtime directory and PID/log setup
- `scripts/testnet/healthcheck.sh` — verify the node is alive and print the last log lines
- `scripts/testnet/stop-node.sh` — shutdown a managed node cleanly

Example usage:

```bash
./scripts/testnet/start-node.sh "$HOME/velodag-test/node-a"
./scripts/testnet/healthcheck.sh "$HOME/velodag-test/node-a"
./scripts/testnet/stop-node.sh "$HOME/velodag-test/node-a"
```

## Minimum pre-launch checklist for a real public testnet

- [ ] Build and verify the release binary
- [ ] Generate a dedicated identity key for each operator-owned node
- [ ] Configure a stable bootstrap peer list
- [ ] Restrict network exposure to the required ports only
- [ ] Run health checks and log monitoring
- [ ] Publish a clear seed-node list and operator contact channel
- [ ] Confirm node sync and block validation on a multi-node test cluster before opening to public peers

## Bottom line

The protocol and node logic are now in a stable state for a testnet. The remaining work is not protocol correctness; it is operational discipline: identity protection, public ingress control, durable bootstrap policy, monitoring, and process supervision.
