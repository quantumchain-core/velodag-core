use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::interval;

// Import workspace modules
use vdag_consensus::{VeloBlock, BlockHeader, Mempool, Transaction, BlockchainStorage};
use vdag_crypto::VeloKeyPair;

#[tokio::main]
async fn main() {
    println!("==================================================");
    println!("🚀 Initializing VeloDAG Core Node [Ticker: VDAG] ");
    println!("==================================================");

    let miner_keys = VeloKeyPair::generate();
    let miner_address = VeloKeyPair::derive_address(&miner_keys.public_key);
    println!("[🔒 Crypto Engine] Local Miner Address Derived: 0x{}", hex::encode(&miner_address[0..6]));

    // --- PHASE 4 INITIALIZATION: Open Persistent Database Storage ---
    println!("[💾 Storage Engine] Initializing database ledger on disk...");
    let storage_engine = BlockchainStorage::open();

    let mut node_mempool = Mempool::new();
    let mut current_tips: Vec<[u8; 32]> = vec![[0u8; 32]];
    let mut block_height = 0;

    let mut block_timer = interval(Duration::from_secs(1));

    loop {
        block_timer.tick().await;
        block_height += 1;

        println!("--------------------------------------------------");
        
        // Simulating 3 incoming user transactions over API
        for i in 1..=3 {
            let sender_keys = VeloKeyPair::generate();
            let sender_addr = VeloKeyPair::derive_address(&sender_keys.public_key);
            let recipient_addr = [i; 32]; 
            let amount = (i as u64) * 500_000; 

            let mut tx_payload = Vec::new();
            tx_payload.extend_from_slice(&sender_addr);
            tx_payload.extend_from_slice(&recipient_addr);
            tx_payload.extend_from_slice(&amount.to_le_bytes());

            let signature = vdag_crypto::sign_message(&tx_payload, &sender_keys.secret_key);

            let simulated_tx = Transaction {
                sender: sender_addr,
                recipient: recipient_addr,
                amount,
                signature,
            };

            node_mempool.add_transaction(simulated_tx);
        }

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let transactions_to_confirm = node_mempool.drain_to_batch(10);
        let confirmed_tx_count = transactions_to_confirm.len();

        let header = BlockHeader {
            timestamp,
            parents: current_tips.clone(),
            tx_merkle_root: [0u8; 32], 
            nonce: 2026,
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
            
            // --- PHASE 4 STORAGE EXECUTION: Save Block To Disk permanently ---
            match storage_engine.save_block(&block_hash, &next_block) {
                Ok(_) => {
                    println!(
                        "[⏱️  Height {:<5}] Block Saved to Disk! Hash: {}... | Confirmed TXs: {}",
                        block_height,
                        hex::encode(&block_hash[0..6]),
                        confirmed_tx_count
                    );
                }
                Err(e) => {
                    eprintln!("[💾 Storage Engine Error] Failed to write block to ledger storage: {}", e);
                }
            }

            current_tips = vec![block_hash];
        }
    }
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
