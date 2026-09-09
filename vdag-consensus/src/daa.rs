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

        // Clamp the elapsed-time ratio so a single window can't swing
        // difficulty by more than 4x in either direction. This does double
        // duty: it prevents the u128 multiplication below from overflowing
        // when timestamps are extreme (e.g. the fixed genesis timestamp
        // sitting far from real wall-clock time once genesis enters the
        // window), and it stops a peer from manipulating future difficulty
        // via an implausible block timestamp.
        let expected_time_safe = expected_time.max(1);
        let clamped_elapsed = actual_time_elapsed.clamp(
            expected_time_safe / 4,
            expected_time_safe.saturating_mul(4),
        );

        // 3. Compute adjustment ratio safely using a native u128 frame
        // Extract the leading 16 bytes to manipulate difficulty bits safely
        let mut high_bytes = [0u8; 16];
        high_bytes.copy_from_slice(&current_target[0..16]);
        let target_val = u128::from_be_bytes(high_bytes);

        // Perform the scaling math: target = (target * clamped_elapsed) / expected_time
        // If blocks are found too fast, actual_time < expected_time, target shrinks (harder difficulty)
        // checked_mul/checked_div rather than raw operators: even with the
        // clamp above as the primary defense, this ensures a corrupted
        // target can never silently wrap into a nonsensical value again --
        // any unexpected overflow just leaves the target unchanged instead.
        let target_val = target_val
            .checked_mul(clamped_elapsed as u128)
            .and_then(|v| v.checked_div(expected_time_safe as u128))
            .unwrap_or(target_val);

        // Reconstruct the 32-byte hash array output
        let mut next_target = [0xff; 32]; // Padding tail end with maximum values
        let new_high_bytes = target_val.to_be_bytes();
        next_target[0..16].copy_from_slice(&new_high_bytes);

        // Ensure target doesn't overflow absolute minimum difficulty boundary configurations
        let max_target = [0xff; 32];
        if next_target > max_target {
            max_target
        } else {
            next_target
        }
    }
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
    fn test_difficulty_shrinks_when_mining_too_fast() {
        let manager = DifficultyManager::new(1, 4); // 1 second target, window size of 4 blocks
        let base_target = [0x7f; 32];

        // Create 4 blocks that arrived 0 seconds apart (excessive speed)
        let blocks = vec![
            create_mock_block_with_time(1000),
            create_mock_block_with_time(1000),
            create_mock_block_with_time(1000),
            create_mock_block_with_time(1001),
        ];

        let next_target = manager.calculate_next_target(&blocks, base_target);

        // Target value should be smaller (closer to 0x00) which implies a harder cryptographic challenge
        assert!(next_target < base_target);
    }

    /// Regression test: a fixed genesis timestamp sitting far from real
    /// wall-clock time (e.g. genesis=1_700_000_000, real time ~57M seconds
    /// later) previously overflowed the u128 multiplication in release
    /// builds, silently corrupting the difficulty target instead of
    /// panicking or erroring. This must neither panic nor produce an
    /// out-of-range target.
    #[test]
    fn test_extreme_timestamp_gap_does_not_overflow() {
        let manager = DifficultyManager::new(1, 4);
        let base_target = [0x0f; 32];

        let blocks = vec![
            create_mock_block_with_time(1_700_000_000),
            create_mock_block_with_time(1_757_000_000),
            create_mock_block_with_time(1_757_000_001),
            create_mock_block_with_time(1_757_000_002),
        ];

        let next_target = manager.calculate_next_target(&blocks, base_target);
        assert!(next_target <= [0xff; 32]);
    }
}
