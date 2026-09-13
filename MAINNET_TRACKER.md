# VeloDAG Mainnet Readiness Tracker

Updated: 2026-09-14

This tracker describes the current repository state. A feature is marked done only when it exists in code, is covered by a test that runs in CI, and (where the risk warrants it) has been verified at runtime, not just at compile time. Passing a build does not make the network mainnet-ready.

**Companion document:** [VeloDAG_Master_Tracker.md] carries the same information in more granular, round-by-round form (what changed, why, which test proves it). This file is the canonical summary; that one is the detailed working log. If they ever disagree, this file wins and the other should be corrected to match.

## Current status

**Stage:** local devnet / early testnet foundation — protocol correctness has advanced substantially since the previous update, but this is still pre-testnet.

**Mainnet decision:** not ready for mainnet launch. Not ready for public testnet either — see "Required before public testnet" below, which is now short and specific rather than open-ended.

**Latest verified state:** 41/41 tests passing in CI (`cargo test --workspace`), across a full review pass that found and fixed several genuine consensus-correctness bugs (see "What changed since the last update" below).

## What changed since the last update (2026-09-08 → 2026-09-14)

A full manual code review found two critical, silently-corrupting bugs that had escaped local testing, plus completed most of the "in progress" items from the previous version of this tracker. In order fixed:

1. **Difficulty adjustment overflow** — a u128 multiplication could silently overflow in release builds, corrupting the difficulty target without crashing or erroring. Fixed with clamped arithmetic and checked operations; regression test added.
2. **Self-declared PoW target** — blocks were validated against whatever difficulty target they themselves claimed, not an independently-derived expected value. This meant a peer could gossip a block claiming a trivially easy target and have it accepted. Fixed: validation now independently recomputes the expected target.
3. **Genesis-per-network** — devnet/testnet/mainnet previously shared one genesis hash regardless of which was selected; `VDAG_NETWORK` didn't actually propagate through the P2P handshake. Fixed, each network now has its own genesis timestamp/hash, and network ID is checked in the handshake.
4. **Block validation hardening** — height continuity (`height == max(parent heights) + 1`), timestamp/future-drift limits (30s), gossip message size cap (2MB), transaction count cap (5,000/block).
5. **Real GHOSTDAG fork choice** — blue/red coloring existed but nothing used it to select a canonical tip; mining just built on whichever block was processed most recently. Fixed: `select_canonical_tip` picks the highest-blue_score tip, deterministic tie-break by lowest hash.
6. **Canonical ledger/difficulty ordering** — ledger balances and difficulty adjustment were derived from arrival order, not the canonical GHOSTDAG-linearized order. Fixed: both now recompute from `get_linear_sort` after every accepted block.
7. **Transaction fees + burn** — flat 100-base-unit fee per transaction, half burned, half distributed via the existing 95/5 miner/treasury split.
8. **Deterministic consensus test vectors** — 6 vectors checked against independently-computed values (Python/hashlib, not the Rust code under test), covering genesis hashes, block hashing, tx/merkle hashing, subsidy halving, fee distribution, and the difficulty-overflow scenario specifically.
9. **Per-transaction mempool affordability filter** — one unaffordable transaction in a batch no longer causes every other valid transaction in that batch to be silently dropped.
10. **RPC hardening** — `get_balance`, `transaction_status`, `version` methods added; opt-in token authentication (`VDAG_RPC_TOKEN`); 64KB request size cap, 50 req/sec per-connection rate limit, 256 max concurrent connections.
11. **Peer-level hardening** — per-peer (2) and global (128) connection limits, malformed-gossip violation tracking, auto-ban at 5 violations.
12. **Adversarial/property-based testing** — found and fixed a real bug: `calculate_subsidy_split` used an unguarded bit-shift that silently produces a wrong nonzero value (not a panic) once era ≥ 64, the same failure class as the earlier difficulty-overflow bug. 5 `proptest` properties added.

## Done

### Build and code quality

- [x] Rust workspace builds in release mode.
- [x] Workspace tests pass (36/36 as of this update).
- [x] Strict workspace clippy passes with warnings denied.
- [x] Runtime artifacts and node identity secrets are ignored by Git.

