# 🚀 VeloDAG Core (`velodag-core`)

[![Language: Rust](https://shields.io)](https://rust-lang.org)
[![License: MIT](https://shields.io)](LICENSE)
[![Crypto: Post-Quantum](https://shields.io)](WHITE-PAPER.md)
[![CI: Built and Passing](https://github.com)](https://github.com)

**VeloDAG (VDAG)** is a next-generation, high-performance Layer-1 ledger built natively in Rust. It utilizes a Directed Acyclic Graph (BlockDAG) ledger layout to achieve sub-second block finality, secured entirely by post-quantum lattice cryptography, persistent disk caching, and programmable zero-knowledge privacy.

📄 **Read the Deep-Dive Architecture:** [VeloDAG Technical Whitepaper](WHITE-PAPER.md)  
🗺️ **Track Long-Term Development:** [Protocol Roadmap](ROADMAP.md)

---

## ✨ Core Technology Pillars

*   🛡️ **Post-Quantum Security:** Natively utilizes NIST-standard **CRYSTALS-Dilithium2** signatures to safeguard transactions against future quantum decryption vectors.
*   ⚡ **High-Throughput BlockDAG:** Replaces rigid single-threaded linear blockchains with a multi-parent graph. Blocks are mined in parallel every **1 second**.
*   💾 **Persistent Local Ledger:** Uses the ultra-fast embedded **`sled` Key-Value engine** to serialize and commit blocks directly to non-volatile local disk storage permanently.
*   🔒 **Zero-Knowledge Privacy:** Implements structural ZK-proofs to mask addresses and transaction values while allowing optional compliant auditor viewing keys.
*   💎 **Fair-Launch & Sound Economics:** Fixed **21,000,000 supply cap** featuring a consensus-enforced **5% development tax** to fund public engineering infrastructure organically.

---

## 📂 Repository Workspace Structure

The codebase is highly modularized into isolated Rust crates managed by a central workspace engine:

*   [`vdag-node/`](vdag-node/) - The main runtime node binary featuring the asynchronous 1-second block execution engine, automatic genesis generation, and built-in CLI block explorer interface.
*   [`vdag-consensus/`](vdag-consensus/) - Enforces network validation rules, emission halvings, unconfirmed transaction mempools, and local `sled` disk database persistence.
*   [`vdag-crypto/`](vdag-crypto/) - Cryptographic layer handling post-quantum signature schemes, key derivations, and address generation.
*   [`vdag-network/`](vdag-network/) - Asynchronous Peer-to-Peer network stack running `libp2p` and Gossipsub block propagation.

---

## 🛠️ Codespace Initialization & Execution Guide

VeloDAG is fully optimized for **GitHub Codespaces**. You do not need to install heavy compilers or software on your local machine; the environment sets up automatically inside your browser.

### 1. Launching the Cloud Dev Environment
1. Navigate to this repository page on your browser.
2. Click the green **Code** button on the right, select the **Codespaces** tab, and click **Create codespace on main**.
3. Wait a few seconds for the virtual environment to load your pre-configured Rust toolchain.

### 2. Execute Internal Test Suites
Verify that the underlying lattice cryptography signing engines and coinbase verification modules pass criteria:
```bash
cargo test --workspace
```

### 3. Run the High-Speed Node Backend
Boot up the node environment to activate the automatic Genesis bootstrapper and start processing transaction simulation influxes live inside the 1-second BlockDAG intervals:
```bash
cargo run -p vdag-node
```

### 4. Query the Local Database via the CLI Explorer
To inspect the properties of any historical block written to your disk storage directory (`velodag_ledger_data`), run the app with the explorer flag. For example, to read the hardcoded immutable **Genesis Block (Block 0)**, run:
```bash
cargo run -p vdag-node -- --get-block 0000000000000000000000000000000000000000000000000000000000000000
```

---
## 📜 License
This project is open-source software licensed under the terms of the **MIT License**.

