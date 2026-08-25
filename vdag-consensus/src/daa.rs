// vdag-consensus/src/daa.rs

use crate::VeloBlock;

pub struct DifficultyManager {
    pub target_block_interval: u64, // Target time between blocks in seconds (e.g., 1)
    pub window_size: usize,         // Number of historical blocks to sample (e.g., 16 or 32)
}

impl DifficultyManager {
    pub fn new(target_block_interval: u64, window_size: usize) -> Self {
        Self {
            target_block_interval,
            window_size,
        }
    }

    /// Calculates the next required difficulty target using a moving average window of blocks
    pub fn calculate_next_target(
        &self,
        recent_blocks: &[VeloBlock],
        current_target: [u8; 32],
    ) -> [u8; 32] {
        // Fallback to current target if there are not enough samples to build an accurate average
        if recent_blocks.len() < self.window_size {
            return current_target;
        }

        // Take only the most recent blocks bounded by the window size configuration
        let window = &recent_blocks[recent_blocks.len() - self.window_size..];

        // 1. Calculate actual time elapsed over the sample window
        let earliest_timestamp = window.first().unwrap().header.timestamp;
        let latest_timestamp = window.last().unwrap().header.timestamp;
        
        let actual_time_elapsed = if latest_timestamp > earliest_timestamp {
            latest_timestamp - earliest_timestamp
        } else {
            1 // Safeguard against clock drift or identical timestamps
        };

        // 2. Calculate the expected time target for this number of blocks
        let expected_time = self.target_block_interval * (self.window_size as u64 - 1);

        // 3. Compute adjustment ratio
        // Big number conversion mechanics to adjust the target hash array without float types
        let mut target_u256 = u256_from_bytes(current_target);

        // If the blocks were mined too fast (actual < expected), target needs to shrink (harder)
        // If the blocks were mined too slow (actual > expected), target needs to expand (easier)
        target_u256 = (target_u256 * actual_time_elapsed) / expected_time;

        // Ensure target doesn't overflow max bounds (an array of 0xFF represents absolute absolute minimum difficulty)
        let max_target = [0xff; 32];
        let calculated_bytes = bytes_from_u256(target_u256);

        if calculated_bytes > max_target {
            max_target
        } else {
            calculated_bytes
        }
    }
}

// --- HELPER COMPUTE MATRICES FOR BIG INT MANIPULATION ---

fn u256_from_bytes(bytes: [u8; 32]) -> Vec<u64> {
    // Simple 4-limb representation of big integers to execute math across 32-byte arrays safely
    let mut limbs = vec![0u64; 4];
    for i in 0..4 {
        let start = i * 8;
        let mut limb_bytes = [0u8; 8];
        limb_bytes.copy_from_slice(&bytes[start..start + 8]);
        limbs[i] = u64::from_le_bytes(limb_bytes);
    }
    limbs
}

fn bytes_from_u256(limbs: Vec<u64>) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for i in 0..4 {
        let start = i * 8;
        let limb_bytes = limbs[i].to_le_bytes();
        bytes[start..start + 8].copy_from_slice(&limb_bytes);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BlockHeader;

    fn create_mock_block_with_time(timestamp: u64) -> VeloBlock {
        VeloBlock {
            header: BlockHeader {
                timestamp,
                parents: vec![],
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
    fn test_difficulty_shrinks_when_mining_too_fast() {
        let manager = DifficultyManager::new(1, 4); // 1 second target, window size of 4 blocks
        let base_target = [0x7f; 32];

        // Create 4 blocks that arrived 0 seconds apart (impossible speed)
        let blocks = vec![
            create_mock_block_with_time(1000),
            create_mock_block_with_time(1000),
            create_mock_block_with_time(1000),
            create_mock_block_with_time(1001),
        ];

        let next_target = manager.calculate_next_target(&blocks, base_target);
        
        // Target value should be smaller (closer to 0) which implies a harder difficulty gate
        assert!(next_target < base_target);
    }
}
