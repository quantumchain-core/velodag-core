# Running a VeloDAG Node

This guide describes how to build, run, stop, reconnect, and back up a VeloDAG node.

## Requirements

From the repository root, verify Rust and the workspace:

```bash
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
echo "$REPO_ROOT"
```

Then:

```bash
cargo --version
cargo check --workspace
```

Build the optimized node binary:

```bash
cargo build --release
```

The binary is created at:

```text
target/release/vdag-node
```

On another machine, this path works as long as you are in the repo root or have exported the repo path as `REPO_ROOT` for the following examples.

## Node Files

Run each node from its own data directory. The node writes these files relative to its current working directory:

| File or directory | Purpose | Private? | Back up? |
| --- | --- | --- | --- |
| `node_identity.key` | Private libp2p identity key. Keeps the node's Peer ID stable across restarts. | **Yes** | **Yes, securely** |
| `velodag_ledger_data/` | Local sled ledger and GHOSTDAG data. | Treat as private | Yes, if preserving local state |
| `bootstrap_peers.txt` | Optional list of peer multiaddresses. | Usually no, but it reveals network topology | Optional |
| `node.log` | Operational log. | May contain peer IDs, addresses, and block data | Optional |

Protect the identity key after the first start:

```bash
chmod 600 node_identity.key
```

Never commit `node_identity.key`, ledger data, or logs to a public repository. If the key is lost, the node gets a new Peer ID and old dial addresses that contain the previous Peer ID no longer work.

A Peer ID and listening multiaddress are public connection information. The private key behind the Peer ID is not public.

## Run One Node

Create a dedicated directory and copy the release binary:

```bash
mkdir -p ~/velodag-test/node-a
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || echo "$PWD")"
cp "$REPO_ROOT/target/release/vdag-node" ~/velodag-test/node-a/
cd ~/velodag-test/node-a
RUST_LOG=info ./vdag-node
```

The node prints its Peer ID and one or more listening addresses, for example:

```text
Local P2P Node Peer ID local_peer_id=12D3...
Local node listening address=/ip4/127.0.0.1/tcp/36537
```

The TCP port is randomized at startup. Save the complete address that includes:

```text
/ip4/<reachable-ip>/tcp/<port>/p2p/<peer-id>
```

The node mines locally about once per second. Stop a foreground node with `Ctrl-C`.

## Submit Transactions through Local RPC

The node exposes a newline-delimited JSON-RPC server on `127.0.0.1:8545` by
default. Override the bind address with `VDAG_RPC_ADDR`, for example:

```bash
VDAG_RPC_ADDR=127.0.0.1:18545 RUST_LOG=info ./vdag-node
```

The RPC listener is loopback-only by default. Do not bind it to `0.0.0.0` or
forward it through a public tunnel without adding authentication and transport
security.

Submit a signed transaction with one JSON request per line:

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"submit_transaction","params":{"sender":"<64-hex-address>","recipient":"<64-hex-address>","amount":1,"public_key":"<public-key-hex>","signature":"<signature-hex>"}}' | nc 127.0.0.1 8545
```

The node verifies the public-key ownership, signature, positive amount, and
duplicate transaction ID before placing the transaction in the mempool. The
miner includes accepted transactions in a later block if the sender has enough
ledger balance.

Check the current pending transaction count:

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"mempool_size"}' | nc 127.0.0.1 8545
```

This is the first local transaction intake interface. A public deployment still
needs authenticated remote RPC, fees, replay protection, wallet tooling, and
rate limits.

### Wallet CLI

The release binary can create a local Dilithium wallet and sign the exact
transaction format accepted by the RPC:

```bash
./target/release/vdag-node wallet create "$HOME/vdag-wallet.json"
./target/release/vdag-node wallet address "$HOME/vdag-wallet.json"
./target/release/vdag-node wallet sign-transfer \
  "$HOME/vdag-wallet.json" \
  0x0202020202020202020202020202020202020202020202020202020202020202 1
```

To sign and submit in one command to a local node:

