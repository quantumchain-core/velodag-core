// vdag-consensus/src/pow.rs

use sha3::{Digest, Sha3_256};
use crate::ghostdag::Block;

/// Configuration structure for mining target thresholds
pub struct PowManager {
    // Defines how many leading zeros or how small the target hash value must be
    pub target_difficulty: [u8; 32], 
}

impl PowManager {
    pub fn new(target_difficulty: [u8; 32]) -> Self {
        Self { target_difficulty }
    }

    /// Serializes block parameters into a unique byte payload for hashing
    pub fn serialize_header(block: &Block) -> Vec<u8> {
        let mut header_bytes = Vec::new();
        
        // Append all parent hashes to make the proof unique to this DAG position
        for parent in &block.parents {
            header_bytes.extend_from_slice(parent);
        }
        
        header_bytes.extend_from_slice(&block.timestamp.to_le_bytes());
        header_bytes.extend_from_slice(&block.nonce.to_le_bytes());
        header_bytes
    }

    /// MINING ENGINE: Increments the block nonce until the header hash meets the target criteria
    pub fn mine_block(&self, block: &mut Block) -> [u8; 32] {
        loop {
            let header = Self::serialize_header(block);
            let mut hasher = Sha3_256::new();
            hasher.update(&header);
            let result = hasher.finalize();
            
            // Check if the resulting hash is below our target difficulty threshold
            if result.as_slice() <= self.target_difficulty.as_slice() {
                let mut valid_hash = [0u8; 32];
                valid_hash.copy_from_slice(result.as_slice());
                return valid_hash;
            }
            
            block.nonce += 1; // Try the next nonce mutation
        }
    }

    /// VERIFICATION ENGINE: Instant validation of incoming blocks propagated by peers
    pub fn verify_pow(&self, block: &Block, expected_hash: &[u8; 32]) -> bool {
        let header = Self::serialize_header(block);
        let mut hasher = Sha3_256::new();
        hasher.update(&header);
        let result = hasher.finalize();
        
        result.as_slice() == expected_hash && result.as_slice() <= self.target_difficulty.as_slice()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mine_and_verify_block() {
        // Setup an intentionally easy target for rapid testing (high byte values)
        let easy_target = [0x0F; 32]; 
        let pow_manager = PowManager::new(easy_target);

        let mut block = Block {
            hash: [0u8; 32],
            parents: vec![[0u8; 32]],
            timestamp: 1626000000,
            nonce: 0,
        };

        // Mine the block
        let computed_hash = pow_manager.mine_block(&mut block);
        
        // Assert that the nonce was modified and the hash meets the target criteria
        assert!(block.nonce > 0);
        assert!(computed_hash <= easy_target);

        // Verify that another node can validate it instantly without re-mining
        let is_valid = pow_manager.verify_pow(&block, &computed_hash);
        assert!(is_valid);
    }
}
