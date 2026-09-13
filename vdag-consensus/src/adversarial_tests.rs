// vdag-consensus/src/adversarial_tests.rs
//
// Property-based tests: generate a wide space of inputs (random bytes,
// extreme numeric values) rather than relying only on hand-picked cases.
// This is what actually found the shift-overflow bug in
// calculate_subsidy_split -- no hand-picked height would have exercised
// era >= 64, but a property sweeping the full u64 range found it
// immediately.
//
// Deliberately scoped to proptest (a plain `cargo test`-compatible crate)
// rather than a full cargo-fuzz harness. cargo-fuzz would explore the
// input space more thoroughly, but needs a nightly toolchain and a
// separate `cargo fuzz run` invocation outside the normal `cargo test`
// loop this project's CI is built around -- introducing that tooling risk
// wasn't judged worth it for this round. A dedicated cargo-fuzz harness
// for the gossip/RPC deserialization paths remains a good candidate for a
// future round, run manually rather than in CI.

use proptest::prelude::*;

use crate::daa::DifficultyManager;
use crate::{BlockHeader, Transaction, VeloBlock, BLOCKS_PER_ERA, DEV_TREASURY_ADDRESS};

fn mock_block_with_timestamp(timestamp: u64) -> VeloBlock {
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
        coinbase_dev_address: DEV_TREASURY_ADDRESS,
        coinbase_dev_output: 0,
    }
}

proptest! {
    /// calculate_subsidy_split must never panic for any possible u64
    /// height, including the extreme end of the range no hand-picked test
    /// case previously exercised.
    #[test]
    fn subsidy_split_never_panics_across_full_height_range(height in any::<u64>()) {
        let (miner, dev) = VeloBlock::calculate_subsidy_split(height);
        prop_assert!(miner.checked_add(dev).is_some());
    }

    /// Regression check for the exact bug found and fixed: once era >= 64,
    /// the reward must be reported as fully exhausted (0, 0). Without the
    /// explicit guard, Rust's release-mode shift-amount masking would
    /// silently produce a wrong, nonzero value instead.
    #[test]
    fn subsidy_split_is_zero_once_era_exceeds_bit_width(era_offset in 0u64..1000) {
        let era = 64 + era_offset;
        let height = era * BLOCKS_PER_ERA; // bounded well under u64::MAX for this input range
        let (miner, dev) = VeloBlock::calculate_subsidy_split(height);
        prop_assert_eq!((miner, dev), (0, 0));
    }

    /// Deserializing arbitrary, likely-malformed bytes as a VeloBlock must
    /// never panic -- a peer sending garbage over gossip should only ever
    /// produce a clean Err, never crash the node.
    #[test]
    fn velo_block_deserialize_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        let result = std::panic::catch_unwind(|| {
            let _ = bincode::deserialize::<VeloBlock>(&bytes);
        });
        prop_assert!(result.is_ok(), "deserializing arbitrary bytes as VeloBlock must not panic");
    }

    /// Same property for Transaction -- the other untrusted-bytes
    /// deserialization target reachable directly from network input.
    #[test]
    fn transaction_deserialize_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        let result = std::panic::catch_unwind(|| {
            let _ = bincode::deserialize::<Transaction>(&bytes);
        });
        prop_assert!(result.is_ok(), "deserializing arbitrary bytes as Transaction must not panic");
    }

    /// calculate_next_target must never panic across a wide random sweep
    /// of timestamps and target bytes -- broader confirmation that the
    /// clamp + checked-arithmetic fix from the earlier overflow bug holds
    /// generally, not just for the one specific scenario it was written
    /// against. Block count varies 0..8 to cover both the "not enough
    /// history yet" early-return path and the real windowed computation.
    #[test]
    fn difficulty_adjustment_never_panics(
        timestamps in prop::collection::vec(any::<u64>(), 0..8),
        target_bytes in prop::array::uniform32(any::<u8>()),
    ) {
        let manager = DifficultyManager::new(1, 4);
        let blocks: Vec<VeloBlock> = timestamps.into_iter().map(mock_block_with_timestamp).collect();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            manager.calculate_next_target(&blocks, target_bytes)
        }));
        prop_assert!(
            result.is_ok(),
            "calculate_next_target must not panic for any timestamp/target combination"
        );
    }
}
