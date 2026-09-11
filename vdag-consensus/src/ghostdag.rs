// vdag-consensus/src/ghostdag.rs

use crate::{LedgerState, VeloBlock};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

pub type BlockHash = [u8; 32];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GhostdagData {
    pub blue_score: u64,
    pub selected_parent: Option<BlockHash>,
    pub blues: Vec<BlockHash>,
    pub reds: Vec<BlockHash>,
}

pub struct GhostdagManager {
    pub k: usize,
    pub block_store: HashMap<BlockHash, VeloBlock>,
    pub ghostdag_cache: HashMap<BlockHash, GhostdagData>,
}

impl GhostdagManager {
    pub fn new(k: usize) -> Self {
        Self {
            k,
            block_store: HashMap::new(),
            ghostdag_cache: HashMap::new(),
        }
    }

    /// Primary entry point to color and sort a block according to GHOSTDAG protocol rules
    pub fn calculate_ghostdag_data(
        &mut self,
        block: &VeloBlock,
        _block_hash: BlockHash,
    ) -> GhostdagData {
        // 1. Genesis Block Check (Has no parents)
        if block.header.parents.is_empty() {
            return GhostdagData {
                blue_score: 0,
                selected_parent: None,
                blues: vec![],
                reds: vec![],
            };
        }

        // 2. Find the Selected Parent (the parent hash with the highest blue score)
        let selected_parent = block
            .header
            .parents
            .iter()
            .filter_map(|p| self.ghostdag_cache.get(p).map(|data| (p, data)))
            .max_by_key(|(_, data)| data.blue_score)
            .map(|(hash, _)| *hash);

        let mut blues = Vec::new();
        let mut reds = Vec::new();

        if let Some(ref sp) = selected_parent {
            // Selected parent's blue set is automatically inherited
            if let Some(sp_data) = self.ghostdag_cache.get(sp) {
                blues.push(*sp);
                // Inherit prior blue set ancestors directly
                for ancestral_blue in &sp_data.blues {
                    if !blues.contains(ancestral_blue) {
                        blues.push(*ancestral_blue);
                    }
                }
            }

            // 3. True Graph Discovery of the Anticone
            let anticone = self.find_anticone(block, sp);

            // 4. Deterministically sort anticone to maintain uniform consensus calculation across peers
            let mut sorted_anticone = anticone;
            sorted_anticone.sort();

            for candidate in sorted_anticone {
                if self.can_be_blue(&candidate, &blues) {
                    blues.push(candidate);
                } else {
                    reds.push(candidate);
                }
            }
        }

        // 5. Total blue score calculation
        let parent_blue_score = selected_parent
            .and_then(|sp| self.ghostdag_cache.get(&sp))
            .map(|data| data.blue_score)
            .unwrap_or(0);

        GhostdagData {
            blue_score: parent_blue_score + blues.len() as u64,
            selected_parent,
            blues,
            reds,
        }
    }

    /// DISCOVERY ENGINE: Finds parallel blocks that are neither ancestors nor descendants of the selected parent
    fn find_anticone(
        &self,
        current_block: &VeloBlock,
        selected_parent: &BlockHash,
    ) -> Vec<BlockHash> {
        let mut anticone = Vec::new();
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        let selected_parent_past = self.get_past_set(selected_parent);

        // Bootstrap queue with the block's parent pointers
        for parent in &current_block.header.parents {
            if parent != selected_parent {
                queue.push_back(*parent);
                visited.insert(*parent);
            }
        }

        while let Some(current_hash) = queue.pop_front() {
            if !selected_parent_past.contains(&current_hash) && current_hash != *selected_parent {
                if !anticone.contains(&current_hash) {
                    anticone.push(current_hash);
                }

                if let Some(blk) = self.block_store.get(&current_hash) {
                    for parent in &blk.header.parents {
                        if !visited.contains(parent) {
                            visited.insert(*parent);
                            queue.push_back(*parent);
                        }
                    }
                }
            }
        }

        anticone
    }

