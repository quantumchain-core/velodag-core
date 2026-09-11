# VeloDAG — Transaction Fee & Burn Specification

_Status: DRAFT — values below are the recommended starting point, adjustable before public testnet, not after._

This closes two open tracker items: transaction fees + fee distribution, and gives the tokenomics story a genuine second leg — supply isn't only created by mining, it's also destroyed by usage.

---

## 1. The Fee

- **Flat, protocol-fixed fee per transaction: `TRANSACTION_FEE = 100` base units** (0.0001 VDAG at the existing 1,000,000-base-unit denomination).
- Not a percentage of the amount sent, not user-chosen, not a fee market. A fixed constant, same philosophy as the block reward and halving schedule: protocol-enforced, not configurable per-transaction.
- **Why flat, not percentage:** a 1000-token transfer isn't more expensive to process than a 1-token transfer — charging proportionally to amount doesn't match the actual cost being covered (spam resistance + fee incentive).
- **Why not a fee market yet:** that's the long-term correct design once there's real congestion to prioritize against. Building congestion-pricing for congestion that doesn't exist yet is unnecessary complexity today. Revisit once there's real transaction volume.
- The fee is **not part of the signed transaction payload** — it's a protocol constant applied at settlement time, not a value the sender chooses or signs. Signature scheme is unaffected.

## 2. The Split — Burn + Existing 95/5 Distribution

Per transaction, the fee splits as follows:

```
fee = TRANSACTION_FEE                  (100 base units)
burn_share = fee / 2                   (50 — destroyed, credited to nobody)
distributed_share = fee - burn_share   (50 — split via the SAME 95/5 ratio as the block reward)
  miner_share = distributed_share * 95 / 100
  dev_share   = distributed_share - miner_share
```

**Why reuse the existing 95/5 ratio rather than a new one:** a second, different split ratio just for fees makes the tokenomics story harder to explain for no real benefit. "Miners and the treasury earn from network activity at a consistent rate, whether that's the block subsidy or transaction fees" is one clean sentence. A different fee ratio invites "why is this one different" with no good answer.

**Sender pays `amount + fee`. Recipient receives exactly `amount`** — fee is additional to the transfer, not deducted from it. This matches standard UX expectations (Bitcoin, Ethereum).

## 3. What Burning Actually Means Here

No burn address. No special account. The burned share is simply never credited to anyone — sender's balance decreases by the full fee, but only `distributed_share` (not the full fee) shows up as a credit anywhere else. Auditable in one sentence:

```
circulating_supply = total_mined - total_burned
```

Verifiable by anyone replaying the chain, no special-cased address that could be mistaken for a real spendable account.

## 4. Interaction With the 21M Hard Cap

Burning only ever pulls circulating supply *below* the fixed emission ceiling — it cannot violate the cap, since burned value was already mined (already counted against the cap) before being destroyed. Worth stating explicitly since burn mechanisms are sometimes mistakenly assumed to conflict with a fixed supply cap. They don't; they can only tighten it further.

## 5. Honest Expectation to Set

With near-zero organic transaction volume today, the deflationary effect of burning will be invisible for a long time. This spec is about the mechanism existing and being auditable now — not about visible deflation happening soon.

## 6. Implementation Notes

- `coinbase_miner_output` / `coinbase_dev_output` fields remain block-reward-only, unchanged. Fee distribution is applied as **separate balance credits** during transaction processing, not folded into the coinbase fields — keeps `verify_coinbase_rewards()` untouched and the blast radius of this change small.
- Sender's required balance becomes `amount + TRANSACTION_FEE`, checked with the same overflow-safe arithmetic already used elsewhere in `LedgerState::apply_block`.
- The existing 95/5 split math (`calculate_subsidy_split`) gets refactored into a shared internal helper so both the block reward and the fee distribution use one single source of truth for the ratio, not two copies that could drift apart.

## 7. Known Adjacent Gap — Not Fixed by This Spec

Mempool block-assembly currently grabs a batch of transactions and lets `LedgerState::apply_block` reject the **entire block** if any single transaction in the batch is unaffordable — there's no per-transaction affordability pre-filter before assembly, and rejected transactions are currently lost from the mempool rather than retried. Fees make this somewhat more likely to trigger (balance requirement just went up), but this is a pre-existing gap unrelated to fees specifically, and is tracked separately rather than fixed in this round.
