use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::interval;

// Import workspace modules
use vdag_consensus::{VeloBlock, BlockHeader, Mempool, Transaction, BlockchainStorage};
use vdag_crypto::VeloKeyPair;

#[tokio::main]
async fn main() {
    // 1. Initialize persistent storage engine context
    let storage_engine = BlockchainStorage::open();

    // --- PHASE 6 ENHANCEMENT: CLI BLOCK EXPLORER PARSER ---
    let args: Vec<String> = env::args().collect();
    if args.len() > 2 && args[1] == "--get-block" {
        println!("🔍 [VeloDAG Explorer] Querying database ledger for target hash key...");
        
        // Decode the input string hex argument back into raw database bytes
        if let Ok(target_hash_vec) = hex::decode_str(&args[2]) {
            if target_hash_vec.len() == 32 {
                let mut target_hash = [0u8; 32];
                target_hash.copy_from_slice(&target_hash_vec);

                match storage_engine.load_block(&target_hash) {
                    Ok(Some(block)) => {
                        println!("\n==================================================");
                        println!("🧱 VELODAG BLOCK DATA METADATA LAYER");
                        println!("==================================================");
                        println!("• Block Height : {}", block.header.height);
                        println!("• Timestamp    : {}", block.header.timestamp);
                        println!("• Nonce Target : {}", block.header.nonce);
                        println!("• Confirmed TXs: {}", block.transactions.len());
                        println!("• Miner Subsidy: {} atomic units", block.coinbase_miner_output);
                        println!("• Dev Tax Levy : {} atomic units", block.coinbase_dev_output);
                        println!("• Parent DAG Tips:");
                        for parent in &block.header.parents {
                            println!("   └── 0x{}", hex::encode(&parent[0..8]));
                        }
                        println!("==================================================\n");
                        return;
                    }
                    Ok(None) => {
                        println!("❌ [Explorer Error] Target block hash was not found inside the active database ledger.");
                        return;
                    }
                    Err(e) => {
                        eprintln!("❌ [Database Error] Failed to read database storage sector: {}", e);
                        return;
                    }
                }
            }
        }
        println!("❌ [Explorer Error] Invalid hash argument length. Must be a 64-character valid hex string.");
        return;
    }

    // --- STANDARD BOOT ROUTINE IF NO EXPLORER FLAGS ARE PASSED ---
    println!("==================================================");
    println!("🚀 Initializing VeloDAG Core Node [Ticker: VDAG] ");
    println!("==================================================");

    // GENESIS CHECK
    let genesis_hash = [0u8; 32];
    match storage_engine.load_block(&genesis_hash) {
        Ok(None) => {
            println!("[🧱 Genesis Engine] Empty ledger detected! Minting Genesis Block 0...");
            let genesis_header = BlockHeader {
                timestamp: 1782384000,
                parents: vec![],
                tx_merkle_root: [0u8; 32],
                nonce: 88888,
                height: 0,
            };
            let genesis_block = VeloBlock {
                header: genesis_header,
                transactions: vec![],
                coinbase_miner_output: 0,
                coinbase_dev_output: 0,
            };
            storage_engine.save_block(&genesis_hash, &genesis_block).unwrap();
            println!("[🧱 Genesis Engine] VeloDAG Genesis Block successfully committed to disk!");
        }
        Ok(Some(_)) => {
            println!("[💾 Storage Engine] Genesis block verification confirmed. Resuming ledger context.");
        }
        Err(e) => {
            eprintln!("[💾 Storage Engine Error] Initialization error: {}", e);
        }
    }

    let miner_keys = VeloKeyPair::generate();
    let miner_address = VeloKeyPair::derive_address(&miner_keys.public_key);
    println!("[🔒 Crypto Engine] Local Miner Address Live: 0x{}", hex::encode(&miner_address[0..6]));

    let mut node_mempool = Mempool::new();
    let mut current_tips: Vec<[u8; 32]> = vec![genesis_hash];
    let mut block_height = 0;

    let mut block_timer = interval(Duration::from_secs(1));

    loop {
        block_timer.tick().await;
        block_height += 1;

        println!("--------------------------------------------------");
        
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
            
            match storage_engine.save_block(&block_hash, &next_block) {
                Ok(_) => {
                    println!(
                        "[⏱️  Height {:<5}] Block Saved! Hash: {}...",
                        block_height,
                        hex::encode(&block_hash[0..8])
                    );
                }
                Err(e) => {
                    eprintln!("[💾 Storage Engine Error] Failed to write block: {}", e);
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

    pub fn decode_str(s: &str) -> Result<Vec<u8>, std::num::ParseIntError> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
            .collect()
    }
}