    /// STRICT K-FACTOR CONSTRAINT ENGINE: Verifies the true blue anticone size threshold rule
    fn can_be_blue(&self, candidate: &BlockHash, current_blues: &[BlockHash]) -> bool {
        let candidate_past = self.get_past_set(candidate);

        for blue in current_blues {
            let blue_past = self.get_past_set(blue);

            // If the candidate block is not in the past of the blue block,
            // and the blue block is not in the past of the candidate block,
            // they are mutually in each other's anticones.
            if !blue_past.contains(candidate)
                && !candidate_past.contains(blue)
                && *blue != *candidate
            {
                // Count how many current blues are also in this specific blue block's anticone
                let mut anticone_count = 0;
                for other_blue in current_blues {
                    if other_blue != blue {
                        let other_past = self.get_past_set(other_blue);
                        if !blue_past.contains(other_blue) && !other_past.contains(blue) {
                            anticone_count += 1;
                        }
                    }
                }

                // If it pushes the anticone size over K limits, it must be marked Red
                if anticone_count >= self.k {
                    return false;
                }
            }
        }
        true
    }

    /// Reconstructs the complete past historical parent graph recursively
    fn get_past_set(&self, block_hash: &BlockHash) -> HashSet<BlockHash> {
        let mut past = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(*block_hash);

        while let Some(hash) = queue.pop_front() {
            if let Some(blk) = self.block_store.get(&hash) {
                for parent in &blk.header.parents {
                    if !past.contains(parent) {
                        past.insert(*parent);
                        queue.push_back(*parent);
                    }
                }
            }
        }
        past
    }

    /// DETERMINISTIC ORDERING ENGINE: Flattens the DAG graph into a single execution stream
    pub fn get_linear_sort(&self, tip_hash: &BlockHash) -> Vec<BlockHash> {        let mut order = Vec::new();
        let mut current = Some(*tip_hash);

        // Follow the selected parent path back to Genesis, collecting branches deterministically
        while let Some(hash) = current {
            if let Some(data) = self.ghostdag_cache.get(&hash) {
                let mut local_set = data.blues.clone();
                local_set.extend(data.reds.clone());
                local_set.sort(); // Maintain strict sorting across different hardware instances

                for block in local_set {
                    if !order.contains(&block) {
                        order.push(block);
                    }
                }

                if !order.contains(&hash) {
                    order.push(hash);
                }
                current = data.selected_parent;
            } else {
                current = None;
            }
        }
        order.reverse(); // Reverse to read from Genesis onwards
        order
    }

    /// Computes the current set of tips: blocks known locally that are not
    /// listed as a parent by any other known block. In today's
    /// single-parent-per-block mining model this is usually exactly one
    /// block, but the function handles a true multi-tip fork correctly,
    /// which is the entire point of doing fork choice at all -- a fork
    /// only exists when there's more than one tip to choose between.
    pub fn compute_tips(&self) -> Vec<BlockHash> {
        let referenced_as_parent: HashSet<BlockHash> = self
            .block_store
            .values()
            .flat_map(|b| b.header.parents.iter().copied())
            .collect();

        self.block_store
            .keys()
            .filter(|hash| !referenced_as_parent.contains(*hash))
            .copied()
            .collect()
    }

    /// GHOSTDAG fork-choice rule: among a set of candidate tips, the one
    /// with the highest blue_score wins -- the "heaviest" / most-blue
    /// subDAG is canonical. Ties are broken by lowest hash bytes so every
    /// honest node given the same candidate set reaches the identical
    /// decision; fork choice that isn't fully deterministic means nodes
    /// can permanently disagree about which chain is canonical.
    pub fn select_best_tip(&self, candidates: &[BlockHash]) -> Option<BlockHash> {
        candidates
            .iter()
            .filter_map(|hash| {
                self.ghostdag_cache
                    .get(hash)
                    .map(|data| (*hash, data.blue_score))
            })
            .max_by(|(hash_a, score_a), (hash_b, score_b)| {
                score_a.cmp(score_b).then_with(|| hash_b.cmp(hash_a))
            })
            .map(|(hash, _)| hash)
    }

    /// Convenience: computes the current tip set and selects the canonical
    /// one via `select_best_tip`. Returns `None` only if there are no
    /// blocks at all locally (shouldn't happen once genesis is loaded).
    pub fn select_canonical_tip(&self) -> Option<BlockHash> {
        self.select_best_tip(&self.compute_tips())
    }

