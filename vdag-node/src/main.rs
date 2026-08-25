use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::interval;

use vdag_consensus::{VeloBlock, BlockHeader, Mempool, Transaction};
use vdag_crypto::VeloKeyPair;

#[tokio::main]
async fn main() {
    println!("==================================================");
    println!("🚀 Initializing VeloDAG Core Node [Ticker: VDAG] ");
    println!("==================================================");

    let node_keys = VeloKeyPair::generate();
    let miner_address = VeloKeyPair::derive_address(&node_keys.public_key);
    println!("[🔒 Crypto Engine] Local Miner Address Live.");

    // Initialize our empty transaction mempool pool
    let mut node_mempool = Mempool::new();
    let mut current_tips: Vec<[u8; 32]> = vec![[0u8; 32]];
    let mut block_height = 0;

    let mut block_timer = interval(Duration::from_secs(1));

    loop {
        block_timer.tick().await;
        block_height += 1;

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        // 1. Drain up to 1000 pending user transactions from memory for this block
        let transactions_to_confirm = node_mempool.drain_to_batch(1000);
        let tx_count = transactions_to_confirm.len();

        let header = BlockHeader {
            timestamp,
            parents: current_tips.clone(),
            tx_merkle_root: [0u8; 32], 
            nonce: 2026, // Updated epoch marker for baseline verification
            height: block_height,
        };

        let (miner_reward, dev_reward) = VeloBlock::calculate_subsidy_split(block_height);

        let next_block = VeloBlock {
            header,
            transactions: transactions_to_confirm,
            coinbase_miner_output: miner_reward,
            coinbase_dev_output: dev_reward,
        };

        if next_block.verify_coinbase_rewards() {
            let block_hash = next_block.calculate_hash();
            println!(
                "[⏱️ Height {:<5}] Block Minted! Hash: {}... | TXs: {:<4} | Dev Tax Verified",
                block_height,
                hex::encode(&block_hash[0..6]),
                tx_count
            );
            current_tips = vec![block_hash];
        }
    }
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
