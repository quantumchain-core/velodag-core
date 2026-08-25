# 🗺️ VeloDAG Protocol Architecture Roadmap

This document outlines the development lifecycle phases for **VeloDAG (VDAG)**. It tracks completed components, active builds, and planned features across our engineering timeline.

---

## 🟩 Phase 1: Cryptographic Foundation & Core Engine (COMPLETED)
*   [x] Establish modular Cargo workspace layout (`vdag-node`, `vdag-crypto`, `vdag-consensus`).
*   [x] Integrate NIST-standard **CRYSTALS-Dilithium2** signature verification.
*   [x] Write SHA3-256 wallet address derivation algorithms.
*   [x] Implement asynchronous 1-second interval execution loop using Tokio.
*   [x] Standardize bitwise shift operators for automatic 4-year token reward halving eras.

## 🟩 Phase 2: Mempool Management & Local Influx Caching (COMPLETED)
*   [x] Design double-spend preventative `HashMap` pending transaction transaction processing arrays.
*   [x] Build transaction memory pool queues (`Mempool`) to process up to 1,000 transactions per batch.
*   [x] Implement mock JSON-RPC API automated transaction generator inside the main loop for throughput simulation.

## 🟩 Phase 3: Peer-to-Peer Topology Layout (COMPLETED)
*   [x] Integrate `libp2p` v0.53 networking protocols into the workspace matrix.
*   [x] Configure secure TCP encrypted connection channels with Noise encryption and Yamux multiplexing.
*   [x] Build Multicast DNS (mDNS) automatic background task local peer discovery listeners.
*   [x] Implement `Gossipsub` network behavior mesh to subscribe to global `"vdag-blocks"` propagation lanes.

## 🟩 Phase 4: State Persistence, Genesis, & CLI Auditing (COMPLETED)
*   [x] Integrate the embedded, high-performance pure-Rust Key-Value database (`sled`).
*   [x] Implement binary object serialization/deserialization layers utilizing `bincode` and `serde`.
*   [x] Write automatic ledger initialization checks to self-mint the immutable, multi-parent **Genesis Block 0**.
*   [x] Build a built-in command line terminal block explorer tool (`--get-block [HASH]`) to query historical blocks from physical disk sectors.
*   [x] Configure a custom GitHub Actions `.devcontainer` settings manifest to automate cloud compilation.

---

## 🟨 Phase 5: GHOSTDAG Protocol Implementation (UPCOMING)
*   [ ] Upgrade linear tip accumulation to a true **GHOSTDAG sorting algorithm**.
*   [ ] Implement anticone block weight calculations to systematically organize parallel blocks without network conflicts.
*   [ ] Replace arbitrary mock nonce loops with a secure Proof-of-Work (PoW) hashing function (e.g., a memory-hard algorithm to allow fair GPU/CPU mining).

## 🟨 Phase 6: Programmable ZK-Privacy Layer (UPCOMING)
*   [ ] Integrate cryptographic proving frameworks (such as `arkworks` or `plonky3`) into transaction verification scopes.
*   [ ] Implement zero-knowledge proofs (zk-SNARKs) to completely mask transactional addresses and asset values.
*   [ ] Design native, programmable compliant **Auditor Viewing Keys** to allow users to generate encrypted evidence of tax compliance seamlessly.

## 🟦 Phase 7: Public Testnet & Client Interfaces (FUTURE)
*   [ ] Launch public, decentralized seed nodes to anchor initial global network connections.
*   [ ] Build an open-source web browser-based Block Explorer tool interface.
*   [ ] Package a lightweight desktop or browser wallet extension application natively supporting Dilithium2 address layouts.
*   [ ] List on decentralized/hobbyist exchanges to initiate public asset trading data feeds.
