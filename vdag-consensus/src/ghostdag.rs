// vdag-consensus/src/ghostdag.rs

use std::collections::{HashMap, HashSet, VecDeque};
use serde::{Serialize, Deserialize};
use crate::VeloBlock;

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
    pub fn calculate_ghostdag_data(&mut self, block: &VeloBlock, block_hash: BlockHash) -> GhostdagData {
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
        let selected_parent = block.header.parents.iter()
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
    fn find_anticone(&self, current_block: &VeloBlock, selected_parent: &BlockHash) -> Vec<BlockHash> {
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
            if !blue_past.contains(candidate) && !candidate_past.contains(blue) && *blue != *candidate {
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
    pub fn get_linear_sort(&self, tip_hash: &BlockHash) -> Vec<BlockHash> {
        let mut order = Vec::new();
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
            },
            transactions: vec![],
            coinbase_miner_output: 0,
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
}