```bash
./target/release/vdag-node wallet submit \
  "$HOME/vdag-wallet.json" \
  0x0202020202020202020202020202020202020202020202020202020202020202 1 \
  127.0.0.1:8545
```

Wallet files contain private Dilithium key material. Protect them with file
permissions and encrypted backups; this CLI does not yet provide password-based
encryption or hardware-wallet protection.

## Public Access with ngrok

This Codespace runs Alpine Linux, so do not use the Debian `apt` ngrok
installation commands. Install the official ngrok binary in your user account:

```bash
mkdir -p "$HOME/.local/bin"
curl -sSL https://bin.equinox.io/c/bNyj1mQVY4c/ngrok-v3-stable-linux-amd64.tgz \
  | tar -xz -C "$HOME/.local/bin"
export PATH="$HOME/.local/bin:$PATH"
ngrok version
```

Configure your token directly in the terminal. Never put the token in a
repository, log file, issue, or chat message:

```bash
ngrok config add-authtoken '<PASTE_YOUR_NGROK_TOKEN_HERE>'
ngrok config check
```

If a token has already been exposed, revoke it in the ngrok dashboard and
create a new one before configuring the client.

The VeloDAG node chooses a random TCP port. Start the node first, then read
the port from its log and start a TCP tunnel:

```bash
cd ~/velodag-test/node-a
RUST_LOG=info nohup ./vdag-node > node-a.log 2>&1 &
echo $! > node.pid
sleep 3

NODE_PORT=$(grep -oE 'tcp/[0-9]+' node-a.log | head -n 1 | cut -d/ -f2)
echo "Node port: $NODE_PORT"

nohup ngrok tcp "$NODE_PORT" > ngrok.log 2>&1 &
echo $! > ngrok.pid
sleep 3
cat ngrok.log
```

Find the public endpoint in `ngrok.log`. It will look like:

```text
tcp://0.tcp.ngrok.io:12345
```

The remote VeloDAG peer must dial the node's libp2p multiaddress using the
ngrok hostname and port, together with the node Peer ID:

```text
/dns4/0.tcp.ngrok.io/tcp/12345/p2p/<NODE_PEER_ID>
```

Use the actual hostname and port printed by ngrok. Keep both the VeloDAG node
and ngrok process running. The public ngrok address may change when the tunnel
restarts, so share the new address with peers after every reconnect.

Stop the tunnel and node separately:

```bash
kill "$(cat ~/velodag-test/node-a/ngrok.pid)" 2>/dev/null || true
kill "$(cat ~/velodag-test/node-a/node.pid)" 2>/dev/null || true
```

If you use a different directory, adjust the paths accordingly. A TCP tunnel
exposes the node's network port publicly; use it only for testing and do not
expose a node containing sensitive or valuable data without additional network
controls.

## Connect Another Node

Start node-a first and copy its reachable listening address. Then use a separate directory for node-b:

```bash
mkdir -p ~/velodag-test/node-b
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || echo "$PWD")"
cp "$REPO_ROOT/target/release/vdag-node" ~/velodag-test/node-b/
cd ~/velodag-test/node-b
RUST_LOG=info ./vdag-node --dial /ip4/127.0.0.1/tcp/36537/p2p/12D3KooW...
```

Replace the example port and Peer ID with node-a's actual values. For another host, replace `127.0.0.1` with an address reachable by node-b and ensure the TCP port is allowed through the host firewall or container networking.

The node automatically requests catch-up blocks after connecting. It pauses local mining while initial sync is pending, validates the received blocks, then resumes mining.

You may provide more than one dial target:

```bash
RUST_LOG=info ./vdag-node \
  --dial /ip4/10.0.0.10/tcp/36537/p2p/12D3KooW... \
  --dial /ip4/10.0.0.11/tcp/40123/p2p/12D3KooX...
```

## Reconnect Automatically

Create `bootstrap_peers.txt` in the node's data directory. Put one complete multiaddress on each line:

```text
# Comments and blank lines are ignored
/ip4/10.0.0.10/tcp/36537/p2p/12D3KooW...
/ip4/10.0.0.11/tcp/40123/p2p/12D3KooX...
```

