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
    use crate::BlockHeader;

    #[test]
    fn test_mine_and_verify_block() {
        // Setup an intentionally easy target for rapid testing (high byte values)
        let mut easy_target = [0xff; 32]; 
        easy_target[0] = 0x0f; // Limit the first byte to enforce a small difficulty challenge
        
        let pow_manager = PowManager::new(easy_target);

        // Instantiate using your proper VeloBlock data layout matching lib.rs
        let mut block = VeloBlock {
            header: BlockHeader {
                timestamp: 1626000000,
                parents: vec![[0u8; 32]],
                tx_merkle_root: [0u8; 32],
                nonce: 0,
                height: 1,
            },
            transactions: vec![],
            coinbase_miner_output: 83238,
            coinbase_dev_output: 0,
        };

        // Mine the block
        let computed_hash = pow_manager.mine_block(&mut block);
        
        // Assert that the nonce was modified and the hash meets target criteria
        assert!(block.header.nonce > 0);
        assert!(computed_hash <= easy_target);

        // Verify using the correct single-argument signature matching the implementation
        let is_valid = pow_manager.verify_pow(&block);
        assert!(is_valid);
    }
}
