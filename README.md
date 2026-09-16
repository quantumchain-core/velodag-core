# 🚀 VeloDAG Core (`velodag-core`)

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Crypto: Post-Quantum](https://img.shields.io/badge/crypto-post--quantum-informational.svg)](WHITE-PAPER.md)

> **Note:** the CI badge previously here was a placeholder pointing nowhere and has been removed. Once this repo has a public GitHub Actions workflow URL, replace this line with a real badge linking to it — see `.github/workflows/` for the actual workflow name.

**VeloDAG (VDAG)** is an experimental high-performance Layer-1 ledger built natively in Rust. It uses a Directed Acyclic Graph (BlockDAG) layout, post-quantum transaction signatures, proof of work, and persistent disk storage. Account balances are currently transparent; privacy proofs are planned and are not enabled by the normal ledger.

📄 **Read the Deep-Dive Architecture:** [VeloDAG Technical Whitepaper](WHITE-PAPER.md)
🗺️ **Track Long-Term Development:** [Protocol Roadmap](ROADMAP.md)
✅ **Track What's Actually Verified:** [Mainnet Readiness Tracker](MAINNET_TRACKER.md) — treat this as authoritative over any status claim elsewhere in the repo.

---

## ✨ Core Technology Pillars

*   🛡️ **Post-Quantum Security:** Natively utilizes NIST-standard **CRYSTALS-Dilithium2** signatures to safeguard transactions against future quantum decryption vectors.
*   ⚡ **High-Throughput BlockDAG:** Replaces rigid single-threaded linear blockchains with a multi-parent graph. Blocks are mined in parallel every **1 second**.
*   💾 **Persistent Local Ledger:** Uses the ultra-fast embedded **`sled` Key-Value engine** to serialize and commit blocks directly to non-volatile local disk storage permanently.
*   🔒 **Privacy — planned, not implemented:** Account balances are fully transparent today (a standard account-based ledger). ZK transaction proofs and shielded balances are future work under active development on a separate branch, not yet merged. **Any specific claim about what that branch currently does should be verified against its own code and tests before being described here as working** — this README doesn't make claims about unreviewed branch work.
*   💎 **Fair-Launch & Sound Economics:** Fixed **21,000,000 supply cap** featuring a consensus-enforced, perpetual **5% development fee** (not a pre-mine — funded entirely by ongoing block rewards) to fund public engineering infrastructure organically. See `VeloDAG_Tokenomics.md` for the full model.

---

## 📂 Repository Workspace Structure

*   [`vdag-node/`](vdag-node/) — The main runtime node binary: the asynchronous 1-second block execution engine, the actual P2P networking stack (libp2p, gossipsub, sync protocol, peer connection limits and banning), the JSON-RPC server, wallet CLI, automatic genesis generation, and the CLI block explorer.
*   [`vdag-consensus/`](vdag-consensus/) — Consensus rules: block/transaction validation, GHOSTDAG fork choice, difficulty adjustment, emission halvings, the ledger, and mempool logic.
*   [`vdag-crypto/`](vdag-crypto/) — Post-quantum signature schemes, key derivation, and address generation.

For the granular, test-verified status of every component above, see [`MAINNET_TRACKER.md`](MAINNET_TRACKER.md).

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
