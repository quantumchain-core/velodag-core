# VeloDAG — Tokenomics

_Last updated: 2026-09-10 — rebuilt to include the transaction fee/burn mechanism (added after the original version of this document was written) and to directly answer the most common question: why would anyone mine before there's transaction volume?_

This document is meant to be published publicly (repo README, website, whitepaper) so anyone evaluating VeloDAG can see exactly how the token model works before running a node or mining. Figures below are pulled directly from the actual constants in `vdag-consensus/src/lib.rs`, not estimated.

---

## 1. The short answer: two separate income streams, not one

**Miners are paid a block reward for every block they mine, whether or not it contains any transactions.** Transaction fees are a *second, additional* income stream that only exists once there's real transaction volume. This is deliberate, and it's the same bootstrapping model Bitcoin and most proof-of-work chains use — day one, with zero organic transactions, mining is still worthwhile because the block reward alone pays out.

| | Exists from block 1, regardless of transactions? | Depends on transaction volume? |
|---|---|---|
| **Block reward (mining reward)** | ✅ Yes — every ~1 second | No |
| **Transaction fees** | No — zero if no transactions | ✅ Yes |

---

## 2. Block Reward (Mining Reward) — the primary incentive

This is paid automatically as part of every block, via the coinbase fields, and is checked by consensus (`verify_coinbase_rewards`) — a block with the wrong reward amount is invalid and gets rejected by every node.

| Parameter | Value |
|---|---|
| Initial block reward | 83,238 base units (0.083238 VDAG) per block |
| Block target | 1 second |
| Blocks per year (nominal) | 31,536,000 |
| Nominal annual issuance (Year 1) | ~2,624,993.57 VDAG |
| Halving interval | Every 4 years (126,144,000 blocks) |
| Split | 95% to the miner / 5% to the development treasury |

**Concretely, per block (Year 1, before any halving):**
- Miner receives: 79,077 base units (0.079077 VDAG)
- Treasury receives: 4,161 base units (0.004161 VDAG)

This is why running a node and mining makes sense from the very first block: you're being paid roughly 0.08 VDAG every second regardless of whether anyone has sent a transaction yet. This is not a pre-mine and not a lump sum — it only exists one block at a time, at the pace blocks are actually mined.

---

## 3. Transaction Fees — the secondary, volume-driven incentive

Once there *is* transaction activity, each transaction adds a small additional fee on top of the block reward for that block.

| Parameter | Value |
|---|---|
| Fee per transaction | 100 base units (0.0001 VDAG), flat — not a percentage of the amount sent |
| Sender pays | amount + fee |
| Recipient receives | exactly the amount (fee is additional, not deducted from the transfer) |

**The fee splits three ways:**
- **50 base units burned** — destroyed, credited to nobody. This is what makes network *usage*, not just mining, pull circulating supply down.
- **48 base units to the miner** — same 95/5 ratio as the block reward, applied to the non-burned half.
- **2 base units to the treasury** — same ratio, same reasoning: one consistent split, not two different rules to explain.

See `VeloDAG_Fee_Burn_Spec.md` for the full reasoning behind this split.

**Why fees are small relative to the block reward today:** at 100 base units per transaction versus ~79,000 base units per block from the reward alone, transaction fees are not meant to be the primary miner incentive yet — they exist mainly to make spam cost something. As real transaction volume grows, fee income grows with it; the block reward halves every 4 years by design, so the balance between the two naturally shifts over the very long term, the same way it does on other proof-of-work networks.

---

## 4. Supply Cap & Burning — they don't conflict

- **Hard cap:** 21,000,000 VDAG, enforced by the halving schedule (block reward issuance only, capped by construction).
- **Burning only ever pulls circulating supply *below* the cap.** Burned value was already mined (already counted against the cap) before being destroyed — burning cannot violate the fixed ceiling, it can only tighten it further.
- **No special burn address.** The burned share of a fee is simply never credited to anyone. Auditable in one sentence: `circulating_supply = total_mined - total_burned`, verifiable by anyone replaying the chain.

---

## 5. Development Treasury — perpetual fee, not a pre-mine

- **Single key, founder-controlled.** No multisig, no lump sum, no allocation minted before genesis.
- **Funded two ways, both ongoing:** 5% of every block reward, and (once there's volume) a share of transaction fees. Both accrue one block/transaction at a time — there is nothing to withdraw that didn't already come from real, ongoing network activity.
- **Treasury address:** `[PUBLISH BEFORE MAINNET LAUNCH]`

---

## 6. What This Is Not

- ❌ No pre-mine or founder allocation minted before genesis
- ❌ No private/pre-sale token allocation
- ❌ No ability to mint outside the fixed emission schedule
- ❌ No ability to change the reward split, fee amount, hard cap, or halving schedule without a hard fork (visible and coordinated, not silent)
- ❌ Transaction fees are not the primary reason to mine today — the block reward is. Don't market this as a fee-driven network yet; it isn't one.

---

## 7. Legal Note

This document is a technical/economic description of the protocol, not legal or financial advice. Token classification varies by jurisdiction and depends on factors beyond this document. If you are evaluating regulatory status in your jurisdiction, consult a qualified lawyer — this is tracked as an open item in `MAINNET_TRACKER.md` and should be completed before mainnet launch.

---

## 8. Open Items Before Publishing

- [ ] Fill in the actual treasury address once generated
- [ ] Legal review complete (see `MAINNET_TRACKER.md`)
- [ ] Link this document from the main README (already linked as of this update)
- [ ] Confirm these figures still match `vdag-consensus/src/lib.rs` if any constant changes before mainnet — this document should never drift from the actual code the way the original version of this doc drifted after the fee/burn mechanism was added.
