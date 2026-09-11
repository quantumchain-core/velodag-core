// vdag-consensus/src/test_vectors.rs
//
// DETERMINISTIC CONSENSUS TEST VECTORS
//
// What makes this different from the unit tests scattered through the rest
// of this crate: every expected value below was computed *independently*,
// in Python using hashlib.sha3_256 and the same arithmetic formulas, NOT
// by running this Rust code and copying its output. That distinction
// matters -- a test that asserts `compute() == compute()` proves the
// function is deterministic and didn't panic; it proves nothing about
// whether the function computes the *correct* thing. Checking against an
// independently-computed value is what actually catches "the code is
// self-consistent but wrong."
//
// This file exists as a single, clearly-labeled place an external
// reviewer/auditor can go to check "does this implementation produce the
// values the spec says it should" without having to read the rest of the
// crate. If any of these ever fail, treat it as a genuine consensus
// regression, not a test to "fix" by updating the expected value --
// the expected values here are the ground truth, computed independently
// of this codebase.
//
// Known gap, flagged rather than hidden: GHOSTDAG blue/red coloring itself
// (calculate_ghostdag_data) is NOT covered by an independent vector here --
// faithfully reimplementing the anticone/k-cluster algorithm a second time
// in Python, correctly, is a substantial undertaking in its own right and
// risks being subtly wrong in a way that would make a "golden" vector
// actively misleading rather than merely absent. The existing tests in
// ghostdag.rs (fork_choice_selects_the_heavier_branch, etc.) cover its
// *behavior* well; an independently-verified GHOSTDAG vector is a good
// candidate for a dedicated future round rather than being rushed here.
//
// This entire module is test-only -- see the `#[cfg(test)] mod
// test_vectors;` declaration in lib.rs.

use crate::daa::DifficultyManager;
use crate::{BlockHeader, Transaction, VeloBlock, DEVNET_NETWORK_ID, MAINNET_NETWORK_ID, TESTNET_NETWORK_ID};

// --- Genesis hashes -----------------------------------------------
// Computed independently: SHA3-256(timestamp_LE || height_LE(0) ||
// tx_merkle_root(zero) || nonce_LE(0) || difficulty_target(0x0f*32) ||
// coinbase_miner_address(zero) || coinbase_dev_address(0xdd*32))

const DEVNET_GENESIS_HASH: [u8; 32] = [
    0xfd, 0xd0, 0x30, 0xa7, 0xbf, 0x0b, 0x77, 0x05, 0x7e, 0x45, 0x88, 0x7d, 0xc2, 0xda, 0xfa, 0x78,
    0x34, 0x6e, 0x25, 0x72, 0xab, 0x98, 0x8f, 0x81, 0x89, 0x3e, 0x05, 0x14, 0x77, 0x0a, 0x0e, 0x1a,
];
const TESTNET_GENESIS_HASH: [u8; 32] = [
    0x9f, 0x37, 0x7a, 0x2a, 0x69, 0xfb, 0x5c, 0xc7, 0x53, 0xb6, 0x77, 0x8c, 0xbe, 0xad, 0x63, 0xe7,
    0x47, 0xe5, 0xca, 0xda, 0xd8, 0x38, 0xa1, 0x92, 0x69, 0xa9, 0xc3, 0x81, 0x20, 0xce, 0x8b, 0xe0,
];
const MAINNET_GENESIS_HASH: [u8; 32] = [
    0x37, 0x5f, 0x80, 0xc9, 0xc9, 0x72, 0x34, 0x19, 0x30, 0x9a, 0x3d, 0x16, 0x52, 0x51, 0x18, 0x7d,
    0xf1, 0xb8, 0xb6, 0xae, 0x01, 0xf0, 0x40, 0x56, 0xd3, 0x35, 0xae, 0x5f, 0x9f, 0xec, 0xca, 0x4b,
];

