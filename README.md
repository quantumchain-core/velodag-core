# 🚀 VeloDAG Core (`velodag-core`)

[![Language: Rust](https://shields.io)](https://rust-lang.org)
[![License: MIT](https://shields.io)](LICENSE)
[![Crypto: Post-Quantum](https://shields.io)](WHITE-PAPER.md)
[![CI: Built and Passing](https://github.com)](https://github.com)

**VeloDAG (VDAG)** is an experimental high-performance Layer-1 ledger built natively in Rust. It uses a Directed Acyclic Graph (BlockDAG) layout, post-quantum transaction signatures, proof of work, and persistent disk storage. Account balances are currently transparent; privacy proofs are planned and are not enabled by the normal ledger.

📄 **Read the Deep-Dive Architecture:** [VeloDAG Technical Whitepaper](WHITE-PAPER.md)  
🗺️ **Track Long-Term Development:** [Protocol Roadmap](ROADMAP.md)

---

## ✨ Core Technology Pillars

*   🛡️ **Post-Quantum Security:** Natively utilizes NIST-standard **CRYSTALS-Dilithium2** signatures to safeguard transactions against future quantum decryption vectors.
*   ⚡ **High-Throughput BlockDAG:** Replaces rigid single-threaded linear blockchains with a multi-parent graph. Blocks are mined in parallel every **1 second**.
*   💾 **Persistent Local Ledger:** Uses the ultra-fast embedded **`sled` Key-Value engine** to serialize and commit blocks directly to non-volatile local disk storage permanently.
*   🔒 **Privacy roadmap:** The node now performs a local non-interactive producer proof-of-knowledge check before block production. This authorizes the configured miner but does not hide transactions; ZK transaction proofs and shielded balances remain future work.
*   💎 **Fair-Launch & Sound Economics:** Fixed **21,000,000 supply cap** featuring a consensus-enforced **5% development tax** to fund public engineering infrastructure organically.

---

## 📂 Repository Workspace Structure

The codebase is highly modularized into isolated Rust crates managed by a central workspace engine:

*   [`vdag-node/`](vdag-node/) - The main runtime node binary featuring the asynchronous 1-second block execution engine, automatic genesis generation, and built-in CLI block explorer interface.
*   [`vdag-consensus/`](vdag-consensus/) - Enforces network validation rules, emission halvings, unconfirmed transaction mempools, and local `sled` disk database persistence.
*   [`vdag-crypto/`](vdag-crypto/) - Cryptographic layer handling post-quantum signature schemes, key derivations, and address generation.
*   [`vdag-network/`](vdag-network/) - Asynchronous Peer-to-Peer network stack running `libp2p` and Gossipsub block propagation.

---

## 🛠️ Setup on Any Device

VeloDAG is designed to run from a standard Rust dev environment or a GitHub Codespace. These commands are portable and do not rely on a fixed machine path.

### 1. Clone and build

From a Unix-like shell:

```bash
git clone https://github.com/quantumchain-core/velodag-core.git
cd velodag-core
cargo build --release
```

### 2. Verify the workspace

```bash
cargo check --workspace
cargo test --workspace
```

### 3. Run the node

```bash
cargo run -p vdag-node
```

For a release binary built in the repo, run:

```bash
./target/release/vdag-node
```

### 4. Use the signed bootstrap config for the active network

The repo supports environment-aware bootstrap policy via `VDAG_NETWORK`:

```bash
VDAG_NETWORK=devnet ./target/release/vdag-node
VDAG_NETWORK=testnet ./target/release/vdag-node
```

Signed config files are shipped in the repo root:

- `bootstrap.devnet.json`
- `bootstrap.testnet.json`

These files are validated before the node dials any bootstrap peers.

### 5. Query the local ledger via explorer mode

```bash
./target/release/vdag-node --get-block 0000000000000000000000000000000000000000000000000000000000000000
```

If you are using the repo as a local checkout, the helper scripts in `scripts/testnet/` automatically detect the repo root and do not require a hardcoded path.

---
## 📜 License
This project is open-source software licensed under the terms of the **MIT License**.

