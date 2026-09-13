# VeloDAG (VDAG): A Post-Quantum Secure BlockDAG Ledger

**Author:** Touqeer Ahmad
**Date:** August 2026 (corrected September 2026 — see note below)
**Status:** Architecture Specification / Active Implementation
**Project Repository:** https://github.com

> **Correction note (2026-09-13):** the original version of this document described zero-knowledge privacy, viewing keys, and a multi-signature treasury as implemented features. Neither was accurate. This version corrects both: ZK privacy is marked as planned future architecture, not current functionality, and the treasury description now matches the actual design (a single founder-controlled key funded by a perpetual per-block fee, not a multisig, not a pre-mine — see `VeloDAG_Tokenomics.md` for the full reasoning). See `MAINNET_TRACKER.md` for what is actually implemented and verified today.

---

## 1. Abstract

VeloDAG (VDAG) is a decentralized layer-1 network designed to address the Blockchain Trilemma without compromising long-term cryptographic security. By pairing a high-throughput, multi-parent Directed Acyclic Graph (BlockDAG) with NIST-standard post-quantum cryptography (CRYSTALS-Dilithium2), VeloDAG targets sub-second transaction processing while remaining resistant to future quantum computing attacks on its signature scheme. VeloDAG funds ongoing protocol development through a consensus-enforced 5% per-block fee — a perpetual mining fee, not a pre-mine — paired with transaction fees that partially burn network supply.

## 2. Cryptographic Security Layer (Post-Quantum Defense)

Traditional networks rely heavily on ECDSA (`secp256k1`) or Ed25519 signature schemes. Advances in quantum computing, specifically Shor's Algorithm, pose an existential threat to these mechanisms by allowing private keys to be mathematically derived from exposed public ledger addresses.

VeloDAG implements **CRYSTALS-Dilithium2** (standardized as ML-DSA-44) for all wallet address generation and transaction signing. This is implemented and verified in code today, not planned.

* **Key Generation:** Wallets use lattice-based key pairs.
* **Address Derivation:** A user's public address is the SHA3-256 hash of their Dilithium2 public key.
* **Trade-off:** Dilithium2 signatures are significantly larger than legacy signatures (~2,420 bytes vs 64 bytes). VeloDAG's transaction format carries the full public key alongside the signature to support verification.

## 3. Consensus & Topology Layer (BlockDAG)

Linear blockchains enforce single-threaded bottlenecks where only one block can be mined globally at a time.

VeloDAG uses a **Directed Acyclic Graph (BlockDAG)** structure based on the GHOSTDAG protocol family:

* **Multi-Parent Architecture:** Each block header can reference multiple parent block hashes rather than a single predecessor.
* **1-Second Block Target:** Block production targets a 1-second interval, adjusted by a difficulty algorithm.
* **Deterministic Fork Choice, Not Fork Prevention:** Competing blocks (forks) can and do occur in any decentralized network with real propagation delay — GHOSTDAG does not prevent this. What it provides is a deterministic rule (blue-score-based fork choice, implemented and tested in this codebase) for every honest node to converge on the same canonical ordering given the same set of observed blocks, so temporary forks resolve consistently rather than causing permanent disagreement.

## 4. Privacy Layer (Planned, Not Implemented)

**This section describes planned future architecture. It is not implemented in the current codebase.** Account balances today are plain and transparent — a standard account-based ledger (balance and nonce per address), visible to anyone who runs a node, the same as most account-based chains. There is no commitment scheme, no shielded balance state, and no zero-knowledge proof integration in the code today.

The long-term intent, not yet built:

* Integrate zero-knowledge proof systems into transaction verification.
* Explore shielded balance/transaction models that hide sender, recipient, or amount while preserving verifiability.
* Explore optional, user-controlled viewing keys for selective disclosure (e.g. voluntary compliance reporting), without making this the default or a protocol requirement.

None of this is scheduled against a specific version or date. It will be described in the roadmap as an active workstream only once implementation actually begins.

## 5. Tokenomics & Emission Model

* **Ticker Symbol:** VDAG
* **Hard Supply Cap:** 21,000,000 tokens
* **Block Target:** 1 Second (31,536,000 blocks per year, nominal)
* **Initial Reward:** 0.083238 VDAG tokens per block
* **Halving Schedule:** Every 4 years (126,144,000 blocks per era)
* **Development Fee:** 5% of every block subsidy is routed at the protocol layer to a development treasury address, controlled by a single key held by the founder. This is **not a multisig** and **not a pre-mine** — the treasury balance only ever grows one block at a time, at the same pace as miners, funded entirely by ongoing block rewards. No lump sum exists before genesis. See `VeloDAG_Tokenomics.md` for the full reasoning behind this model.
* **Transaction Fees:** A flat per-transaction fee is charged in addition to the transfer amount. Half is burned outright (destroying supply); the other half follows the same 95/5 split as the block reward. See `VeloDAG_Fee_Burn_Spec.md` for exact values and reasoning.

---

## Implementation Status

This document describes architecture and intent. For what is actually implemented, tested, and verified in CI today, see `MAINNET_TRACKER.md` — treat that document as authoritative over this one for any question of "is X actually built."