Then start the node normally:

```bash
cd ~/velodag-test/node-b
RUST_LOG=info ./vdag-node
```

The node reads this file at startup and attempts to dial every listed address. Because the node identity is persisted, the node's own Peer ID remains stable. The peer's port can still change if that peer restarts, so update the address when its port changes.

## Keep a Node Running

### Background process with a PID file

```bash
cd ~/velodag-test/node-a
chmod 600 node_identity.key 2>/dev/null || true
nohup env RUST_LOG=info ./vdag-node > node.log 2>&1 &
echo $! > node.pid
```

Check that it is running:

```bash
kill -0 "$(cat node.pid)" && echo "node is running"
tail -f node.log
```

### Recommended: tmux

If `tmux` is installed, it keeps the process attached to a reusable terminal session:

```bash
cd ~/velodag-test/node-a
tmux new -s vdag-a
RUST_LOG=info ./vdag-node
```

Detach with `Ctrl-B`, then `D`. Reattach later:

```bash
tmux attach -t vdag-a
```

## Stop a Node

For a foreground node, press:

```text
Ctrl-C
```

For a background node started with `node.pid`, request a normal shutdown:

```bash
kill "$(cat ~/velodag-test/node-a/node.pid)"
```

If it does not stop, identify the process before using a stronger signal:

```bash
ps -fp "$(cat ~/velodag-test/node-a/node.pid)"
kill -TERM "$(cat ~/velodag-test/node-a/node.pid)"
```

Use `kill -KILL` only as a last resort. A normal stop is preferred so the process can finish its current storage operation.

For a bounded test run, use `timeout`:

```bash
cd ~/velodag-test/node-a
RUST_LOG=info timeout 20 ./vdag-node
```

Exit code `124` means `timeout` stopped the node after 20 seconds; it does not mean the node crashed.

## Logs and Health Checks

Look for these messages:

```text
Local P2P Node Peer ID
Local node listening address
Connection established
Received sync catch-up blocks
Accepted block
[⏱️ Block Mined Locally]
```

Useful commands:

```bash
grep -E 'Connection established|Received sync catch-up blocks|Accepted block|Block Mined Locally|Rejected block|Sync request failed' node.log

tail -n 50 node.log
```

A healthy joining node should show connection and sync messages, accepted catch-up blocks, and then local mining. Repeated `insufficient PoW` or `Sync request failed` messages indicate a peer, data, or network problem.

## Restarting and Recovery

To restart the same node with the same identity and local ledger:

```bash
cd ~/velodag-test/node-a
RUST_LOG=info ./vdag-node
```

Do not delete `node_identity.key` if other nodes use its Peer ID. Do not delete `velodag_ledger_data/` unless you intentionally want a fresh local ledger.

To make a fresh node with a new identity, create a new empty directory and copy only the binary:

```bash
mkdir -p ~/velodag-test/node-new
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || echo "$PWD")"
cp "$REPO_ROOT/target/release/vdag-node" ~/velodag-test/node-new/
cd ~/velodag-test/node-new
RUST_LOG=info ./vdag-node
```

It will generate a new private identity key and a new local ledger. It must be connected to a peer with `--dial` or `bootstrap_peers.txt` to catch up.

## Backups and Secrets

Back up these items securely:

```text
node_identity.key
velodag_ledger_data/
bootstrap_peers.txt  (optional)
```

The most sensitive item is `node_identity.key`. Anyone who obtains it can run the node as that Peer ID. Do not paste its contents into chat, issues, logs, or commits. Store backups with restricted permissions and encrypt them at rest.

If the key is exposed, stop the node, remove the compromised identity from trusted infrastructure, create a new data directory, and start with a newly generated identity.

## Explorer Mode

The node also has a one-shot block explorer mode. Pass a 64-character block hash after `--get-block`:

```bash
cd ~/velodag-test/node-a
./vdag-node --get-block 0000000000000000000000000000000000000000000000000000000000000000
```

Do not run explorer mode against the same live data directory at the same time as a node process.
