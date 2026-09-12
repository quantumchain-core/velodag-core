# VeloDAG — Master Tracker

_Last updated: 2026-09-12_
_This is the single source of truth for project state. Update it every time something lands or gets found — not tool-dependent, works whether you're pasting code by hand or using an AI coding agent._

---

## ✅ VERIFIED DONE

Everything below is confirmed via passing CI (`cargo test --workspace` green on GitHub Actions), not just "written." Where a regression test exists, it's named — meaning if this ever breaks again, a test fails loudly instead of it silently regressing.

### Core Engine
- [x] GHOSTDAG multi-parent DAG engine (Blue/Red coloring, anticone tracking)
- [x] SHA3-256 PoW miner
- [x] sled embedded storage (blocks, GHOSTDAG scores, ledger state)
- [x] Account-based ledger (balances, nonces, duplicate-tx rejection)
- [x] Post-quantum signatures (CRYSTALS-Dilithium2) — real verification, not a stub
- [x] Encrypted local wallet (Argon2 + ChaCha20-Poly1305), backup/restore, wrong-password rejection
- [x] Local JSON-RPC server (`submit_transaction`, `mempool_size`)

### Networking
- [x] Noise/Yamux encrypted P2P transport (libp2p)
- [x] Gossipsub block propagation
- [x] mDNS peer auto-discovery
- [x] Persistent node identity across restarts
- [x] Block sync / catch-up protocol (`request_response`)
- [x] Orphan pool with full re-validation on replay
- [x] Signed bootstrap configuration

### Consensus Correctness (this session's review pass — all regression-tested)
- [x] **Difficulty overflow bug fixed** — `test_extreme_timestamp_gap_does_not_overflow`
- [x] **Self-declared PoW target closed** — blocks validated against independently-recomputed expected target, not their own claim
- [x] **Genesis-per-network fixed** — `genesis_hash_differs_per_network`, `genesis_hash_for_network_actually_depends_on_the_network`
- [x] **Block validation hardening** — height continuity, timestamp/future-drift limits, message size cap, transaction count cap
- [x] **Real fork choice** — `fork_choice_selects_the_heavier_branch`, `fork_choice_tie_break_is_deterministic`
- [x] **Canonical ledger/difficulty ordering** — `recompute_ledger_matches_canonical_replay` (ledger and difficulty now derived from `get_linear_sort`, not arrival order)
- [x] **Transaction fees + burn** — `transaction_fee_is_charged_split_and_partially_burned` (flat fee, half burned, half via existing 95/5 split)
- [x] **Deterministic consensus test vectors** — 6 vectors checked against independently-computed values (Python/hashlib, not the Rust code under test): genesis hashes, block hash, tx id/merkle root, subsidy halving boundary, fee distribution, difficulty-overflow scenario. GHOSTDAG coloring itself intentionally not vectorized yet (flagged in-file as a future round, not silently skipped).
- [x] **Per-transaction mempool affordability filter** — `select_affordable` replaces all-or-nothing block assembly; one bad transaction no longer drops every other valid one in the same batch.
- [x] **RPC balance/status/versioning** — `get_balance`, `transaction_status`, `version` methods; required wrapping `LedgerState` in `Arc<Mutex<...>>` so RPC actually has something to read.

**29/29 tests passing in CI as of last check.**

---

## 🔴 KNOWN GAPS — Flagged, Not Yet Fixed

Explicitly documented (not hidden) limitations from work already done:

- [ ] **Mempool block-assembly is all-or-nothing** — ~~fixed via `select_affordable`, see done items above~~
- [ ] Rejected mempool transactions aren't re-queued — logged with a reason, but not retried once the blocking condition (e.g. nonce ordering) resolves itself.
- [ ] **`recompute_ledger`/`canonical_block_order` replay from genesis every time** — fine at testnet scale, becomes O(chain length) per new block as the chain grows. No checkpointing yet.
- [ ] **`fixed_genesis_hash_for_network` uses placeholder timestamps for testnet/mainnet** — must be replaced with real, publicly-announced launch moments before those networks go live.

---

## Remaining Work — Tiered by Realistic Effort

### 🟢 Fast / mechanical
- [x] ~~Deterministic consensus test vectors~~ — **done, see above**
- [x] ~~Per-transaction mempool affordability filter~~ — **done, see above**
- [x] ~~Balance queries, transaction status queries~~ — **done, see above**
- [x] ~~RPC versioning~~ — **done, see above**
- [ ] RPC authentication for remote access
- [ ] RPC rate limits / request size limits
- [ ] Confirmation/finality queries (needs a tx_id→height/confirmation-depth index, not yet tracked)
- [ ] Peer connection limits
- [ ] Malformed-message protection
- [ ] Peer banning + recovery

### 🟡 Medium — needs a design decision or moderate new surface area
- [x] ~~Transaction fees + fee distribution~~ — **done, see above**
- [ ] Supply cap proof (document + test proving the halving schedule converges correctly)
- [ ] Denomination and rounding rules (formal doc)
- [ ] Ledger checkpointing (closes the O(n) replay gap above)

### 🔴 Slow regardless of tooling — calendar time, not coding time
- [ ] Long-running multi-node testnet
- [ ] Reorganization tests under real network conditions
- [ ] Partition / restart / long-sync-gap tests
- [ ] Fuzz testing (transaction decoding, block decoding, network messages)
- [ ] Independent cryptography + consensus security audit
- [ ] Reproducible signed binary builds
- [ ] NAT traversal / relay support

---

## Documentation Status

- [x] `VeloDAG_Genesis_Network_Spec.md` — genesis/network ID spec
- [x] `VeloDAG_Fee_Burn_Spec.md` — fee/burn spec
- [x] `VeloDAG_Tokenomics.md` — public tokenomics doc (perpetual fee, no pre-mine model)
- [ ] `ROADMAP.md` in-repo — still overstates some completed features per earlier tracker note, needs a correction pass
- [ ] `WHITE-PAPER.md` — ZK privacy / viewing keys still described as implemented; not true yet, needs marking as planned
- [ ] Final protocol specification (consolidated doc covering consensus, genesis, network IDs, fees, denomination)

---

## How To Use This File

- Check something off the moment CI confirms it green — not before.
- If a fix reveals a new gap (like the mempool all-or-nothing issue did), add it to Known Gaps immediately, don't let it live only in chat history.
- Re-tier remaining items as effort estimates change — the 🟢/🟡/🔴 split is a working guess, not gospel.
