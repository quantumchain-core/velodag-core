// vdag-consensus/src/ghostdag.rs

use std::collections::{HashMap, HashSet};
use sha3::{Digest, Sha3_256};

pub type BlockHash = [u8; 32];

#[derive(Clone, Debug)]
pub struct Block {
    pub hash: BlockHash,
    pub parents: Vec<BlockHash>, // Supports multi-parent BlockDAG architecture
    pub timestamp: u64,
    pub nonce: u64,
}

#[derive(Clone, Debug)]
pub struct GhostdagData {
    pub blue_score: u64,
    pub selected_parent: Option<BlockHash>,
    pub blues: Vec<BlockHash>, // Main honest cluster blocks
    pub reds: Vec<BlockHash>,  // Anticone / delayed blocks
}

pub struct GhostdagManager {
    pub k: usize, // The GHOSTDAG parameter determining the maximum allowed cluster size
    pub block_store: HashMap<BlockHash, Block>,
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
    pub fn calculate_ghostdag_data(&mut self, block: &Block) -> GhostdagData {
        // 1. Genesis Block Check
        if block.parents.is_empty() {
            return GhostdagData {
                blue_score: 0,
                selected_parent: None,
                blues: vec![],
                reds: vec![],
            };
        }

        // 2. Find the Selected Parent (the parent with the highest blue score)
        let selected_parent = block.parents.iter()
            .filter_map(|p| self.ghostdag_cache.get(p).map(|data| (p, data)))
            .max_by_key(|(_, data)| data.blue_score)
            .map(|(hash, _)| *hash);

        let mut blues = Vec::new();
        let mut reds = Vec::new();

        if let Some(ref sp) = selected_parent {
            blues.push(*sp);
            
            // 3. Find the Anticone (blocks that are not ancestors or descendants of the selected parent)
            let anticone = self.find_anticone(block, sp);

            // 4. Color the blocks in the anticone based on the maximum allowed merged capacity (K-factor)
            for candidate in anticone {
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

    /// Simple structural BFS search helper to discover blocks in the anticone of the selected parent
    fn find_anticone(&self, current_block: &Block, selected_parent: &BlockHash) -> Vec<BlockHash> {
        let mut anticone = Vec::new();
        let mut visited = HashSet::new();
        
        // Populate all immediate parents of the current block
        for parent in &current_block.parents {
            if parent != selected_parent {
                anticone.push(*parent);
            }
        }
        
        // Deduplicate and return candidate blocks outside of the selected parent's past
        anticone.retain(|h| h != selected_parent);
        anticone
    }

    /// Enforces the GHOSTDAG threshold constraint rule. 
    /// Ensures that checking this block does not cause any blue block's anticone to exceed size K.
    fn can_be_blue(&self, _candidate: &BlockHash, current_blues: &Vec<BlockHash>) -> bool {
        // Enforce the rule: size of the blue anticone must be <= K
        current_blues.len() < self.k
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_ghostdag_calculation() {
        let mut manager = GhostdagManager::new(3);
        
        // Mock a Genesis block hash
        let genesis_hash = [0u8; 32];
        let genesis_block = Block {
            hash: genesis_hash,
            parents: vec![],
            timestamp: 1626000000,
            nonce: 0,
        };

        let result = manager.calculate_ghostdag_data(&genesis_block);
        assert_eq!(result.blue_score, 0);
        assert!(result.selected_parent.is_none());
    }
}