    /// Recomputes ledger state from scratch by replaying every block in
    /// `tip`'s canonical linear order (`get_linear_sort`), rather than
    /// trusting whatever order blocks happened to be applied in as they
    /// arrived.
    ///
    /// This closes a real gap: ledger balances were previously a function
    /// of arrival order (whichever order blocks were processed in), not of
    /// the canonical chain -- meaning two honest nodes that saw the exact
    /// same set of blocks in a different order could, in principle, end up
    /// disagreeing about balances even though they agree on which blocks
    /// exist. Replaying via `get_linear_sort` guarantees every node
    /// converges to the identical ledger given the identical DAG,
    /// regardless of arrival order.
    ///
    /// Known limitation, intentionally not solved here: this replays from
    /// genesis every time it's called, which is fine at testnet scale but
    /// becomes an O(chain length) cost per new block as the chain grows.
    /// Checkpointing / incremental recomputation is a real optimization to
    /// revisit before this needs to handle a long-running mainnet chain --
    /// correctness first, performance once correctness is proven.
    pub fn recompute_ledger(&self, tip: &BlockHash) -> Result<LedgerState, String> {
        let mut ledger = LedgerState::default();
        for hash in self.get_linear_sort(tip) {
            let block = self
                .block_store
                .get(&hash)
                .ok_or_else(|| format!("missing block {hash:02x?} during ledger replay"))?;
            ledger.apply_block(block)?;
        }
        Ok(ledger)
    }

