fee = TRANSACTION_FEE                  (100 base units)
burn_share = fee / 2                   (50 — destroyed, credited to nobody)
distributed_share = fee - burn_share   (50 — split via the SAME 95/5 ratio as the block reward)
  miner_share = distributed_share * 95 / 100
  dev_share   = distributed_share - miner_share