### Cryptography and transaction integrity

- [x] Dilithium2 key generation and signature verification exist.
- [x] Transaction public keys are carried with transactions.
- [x] Sender address ownership is checked against the transaction public key.
- [x] Peer nodes reject invalid transaction signatures.
- [x] Transaction contents are committed through a deterministic Merkle root.
- [x] Peer nodes reject transaction commitment mismatches.
- [x] Genesis hash and block hashing verified against independently-computed test vectors (not just self-consistency checks).

### Consensus correctness

- [x] Genesis is fixed and network-specific (devnet/testnet/mainnet each have distinct genesis hashes).
- [x] Network ID is checked in the P2P sync handshake, not just genesis hash.
- [x] Block height continuity is validated (must equal max(parent heights) + 1).
- [x] Block timestamps are validated against future-drift and parent-timestamp-precedence rules.
- [x] Difficulty adjustment uses clamped, overflow-safe arithmetic (regression-tested against the exact scenario that previously broke it).
- [x] PoW is validated against an independently-derived expected target, not a block's own declared target.
- [x] GHOSTDAG fork choice is implemented and deterministic (highest blue_score wins, ties broken by lowest hash).
- [x] Ledger state and difficulty adjustment are derived from canonical GHOSTDAG order (`get_linear_sort`), not arrival order.
- [x] Transaction fees are implemented (flat fee, partial burn, remainder via existing 95/5 split).

### Ledger foundation

- [x] Account balances exist in consensus state.
- [x] Block rewards are credited to explicit miner and treasury addresses.
- [x] Transfers reject insufficient balances (now inclusive of the transaction fee).
- [x] Duplicate confirmed transactions are rejected.
- [x] Ledger snapshots are persisted in sled.
- [x] Nodes restore ledger state, block height, tip, and identity after restart.
- [x] Per-transaction mempool affordability filtering (one bad transaction no longer drops an entire batch).

### Transaction intake and RPC

- [x] Loopback JSON-RPC server accepts newline-delimited requests.
- [x] `submit_transaction` validates signed transactions before mempool insertion.
- [x] `mempool_size` exposes the current pending transaction count.
- [x] `get_balance` and `transaction_status` (pending/confirmed/unknown) queries.
- [x] `version` method for client compatibility checking.
- [x] Optional token authentication (`VDAG_RPC_TOKEN`), all methods except `version` gated when set.
- [x] Request size cap (64KB) and rate limits (50/sec per connection, 256 concurrent connections).
- [x] Live RPC intake has been tested against the release binary.

### Wallet tooling

- [x] Wallet creation and address derivation CLI commands exist.
- [x] Wallet CLI signs the canonical transfer payload.
- [x] Wallet CLI can submit signed transfers to the local RPC.
- [x] Wallet key material is encrypted with a password-derived key.
- [x] Wrong wallet passwords are rejected by authenticated decryption.
- [x] Wallet verify, encrypted backup, and restore commands exist.

### Local networking and operations

- [x] Noise and Yamux encrypted libp2p transport is configured.
- [x] Gossipsub block propagation and request/response sync work locally.
- [x] Two-node local launch and catch-up sync have been verified.
- [x] Startup waits briefly for peer discovery and uses the shared genesis parent.
- [x] Start, stop, health-check, and two-node launch scripts exist.
- [x] Peer connection limits (per-peer and global).
- [x] Malformed-message violation tracking and automatic peer banning.
- [x] Environment-aware seed bootstrap configuration with signed bootstrap validation.

## Required before public testnet

This list is now short and specific — most of what used to be here is done (see above).

### Protocol

- [ ] Define denomination and rounding rules formally (the numbers already work; this is documenting and freezing them).
- [ ] Prove the supply cap converges correctly across all halving eras (currently correct in code and covered by the subsidy test vector at one era boundary; a fuller proof across all eras is not yet written).

### State model