    /// Returns the canonical linear order as actual block data (not just
    /// hashes) -- for consumers like difficulty adjustment that need real
    /// block contents in canonical order rather than arrival order. Same
    /// motivation and same performance caveat as `recompute_ledger`.
    pub fn canonical_block_order(&self, tip: &BlockHash) -> Vec<VeloBlock> {
        self.get_linear_sort(tip)
            .iter()
            .filter_map(|hash| self.block_store.get(hash).cloned())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BlockHeader;

    fn create_mock_block(parents: Vec<BlockHash>) -> VeloBlock {
        VeloBlock {
            header: BlockHeader {
                timestamp: 100,
                parents,
                tx_merkle_root: [0u8; 32],
                nonce: 0,
                height: 0,
                difficulty_target: [0x0f; 32],
            },
            transactions: vec![],
            coinbase_miner_address: [0u8; 32],
            coinbase_miner_output: 0,
            coinbase_dev_address: [0u8; 32],
            coinbase_dev_output: 0,
        }
    }

    #[test]
    fn test_genesis_ghostdag_calculation() {
        let mut manager = GhostdagManager::new(3);
        let genesis_hash = [0u8; 32];
        let genesis_block = create_mock_block(vec![]);

        let result = manager.calculate_ghostdag_data(&genesis_block, genesis_hash);
        assert_eq!(result.blue_score, 0);
        assert!(result.selected_parent.is_none());
    }

    /// Regression test for real fork choice: given two competing branches
    /// off the same genesis, the branch with more blocks (and therefore a
    /// higher blue_score) must be selected as canonical -- not whichever
    /// branch happens to be passed first, and not whichever arrived most
    /// recently. This is the actual behavior that was missing before: the
    /// coloring math existed, but nothing used it to pick a winner.
    #[test]
    fn fork_choice_selects_the_heavier_branch() {
        let mut manager = GhostdagManager::new(3);

        let genesis_hash = [0u8; 32];
        let genesis = create_mock_block(vec![]);
        let genesis_data = manager.calculate_ghostdag_data(&genesis, genesis_hash);
        manager.block_store.insert(genesis_hash, genesis);
        manager.ghostdag_cache.insert(genesis_hash, genesis_data);

        // Branch A: a single block off genesis.
        let a1 = create_mock_block(vec![genesis_hash]);
        let a1_hash = [1u8; 32];
        let a1_data = manager.calculate_ghostdag_data(&a1, a1_hash);
        manager.block_store.insert(a1_hash, a1);
        manager.ghostdag_cache.insert(a1_hash, a1_data);

        // Branch B: two blocks off genesis -- strictly heavier than A.
        let b1 = create_mock_block(vec![genesis_hash]);
        let b1_hash = [2u8; 32];
        let b1_data = manager.calculate_ghostdag_data(&b1, b1_hash);
        manager.block_store.insert(b1_hash, b1);
        manager.ghostdag_cache.insert(b1_hash, b1_data);

        let b2 = create_mock_block(vec![b1_hash]);
        let b2_hash = [3u8; 32];
        let b2_data = manager.calculate_ghostdag_data(&b2, b2_hash);
        manager.block_store.insert(b2_hash, b2);
        manager.ghostdag_cache.insert(b2_hash, b2_data);

        // compute_tips must find exactly the two real tips (a1 and b2),
        // not genesis or b1 (both of which are now someone's parent).
        let mut tips = manager.compute_tips();
        tips.sort();
        let mut expected = vec![a1_hash, b2_hash];
        expected.sort();
        assert_eq!(tips, expected);

        // Fork choice must pick b2 (the heavier branch), regardless of
        // candidate ordering passed in.
        assert_eq!(manager.select_best_tip(&[a1_hash, b2_hash]), Some(b2_hash));
        assert_eq!(manager.select_best_tip(&[b2_hash, a1_hash]), Some(b2_hash));
        assert_eq!(manager.select_canonical_tip(), Some(b2_hash));
    }

    /// Regression test: when two tips have the identical blue_score (a
    /// genuine tie), selection must be deterministic -- every node must
    /// reach the same decision given the same tips, or the network can
    /// permanently fork on the tie-break alone.
    #[test]
    fn fork_choice_tie_break_is_deterministic() {
        let mut manager = GhostdagManager::new(3);
        let genesis_hash = [0u8; 32];
        let genesis = create_mock_block(vec![]);
        let genesis_data = manager.calculate_ghostdag_data(&genesis, genesis_hash);
        manager.block_store.insert(genesis_hash, genesis);
        manager.ghostdag_cache.insert(genesis_hash, genesis_data);

        // Two single-block branches off genesis: identical blue_score.
        let a1 = create_mock_block(vec![genesis_hash]);
        let a1_hash = [5u8; 32];
        let a1_data = manager.calculate_ghostdag_data(&a1, a1_hash);
        manager.block_store.insert(a1_hash, a1);
        manager.ghostdag_cache.insert(a1_hash, a1_data);

        let b1 = create_mock_block(vec![genesis_hash]);
        let b1_hash = [9u8; 32];
        let b1_data = manager.calculate_ghostdag_data(&b1, b1_hash);
        manager.block_store.insert(b1_hash, b1);
        manager.ghostdag_cache.insert(b1_hash, b1_data);

        // Lower hash ([5;32] < [9;32]) must win regardless of argument order.
        assert_eq!(manager.select_best_tip(&[a1_hash, b1_hash]), Some(a1_hash));
        assert_eq!(manager.select_best_tip(&[b1_hash, a1_hash]), Some(a1_hash));
    }

    /// Regression test: ledger balances must come from replaying the
    /// canonical linear order (genesis -> b1 -> b2), accumulating coinbase
    /// rewards correctly along the way -- not from however blocks happened
    /// to be inserted into the store. This is the actual property that was
    /// missing before recompute_ledger existed: the authoritative ledger
    /// is now a pure function of the DAG's canonical chain.
    #[test]
    fn recompute_ledger_matches_canonical_replay() {
        let mut manager = GhostdagManager::new(3);

        let genesis_hash = [0u8; 32];
        let genesis = create_mock_block(vec![]);
        let genesis_data = manager.calculate_ghostdag_data(&genesis, genesis_hash);
        manager.block_store.insert(genesis_hash, genesis);
        manager.ghostdag_cache.insert(genesis_hash, genesis_data);

        let miner = [7u8; 32];
        let dev = crate::DEV_TREASURY_ADDRESS;

        let mut b1 = create_mock_block(vec![genesis_hash]);
        b1.header.height = 1;
        let (miner_reward_1, dev_reward_1) = VeloBlock::calculate_subsidy_split(1);
        b1.coinbase_miner_address = miner;
        b1.coinbase_miner_output = miner_reward_1;
        b1.coinbase_dev_address = dev;
        b1.coinbase_dev_output = dev_reward_1;
        let b1_hash = [1u8; 32];
        let b1_data = manager.calculate_ghostdag_data(&b1, b1_hash);
        manager.block_store.insert(b1_hash, b1);
        manager.ghostdag_cache.insert(b1_hash, b1_data);

        let mut b2 = create_mock_block(vec![b1_hash]);
        b2.header.height = 2;
        let (miner_reward_2, dev_reward_2) = VeloBlock::calculate_subsidy_split(2);
        b2.coinbase_miner_address = miner;
        b2.coinbase_miner_output = miner_reward_2;
        b2.coinbase_dev_address = dev;
        b2.coinbase_dev_output = dev_reward_2;
        let b2_hash = [2u8; 32];
        let b2_data = manager.calculate_ghostdag_data(&b2, b2_hash);
        manager.block_store.insert(b2_hash, b2);
        manager.ghostdag_cache.insert(b2_hash, b2_data);

        let ledger = manager
            .recompute_ledger(&b2_hash)
            .expect("replay should succeed for a valid canonical chain");

        assert_eq!(ledger.balance(&miner), miner_reward_1 + miner_reward_2);
        assert_eq!(ledger.balance(&dev), dev_reward_1 + dev_reward_2);
    }
}
