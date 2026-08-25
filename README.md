# 🚀 VeloDAG Core (`velodag-core`)

[![Language: Rust](https://shields.io)](https://rust-lang.org)
[![License: MIT](https://shields.io)](LICENSE)
[![Crypto: Post-Quantum](https://shields.io)](WHITE-PAPER.md)

**VeloDAG (VDAG)** is a next-generation, high-performance Layer-1 ledger built natively in Rust. It utilizes a Directed Acyclic Graph (BlockDAG) ledger layout to achieve sub-second block finality, secured entirely by post-quantum lattice cryptography and programmable zero-knowledge privacy.

📄 **Read the Full Technical Architecture:** [VeloDAG Whitepaper](WHITE-PAPER.md)

---

## ✨ Core Technology Pillars

*   🛡️ **Post-Quantum Security:** Natively utilizes NIST-standard **CRYSTALS-Dilithium2** signatures to safeguard against quantum decryption vectors.
*   ⚡ **High-Throughput BlockDAG:** Replaces rigid linear blockchains with a multi-parent graph. Blocks are processed in parallel every **1 second**.
*   🔒 **Zero-Knowledge Privacy:** Implements structural ZK-proofs to mask addresses and transaction amounts while allowing optional compliance viewing keys.
*   💎 **Fair-Launch & Sound Economics:** Fixed **21,000,000 supply cap** with a consensus-enforced **5% development tax** to fund public infrastructure organically.

---

## 📂 Repository Workspace Structure

The project is modularized into isolated Rust crates linked together via a unified workspace matrix:

*   [`vdag-node/`](vdag-node/) - The main runtime node binary featuring the asynchronous 1-second block execution engine.
*   [`vdag-crypto/`](vdag-crypto/) - Cryptographic layer handling post-quantum signature schemes and key derivations.
*   [`vdag-consensus/`](vdag-consensus/) - Enforces network validation rules, halvings, and the 5% dev tax allocation splits.
*   [`vdag-network/`](vdag-network/) - Asynchronous Peer-to-Peer network stack running `libp2p` and Gossipsub block propagation.

---

## 🛠️ Laptop Compilation & Local Run Guide

When accessing this repository from a machine with the Rust toolchain installed, use the following commands to initialize and run a simulated local cluster instance:

### 1. Clone the Codebase
```bash
git clone https://github.com
cd velodag-core
```

### 2. Execute Internal Test Suites
Verify that the underlying lattice cryptography signing engines and coinbase verification modules pass local criteria:
```bash
cargo test --workspace
```

### 3. Run the Node Backend Simulator
Initialize the runtime environment to boot the P2P networking configurations and activate the transaction-generating mempool heartbeat:
```bash
cargo run -p vdag-node
```

---

## 🗺️ Product Roadmap

*   [x] **Phase 1:** Core modular workspace assembly and file mapping.
*   [x] **Phase 2:** Local transaction mempool queues and mock JSON-RPC generator.
*   [x] **Phase 3:** Integration of `libp2p` network behavior swarms and multi-peer discovery listeners.
*   [ ] **Phase 4 (Next):** Complete state persistence database configuration, genesis block mining engine, and public mining testnet launch.

---
## 📜 License
This project is open-source software licensed under the terms of the **MIT License**.
