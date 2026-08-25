use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::interval;

// Import our custom modules from the workspace crates
use vdag_consensus::{VeloBlock, BlockHeader};
use vdag_crypto::VeloKeyPair;

#[tokio::main]
async fn main() {
    println!("==================================================");
    println!("🚀 Initializing VeloDAG Core Node [Ticker: VDAG] ");
    println!("==================================================");

    // 1. Generate local Post-Quantum keys for this node instance
    println!("[🔒 Crypto Engine] Generating Dilithium2 keypair...");
    let node_keys = VeloKeyPair::generate();
    let miner_address = VeloKeyPair::derive_address(&node_keys.public_key);
    println!("[🔒 Crypto Engine] Local Miner Address Derived: {:?}", hex::encode(&miner_address[0..8]));

    // 2. Setup a mock Genesis/parent block pointer to start our DAG layout
    let mut current_tips: Vec<[u8; 32]> = vec![[0u8; 32]];
    let mut block_height = 0;

    // 3. Fire the asynchronous 1-second BlockDAG production heartbeat
    let mut block_timer = interval(Duration::from_secs(1));

    loop {
        block_timer.tick().await;
        block_height += 1;

        // Fetch current Unix timestamp in seconds
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // 4. Construct a new block header pointing to previous DAG tips
        let header = BlockHeader {
            timestamp,
            parents: current_tips.clone(),
            tx_merkle_root: [0u8; 32], // Empty block for local compilation testing
            nonce: 1001,               // Fixed arbitrary mock nonce for bootstrap loop
            height: block_height,
        };

        // 5. Calculate consensus splits (95% Miner / 5% Dev Tax) automatically
        let (miner_reward, dev_reward) = VeloBlock::calculate_subsidy_split(block_height);

        let mut next_block = VeloBlock {
            header,
            transactions: vec![],
            coinbase_miner_output: miner_reward,
            coinbase_dev_output: dev_reward,
        };

        // 6. Enforce strict consensus rules before broadcasting the simulated block
        if next_block.verify_coinbase_rewards() {
            let block_hash = next_block.calculate_hash();
            println!(
                "[⏱️  Height {:<5}] Block Minted! Hash: {}... | Miner: {} units | Dev Tax: {} units",
                block_height,
                hex::encode(&block_hash[0..6]),
                miner_reward,
                dev_reward
            );

            // Update local tips so the next minted block builds on top of this block
            current_tips = vec![block_hash];
        } else {
            eprintln!("[⚠️  ERROR] Block mutated or invalid reward distributions calculated. Dropping block.");
        }
    }
}

// Simple mini helper module to represent string hex for clean terminal logs
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
