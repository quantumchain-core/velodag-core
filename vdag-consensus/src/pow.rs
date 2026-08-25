// vdag-consensus/src/pow.rs

use crate::VeloBlock;

pub struct PowManager {
    pub target_difficulty: [u8; 32], 
}

impl PowManager {
    pub fn new(target_difficulty: [u8; 32]) -> Self {
        Self { target_difficulty }
    }

    /// MINING ENGINE: Modifies the block header nonce until the resulting hash satisfies difficulty criteria
    pub fn mine_block(&self, block: &mut VeloBlock) -> [u8; 32] {
        loop {
            let hash = block.calculate_hash();
            
            // Check if hash matches difficulty threshold boundaries
            if hash <= self.target_difficulty {
                return hash;
            }
            
            block.header.nonce += 1;
        }
    }

    /// VERIFICATION ENGINE: Used by peer nodes to validate inbound blocks instantly
    pub fn verify_pow(&self, block: &VeloBlock) -> bool {
        let hash = block.calculate_hash();
        hash <= self.target_difficulty
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
