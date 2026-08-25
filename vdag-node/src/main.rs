pub mod network;
// vdag-node/src/main.rs

use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::interval;

use vdag_consensus::{
    VeloBlock, BlockHeader, Mempool, Transaction, BlockchainStorage,
    ghostdag::GhostdagManager, pow::PowManager, daa::DifficultyManager
};
use vdag_crypto::VeloKeyPair;

#[tokio::main]
async fn main() {
    let storage_engine = BlockchainStorage::open();
    let args: Vec<String> = env::args().collect();

    // 1. Process Explorer CLI Flags
    if args.len() > 2 && args[1] == "--get-block" {
        run_explorer(&storage_engine, &args[2]);
        return;
    }

    println!("==================================================");
    println!("🚀 Initializing VeloDAG Core Node [Ticker: VDAG] ");
    println!("==================================================");

    let mut ghostdag = GhostdagManager::new(3); 
    let difficulty_manager = DifficultyManager::new(1, 4); 
    let mut current_difficulty_target = [0x0f; 32]; 
    let mut block_history: Vec<VeloBlock> = Vec::new();
    let genesis_hash = [0u8; 32];

    // 2. Genesis Initialization Engine Check
    match storage_engine.load_block(&genesis_hash) {
        Ok(None) => {
            println!("[🧱 Genesis Engine] Minting Genesis Block 0...");
            let genesis_block = create_block(vec![], 0, 0, 0);
            let genesis_dag_data = ghostdag.calculate_ghostdag_data(&genesis_block, genesis_hash);
            
            storage_engine.save_block(&genesis_hash, &genesis_block).unwrap();
            storage_engine.save_ghostdag_data(&genesis_hash, &genesis_dag_data).unwrap();
            
            ghostdag.block_store.insert(genesis_hash, genesis_block.clone());
            ghostdag.ghostdag_cache.insert(genesis_hash, genesis_dag_data);
            block_history.push(genesis_block);
        }
        Ok(Some(genesis_blk)) => {
            println!("[💾 Storage Engine] Resuming ledger context.");
            let genesis_dag_data = ghostdag.calculate_ghostdag_data(&genesis_blk, genesis_hash);
            ghostdag.block_store.insert(genesis_hash, genesis_blk.clone());
            ghostdag.ghostdag_cache.insert(genesis_hash, genesis_dag_data);
            block_history.push(genesis_blk);
        }
        Err(e) => eprintln!("[💾 Storage Engine Error] Initialization error: {}", e),
    }

    let miner_keys = VeloKeyPair::generate();
    let miner_address = VeloKeyPair::derive_address(&miner_keys.public_key);
    println!("[🔒 Crypto Engine] Local Miner Live: 0x{}", encode_hex(&miner_address[0..6]));

    let mut node_mempool = Mempool::new();
    let mut current_tips = vec![genesis_hash];
    let mut block_height = 0;
    let mut block_timer = interval(Duration::from_secs(1));

    // 3. Core 1-Second Runtime BlockDAG Processing Loop
    loop {
        block_timer.tick().await;
        block_height += 1;
        println!("--------------------------------------------------");
        
        simulate_transactions(&mut node_mempool);

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let (miner_reward, dev_reward) = VeloBlock::calculate_subsidy_split(block_height);
        
        let mut next_block = create_block(current_tips.clone(), block_height, miner_reward, dev_reward);
        next_block.header.timestamp = timestamp;
        next_block.transactions = node_mempool.drain_to_batch(10);

        if next_block.verify_coinbase_rewards() {
            let pow_manager = PowManager::new(current_difficulty_target);
            let block_hash = pow_manager.mine_block(&mut next_block);
            let dag_data = ghostdag.calculate_ghostdag_data(&next_block, block_hash);

            ghostdag.block_store.insert(block_hash, next_block.clone());
            ghostdag.ghostdag_cache.insert(block_hash, dag_data.clone());
            block_history.push(next_block.clone());

            if storage_engine.save_block(&block_hash, &next_block).is_ok() {
                let _ = storage_engine.save_ghostdag_data(&block_hash, &dag_data);
                println!(
                    "[⏱️ Height {:<5}] Block Mined! Hash: {}... Nonce: {}",
                    block_height, encode_hex(&block_hash[0..8]), next_block.header.nonce
                );
            }

            current_difficulty_target = difficulty_manager.calculate_next_target(&block_history, current_difficulty_target);
            current_tips = vec![block_hash];
        }
    }
}

// --- OUT-OF-LOOP ISOLATED HELPER UTILITIES ---

fn create_block(parents: Vec<[u8; 32]>, height: u64, miner: u64, dev: u64) -> VeloBlock {
    VeloBlock {
        header: BlockHeader { timestamp: 0, parents, tx_merkle_root: [0u8; 32], nonce: 0, height },
        transactions: vec![],
        coinbase_miner_output: miner,
        coinbase_dev_output: dev,
    }
}

fn simulate_transactions(mempool: &mut Mempool) {
    for i in 1..=3 {
        let sender_keys = VeloKeyPair::generate();
        let sender_addr = VeloKeyPair::derive_address(&sender_keys.public_key);
        let recipient_addr = [i; 32]; 
        let amount = (i as u64) * 500_000; 

        let mut payload = Vec::new();
        payload.extend_from_slice(&sender_addr);
        payload.extend_from_slice(&recipient_addr);
        payload.extend_from_slice(&amount.to_le_bytes());

        let signature = vdag_crypto::sign_message(&payload, &sender_keys.secret_key);
        mempool.add_transaction(Transaction { sender: sender_addr, recipient: recipient_addr, amount, signature });
    }
}

fn run_explorer(storage: &BlockchainStorage, hash_str: &str) {
    if let Ok(hash_vec) = decode_hex(hash_str) {
        if hash_vec.len() == 32 {
            let mut target_hash = [0u8; 32];
            target_hash.copy_from_slice(&hash_vec);

            if let Ok(Some(block)) = storage.load_block(&target_hash) {
                println!("\n==================================================");
                println!("🧱 VELODAG BLOCK METADATA EXPLORER");
                println!("==================================================");
                println!("• Height: {} | Nonce: {}", block.header.height, block.header.nonce);
                println!("• Confirmed TXs: {}", block.transactions.len());
                println!("• Miner Subsidy: {} | Dev Tax: {}", block.coinbase_miner_output, block.coinbase_dev_output);
                
                if let Ok(Some(dag)) = storage.load_ghostdag_data(&target_hash) {
                    println!("• GHOSTDAG Score: {}", dag.blue_score);
                    println!("• Blue Count: {} | Red Count: {}", dag.blues.len(), dag.reds.len());
                }
                println!("==================================================\n");
            }
        }
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn decode_hex(s: &str) -> Result<Vec<u8>, std::num::ParseIntError> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16)).collect()
}
