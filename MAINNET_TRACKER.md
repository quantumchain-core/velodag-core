# VeloDAG Mainnet Readiness Tracker

Updated: 2026-09-08

This tracker describes the current repository state. A feature is marked done only when it exists in code and has a focused verification path. Passing a build does not make the network mainnet-ready.

## Current status

**Stage:** local devnet / early testnet foundation

**Mainnet decision:** not ready for mainnet launch

**Latest verified release build:** `cargo build --release`

## Done

### Build and code quality

- [x] Rust workspace builds in release mode.
- [x] Workspace tests pass.
- [x] Strict workspace clippy passes with warnings denied.
- [x] Runtime artifacts and node identity secrets are ignored by Git.

### Cryptography and transaction integrity

- [x] Dilithium2 key generation and signature verification exist.
- [x] Transaction public keys are carried with transactions.
- [x] Sender address ownership is checked against the transaction public key.
- [x] Peer nodes reject invalid transaction signatures.
- [x] Transaction contents are committed through a deterministic Merkle root.
- [x] Peer nodes reject transaction commitment mismatches.

### Ledger foundation

- [x] Account balances exist in consensus state.
- [x] Block rewards are credited to explicit miner and treasury addresses.
- [x] Transfers reject insufficient balances.
- [x] Duplicate confirmed transactions are rejected.
- [x] Ledger snapshots are persisted in sled.
- [x] Nodes restore ledger state, block height, tip, and identity after restart.

### Transaction intake

- [x] Loopback JSON-RPC server accepts newline-delimited requests.
- [x] `submit_transaction` validates signed transactions before mempool insertion.
- [x] `mempool_size` exposes the current pending transaction count.
- [x] Live RPC intake has been tested against the release binary.

### Wallet tooling

- [x] Wallet creation and address derivation CLI commands exist.
- [x] Wallet CLI signs the canonical transfer payload.
- [x] Wallet CLI can submit signed transfers to the local RPC.
- [x] Wallet key material is encrypted with a password-derived key.
- [x] Wrong wallet passwords are rejected by authenticated decryption.

### Local networking and operations

- [x] Noise and Yamux encrypted libp2p transport is configured.
- [x] Gossipsub block propagation and request/response sync work locally.
- [x] Two-node local launch and catch-up sync have been verified.
- [x] Startup waits briefly for peer discovery and uses the shared genesis parent.
- [x] Start, stop, health-check, and two-node launch scripts exist.
- [x] Basic node runbook and testnet hardening documentation exist.

## In progress / required before public testnet

### Protocol correctness

- [ ] Replace the all-zero runtime genesis identifier with one published immutable genesis block and hash.
- [ ] Define network IDs so devnet, testnet, and mainnet cannot connect accidentally.
- [ ] Validate block height continuity and parent relationships.
- [ ] Validate timestamps and reject future or manipulated timestamps.
- [ ] Complete and formally specify GHOSTDAG coloring, ordering, fork choice, and finality.
- [ ] Harden difficulty adjustment with bounds, timestamp rules, and adversarial tests.
- [ ] Define deterministic behavior for competing blocks at the same height.
- [ ] Define transaction fees and miner fee accounting.

### State and transaction model

- [ ] Replace the local-only RPC with a versioned, authenticated production API.
- [ ] Add transaction nonces or another replay-protection mechanism.
- [ ] Define whether the ledger is account-based or UTXO-based and freeze the specification.
- [ ] Persist and validate state transitions atomically with block commits.
- [ ] Add state migration/versioning for database upgrades.
- [ ] Reconcile alternate DAG branches instead of applying every accepted block to one linear balance state.
- [x] Add wallet transaction creation CLI.
- [x] Add encrypted key storage.
- [ ] Add secure backup/recovery procedures and hardware-wallet support.

### Networking security

- [ ] Add stable public seed nodes and signed bootstrap configuration.
- [ ] Add peer connection limits and inbound/outbound rate limits.
- [ ] Add message and sync request abuse protection.
- [ ] Add peer banning, scoring policy, and recovery behavior.
- [ ] Add NAT traversal or relay support for nodes behind private networks.
- [ ] Test partitions, delayed peers, duplicate messages, malformed payloads, and long sync gaps.

### Clients and interfaces

- [ ] Implement a stable versioned RPC/API.
- [ ] Add transaction status and confirmation queries.
- [ ] Add a wallet client compatible with the final transaction format.
- [ ] Build a block explorer from validated persisted state.
- [ ] Document address, denomination, fee, confirmation, and finality rules.

## Mainnet launch gates

All of the following must be complete before a mainnet announcement:

- [ ] Protocol specification is frozen and versioned.
- [ ] Genesis block, network ID, treasury address, and monetary units are published.
- [ ] Transaction and state model is implemented, documented, and independently reviewed.
- [ ] Consensus implementation has deterministic test vectors and adversarial tests.
- [ ] Public testnet has run for an agreed soak period with multiple independent operators.
- [ ] Restart, backup, restore, migration, partition, and reorganization tests pass.
- [ ] Independent cryptographic, consensus, and networking audits are complete.
- [ ] Reproducible signed release binaries are available.
- [ ] Seed nodes, monitoring, alerting, firewall rules, and incident response are operational.
- [ ] Upgrade and rollback procedures have been rehearsed.
- [ ] Wallet, RPC, explorer, and operator documentation are published.
- [ ] White paper claims match implemented functionality.

## Known documentation gap

[WHITE-PAPER.md](WHITE-PAPER.md) describes zero-knowledge privacy, viewing keys, and a production state model that are not implemented yet. Those sections must be labelled as planned architecture or removed until the corresponding code exists.

[ROADMAP.md](ROADMAP.md) also labels mock transaction generation and several consensus components as completed even though the current implementation is still a testnet foundation. This tracker is the more conservative source for launch status.

## Immediate next work

1. Freeze and implement the genesis/network identity specification.
2. Add a wallet-compatible transaction creation and signing flow.
3. Make state transitions atomic and DAG-aware.
4. Add transaction status, confirmation queries, and RPC authentication.
5. Add adversarial consensus and network tests.
6. Run a long-lived public testnet before any mainnet decision.
