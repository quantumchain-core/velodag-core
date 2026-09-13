# 🗺️ VeloDAG Protocol Architecture Roadmap

_Last corrected: 2026-09-13 — see note below._

> **Correction note:** this document previously described several completed, tested components (real PoW, GHOSTDAG fork choice, a hardened RPC server) as "upcoming," while also describing a mock/simulated RPC layer as complete. Both were inaccurate. This version reflects actual current state. For the authoritative, test-verified status of any specific item, see `MAINNET_TRACKER.md` — this roadmap is for narrative/phase context, that tracker is the source of truth.

This document outlines the development lifecycle phases for **VeloDAG (VDAG)**. It tracks completed components, active builds, and planned features across the engineering timeline.

---

## 🟩 Phase 1: Cryptographic Foundation & Core Engine (COMPLETED)
* [x] Modular Cargo workspace layout (`vdag-node`, `vdag-crypto`, `vdag-consensus`).
* [x] NIST-standard **CRYSTALS-Dilithium2** signature generation and verification — real, verified, not a stub.
* [x] SHA3-256 wallet address derivation.
* [x] Asynchronous 1-second block production loop using Tokio.
* [x] Bitwise-shift-based halving schedule (verified against independently-computed test vectors, including at the exact era boundary).

## 🟩 Phase 2: Mempool, Ledger & Transaction Correctness (COMPLETED)
* [x] Mempool with duplicate-transaction rejection and batch draining.
* [x] Account-based ledger: balances, sequential nonces, signature verification, insufficient-balance rejection.
* [x] Per-transaction mempool affordability filtering — one unaffordable transaction no longer causes an entire batch to be dropped.
* [x] Transaction fees: flat fee per transaction, half burned, half distributed via the existing 95/5 miner/treasury split.
* [x] Real JSON-RPC server: `submit_transaction`, `mempool_size`, `get_balance`, `transaction_status`, `version` — no longer a mock/simulated generator.
* [x] RPC hardening: optional token authentication, request size limits, per-connection rate limits, concurrent connection caps.

## 🟩 Phase 3: Peer-to-Peer Topology (COMPLETED)
* [x] `libp2p` v0.53 networking, Noise encryption + Yamux multiplexing.
* [x] mDNS local peer discovery.
* [x] Gossipsub mesh for block propagation, tuned for small-network mesh parameters.
* [x] Request/response sync protocol for catch-up.
* [x] Peer connection limits (per-peer and global), malformed-message violation tracking, automatic banning.

## 🟩 Phase 4: State Persistence, Genesis, & CLI Auditing (COMPLETED)
* [x] `sled` embedded key-value storage for blocks, GHOSTDAG scores, and ledger snapshots.
* [x] `bincode`/`serde` serialization.
* [x] Fixed, network-specific genesis blocks (devnet/testnet/mainnet each have distinct genesis hashes; network ID is checked in the P2P handshake, not just genesis).
* [x] CLI block explorer (`--get-block [HASH]`).
* [x] GitHub Actions CI running the full test suite on every push.

## 🟩 Phase 5: GHOSTDAG Consensus (COMPLETED — core mechanism; hardening ongoing)
* [x] Real SHA3-256 proof-of-work, difficulty-adjusted (not a mock nonce loop).
* [x] Blue/red anticone coloring (`calculate_ghostdag_data`).
* [x] **Deterministic fork choice**: canonical tip selection by highest blue_score, ties broken by lowest hash — implemented and regression-tested (including a two-branch fork scenario proving the heavier branch wins).
* [x] **Canonical ledger/difficulty ordering**: both are derived from `get_linear_sort` (the canonical GHOSTDAG order), not raw arrival order — closing a real correctness gap where two honest nodes could otherwise disagree on balances depending on message arrival timing.
* [x] Block validation hardening: height continuity, timestamp/future-drift limits, message size and transaction-count caps.
* [x] Difficulty adjustment hardened against integer overflow (a real bug found and fixed this cycle — see `MAINNET_TRACKER.md`).
* [x] Deterministic consensus test vectors, checked against independently-computed values, not just internal self-consistency.
* [ ] Ledger checkpointing — `recompute_ledger` currently replays from genesis on every block; correct but not yet scalable to a long-running chain.
* [ ] Adversarial/fuzz testing of consensus and network message handling.

## 🟨 Phase 6: Programmable ZK-Privacy Layer (PLANNED, NOT STARTED)
* [ ] Integrate zero-knowledge proving frameworks into transaction verification.
* [ ] Explore shielded balance/transaction models.
* [ ] Explore optional, user-controlled viewing keys for voluntary disclosure.

No code exists for this phase today. Account balances are fully transparent (a standard account-based ledger), the same as most non-privacy chains. See `WHITE-PAPER.md` §4 for the corrected description of this phase's actual status.

## 🟦 Phase 7: Public Testnet & Client Interfaces (NEXT)
* [ ] Launch public, decentralized seed nodes.
* [ ] Per-IP rate/connection bucketing for both RPC and P2P (currently global/per-connection, not per-source-address).
* [ ] Attribute invalid-block rejections (not just malformed bytes) to a peer for ban purposes.
* [ ] Block explorer web interface.
* [ ] Wallet client application.
* [ ] Long-running multi-operator soak test before any mainnet decision.

---

For the granular, test-by-test verification status behind every checkbox above, see `MAINNET_TRACKER.md`.
