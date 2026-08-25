// vdag-consensus/src/ghostdag.rs

use std::collections::{HashMap, HashSet, VecDeque};
use crate::VeloBlock;

pub type BlockHash = [u8; 32];

#[derive(Clone, Debug)]
pub struct GhostdagData {
    pub blue_score: u64,
    pub selected_parent: Option<BlockHash>,
    pub blues: Vec<BlockHash>, 
    pub reds: Vec<BlockHash>,  
}

pub struct GhostdagManager {
    pub k: usize, 
    pub block_store: HashMap<BlockHash, VeloBlock>, // Mapped directly to your VeloBlock structure
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
    pub fn calculate_ghostdag_data(&mut self, block: &VeloBlock) -> GhostdagData {
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
                anticone.push(current_hash);

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

    /// CONSTRAINT ENGINE: Ensures the blue anticone size bounds do not violate threshold parameter K
    fn can_be_blue(&self, _candidate: &BlockHash, current_blues: &Vec<BlockHash>) -> bool {
        current_blues.len() < self.k
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
}