- [ ] Ledger checkpointing — `recompute_ledger` currently replays from genesis on every accepted block, which is correct but becomes an O(chain length)-per-block cost as the chain grows. Fine at current scale; needs addressing before a long-running testnet.
- [ ] Persist and validate state transitions atomically with block commits at the storage layer (ledger state itself is now correctly derived; the storage-write atomicity with block storage hasn't been separately audited).
- [ ] Add state migration/versioning for database upgrades.

### Networking security

- [ ] Per-IP bucketing for RPC rate limits and connection counts (currently limits are global/per-connection, not per-source-address).
- [ ] Attribute invalid-block rejections (bad PoW, bad signatures — not just malformed bytes) to a specific peer for ban purposes; currently only malformed gossip payloads count toward the ban threshold.
- [ ] Peer un-ban / recovery mechanism (bans are currently permanent for the life of the process).
- [ ] Add stable public seed nodes and operator-run bootstrap policy.
- [ ] Add NAT traversal or relay support for nodes behind private networks.
- [ ] Test partitions, delayed peers, duplicate messages, and long sync gaps under real (not loopback) network conditions.

### Clients and interfaces

- [ ] Confirmation/finality depth queries (needs a tx_id→height index; `transaction_status` currently reports pending/confirmed/unknown but not confirmation depth).
- [ ] Build a block explorer from validated persisted state.
- [ ] Document address, denomination, fee, confirmation, and finality rules in one place (currently split across this tracker, the fee spec, and the tokenomics doc).

## Mainnet launch gates

All of the following must be complete before a mainnet announcement:

- [ ] Protocol specification is frozen and versioned.
- [ ] Genesis block, network ID, treasury address, and monetary units are published. (Genesis/network ID mechanism is implemented; the actual testnet and mainnet genesis timestamps are still placeholders — see the genesis/network spec.)
- [ ] Transaction and state model is implemented, documented, and independently reviewed.
- [x] Consensus implementation has deterministic test vectors. Adversarial tests (fuzzing, deliberately malformed input at scale) are still open.
- [ ] Public testnet has run for an agreed soak period with multiple independent operators.
- [ ] Restart, backup, restore, migration, partition, and reorganization tests pass.
- [ ] Independent cryptographic, consensus, and networking audits are complete.
- [ ] Reproducible signed release binaries are available.
- [ ] Seed nodes, monitoring, alerting, firewall rules, and incident response are operational.
- [ ] Upgrade and rollback procedures have been rehearsed.
- [ ] Wallet, RPC, explorer, and operator documentation are published.
- [ ] White paper claims match implemented functionality — **corrected as of this update, see below.**

## Documentation status (corrected 2026-09-14)

[WHITE-PAPER.md](WHITE-PAPER.md) previously described zero-knowledge privacy, viewing keys, and a "multi-signature development treasury" as implemented. Both were inaccurate: ZK privacy is not implemented (account balances are plain and transparent, not commitment-based), and the actual treasury design is a single founder-controlled key, not a multisig (see VeloDAG_Tokenomics.md for the reasoning — a perpetual per-block fee with no pre-mine, publicly disclosed, rather than a multisig). **Both corrected as of this update.**

[ROADMAP.md](ROADMAP.md) was significantly out of date — it described GHOSTDAG fork choice, real PoW, and non-mock RPC as "upcoming" when they are now implemented and tested. **Corrected as of this update.**

[TESTNET_HARDENING.md](TESTNET_HARDENING.md) claimed "the remaining work is not protocol correctness" — this was wrong at the time it was written, and tonight's review found and fixed multiple genuine protocol-correctness bugs that contradict that claim directly. **Corrected as of this update** with a note pointing back here.

## Immediate next work

1. Ledger checkpointing (closes the O(n)-per-block replay cost before it matters at scale).
2. Confirmation/finality depth queries.
3. Per-IP rate/connection bucketing and invalid-block peer attribution (closes the two remaining networking-security gaps).
4. ~~Adversarial/fuzz testing.~~ **Done** — property-based tests (`proptest`) added; found and fixed a real bug (subsidy-split shift-overflow at era ≥ 64, same "silently wrong in release" class as the earlier difficulty-overflow bug). Full `cargo-fuzz` harness for the gossip/RPC deserialization paths remains a good manual (non-CI) follow-up.
5. Run a long-lived public testnet before any mainnet decision.