#[test]
fn vector_genesis_hashes_per_network() {
    assert_eq!(
        crate::fixed_genesis_hash_for_network(DEVNET_NETWORK_ID),
        DEVNET_GENESIS_HASH
    );
    assert_eq!(
        crate::fixed_genesis_hash_for_network(TESTNET_NETWORK_ID),
        TESTNET_GENESIS_HASH
    );
    assert_eq!(
        crate::fixed_genesis_hash_for_network(MAINNET_NETWORK_ID),
        MAINNET_GENESIS_HASH
    );
}

// --- Non-genesis block hash ----------------------------------------
// A single hand-constructed block, height 1, parented on devnet genesis,
// with a fixed sample tx_merkle_root/nonce/difficulty/miner address. Hash
// computed independently the same way as above, just with a real parent
// and non-zero fields.

const SAMPLE_TX_MERKLE_ROOT: [u8; 32] = [0xab; 32];
const SAMPLE_DIFFICULTY_TARGET: [u8; 32] = [
    0x00, 0x00, 0x0f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
];
const SAMPLE_MINER_ADDR: [u8; 32] = [0x11; 32];
const SAMPLE_BLOCK1_HASH: [u8; 32] = [
    0xc2, 0xeb, 0xec, 0xbc, 0xb3, 0xff, 0x14, 0x88, 0x62, 0x3a, 0xb4, 0x19, 0x2f, 0xec, 0x2f, 0x65,
    0xc0, 0x17, 0x70, 0xa8, 0xc2, 0xfe, 0x44, 0x6f, 0x89, 0x2d, 0x9d, 0x96, 0xe4, 0x65, 0x11, 0xd1,
];

#[test]
fn vector_non_genesis_block_hash() {
    let block = VeloBlock {
        header: BlockHeader {
            timestamp: 1_757_000_000,
            parents: vec![DEVNET_GENESIS_HASH],
            tx_merkle_root: SAMPLE_TX_MERKLE_ROOT,
            nonce: 424_242,
            height: 1,
            difficulty_target: SAMPLE_DIFFICULTY_TARGET,
        },
        transactions: vec![],
        coinbase_miner_address: SAMPLE_MINER_ADDR,
        coinbase_miner_output: 0,
        coinbase_dev_address: crate::DEV_TREASURY_ADDRESS,
        coinbase_dev_output: 0,
    };
    assert_eq!(block.calculate_hash(), SAMPLE_BLOCK1_HASH);
}

// --- Transaction id + merkle root -----------------------------------
// Two transactions with empty public_key/signature (matching the pattern
// used elsewhere in this crate's tests). Computed independently: tx_id =
// SHA3-256(sender || recipient || amount_LE || nonce_LE), merkle_root(2
// leaves) = SHA3-256(id1 || id2).

const TX1_ID: [u8; 32] = [
    0xef, 0x8e, 0x19, 0x94, 0x91, 0xe2, 0xa0, 0xb4, 0x58, 0x1b, 0xa2, 0x51, 0x16, 0xba, 0x1f, 0xed,
    0x85, 0x6e, 0x40, 0xfe, 0xde, 0x56, 0xc0, 0x80, 0xb6, 0xf4, 0x10, 0x08, 0x50, 0xa4, 0xc4, 0x58,
];
const TX2_ID: [u8; 32] = [
    0x7f, 0x28, 0xc4, 0xf3, 0xa7, 0x58, 0x03, 0x4c, 0x4e, 0x10, 0x9d, 0x2e, 0x77, 0xa5, 0xc1, 0xeb,
    0x3f, 0xb0, 0xf3, 0xdd, 0xf9, 0x24, 0x4e, 0x0c, 0x14, 0xfa, 0x1a, 0x28, 0x96, 0xdc, 0x72, 0x6b,
];
const MERKLE_ROOT_2TX: [u8; 32] = [
    0x10, 0xe1, 0xe7, 0x90, 0xc3, 0x27, 0x94, 0x3a, 0x8a, 0xad, 0x40, 0xf5, 0x47, 0xf5, 0x52, 0x5f,
    0x86, 0x8d, 0xdd, 0x4e, 0x40, 0x2d, 0xc2, 0x59, 0xa5, 0xf3, 0x00, 0x00, 0x4a, 0xa1, 0x81, 0x84,
];

#[test]
fn vector_transaction_id_and_merkle_root() {
    let tx1 = Transaction {
        sender: [1u8; 32],
        recipient: [2u8; 32],
        amount: 5000,
        nonce: 0,
        public_key: vec![],
        signature: vec![],
    };
    let tx2 = Transaction {
        sender: [3u8; 32],
        recipient: [4u8; 32],
        amount: 7500,
        nonce: 0,
        public_key: vec![],
        signature: vec![],
    };

    assert_eq!(VeloBlock::transaction_id(&tx1), TX1_ID);
    assert_eq!(VeloBlock::transaction_id(&tx2), TX2_ID);
    assert_eq!(
        VeloBlock::transaction_merkle_root(&[tx1, tx2]),
        MERKLE_ROOT_2TX
    );
}

// --- Subsidy split (block reward halving schedule) ------------------
// Computed independently: era = height / BLOCKS_PER_ERA,
// total = INITIAL_BLOCK_REWARD >> era, dev = total*5/100 (integer),
// miner = total - dev.

#[test]
fn vector_subsidy_split_across_era_boundary() {
    assert_eq!(VeloBlock::calculate_subsidy_split(1), (79_077, 4_161));
    assert_eq!(
        VeloBlock::calculate_subsidy_split(crate::BLOCKS_PER_ERA - 1),
        (79_077, 4_161)
    );
    // Exact halving boundary: total reward must exactly halve.
    assert_eq!(
        VeloBlock::calculate_subsidy_split(crate::BLOCKS_PER_ERA),
        (39_539, 2_080)
    );
    assert_eq!(
        VeloBlock::calculate_subsidy_split(crate::BLOCKS_PER_ERA * 2),
        (19_769, 1_040)
    );
}

// --- Fee distribution -------------------------------------------------
// Computed independently: burn = fee/2 = 50, distributed = 50,
// dev = distributed*5/100 = 2, miner = distributed - dev = 48.

#[test]
fn vector_fee_distribution() {
    assert_eq!(VeloBlock::fee_distribution(), (48, 2, 50));
}

// --- Difficulty adjustment: extreme timestamp gap --------------------
// Same scenario as daa::tests::test_extreme_timestamp_gap_does_not_overflow,
// but checked against an exact independently-computed expected value
// rather than only "doesn't panic and stays in range" -- this is
// strictly stronger evidence that the clamp+checked-arithmetic fix
// produces the *correct* result, not just a safe one.

const DAA_BASE_TARGET: [u8; 32] = [0x0f; 32];
const DAA_NEXT_TARGET_EXPECTED: [u8; 32] = [
    0x3c, 0x3c, 0x3c, 0x3c, 0x3c, 0x3c, 0x3c, 0x3c, 0x3c, 0x3c, 0x3c, 0x3c, 0x3c, 0x3c, 0x3c, 0x3c,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
];

fn mock_block_with_time(timestamp: u64) -> VeloBlock {
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
        coinbase_dev_address: crate::DEV_TREASURY_ADDRESS,
        coinbase_dev_output: 0,
    }
}

#[test]
fn vector_difficulty_adjustment_extreme_gap() {
    let manager = DifficultyManager::new(1, 4);
    let blocks = vec![
        mock_block_with_time(1_700_000_000),
        mock_block_with_time(1_757_000_000),
        mock_block_with_time(1_757_000_001),
        mock_block_with_time(1_757_000_002),
    ];
    let next_target = manager.calculate_next_target(&blocks, DAA_BASE_TARGET);
    assert_eq!(next_target, DAA_NEXT_TARGET_EXPECTED);
}
