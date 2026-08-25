use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::interval;

// Import our workspace modules
use vdag_consensus::{VeloBlock, BlockHeader, Mempool, Transaction};
use vdag_crypto::VeloKeyPair;

#[tokio::main]
async fn main() {
    println!("==================================================");
    println!("🚀 Initializing VeloDAG Core Node [Ticker: VDAG] ");
    println!("==================================================");

    // 1. Generate local Post-Quantum keys for the BlockDAG miner node
    let miner_keys = VeloKeyPair::generate();
    let miner_address = VeloKeyPair::derive_address(&miner_keys.public_key);
    println!("[🔒 Crypto Engine] Local Miner Address Derived: 0x{}", hex::encode(&miner_address[0..6]));

    // 2. Instantiate our Phase 2 Mempool memory queue
    let mut node_mempool = Mempool::new();
    let mut current_tips: Vec<[u8; 32]> = vec![[0u8; 32]];
    let mut block_height = 0;

    // 3. Setup a strict 1-second interval loop timer for the ledger block generation
    let mut block_timer = interval(Duration::from_secs(1));

    loop {
        block_timer.tick().await;
        block_height += 1;

        println!("--------------------------------------------------");
        
        // --- SIMULATOR: Mock JSON-RPC Incoming Transaction Influx ---
        // Every second, we simulate 3 new random users generating and signing quantum transactions
        println!("[📡 RPC Server] Simulating 3 incoming user transactions over API...");
        for i in 1..=3 {
            let sender_keys = VeloKeyPair::generate();
            let sender_addr = VeloKeyPair::derive_address(&sender_keys.public_key);
            let recipient_addr = [i; 32]; // Mock recipient destination wallet address
            let amount = (i as u64) * 500_000; // Mock transaction amount units

            // Create a payload byte string representing the unique transaction data
            let mut tx_payload = Vec::new();
            tx_payload.extend_from_slice(&sender_addr);
            tx_payload.extend_from_slice(&recipient_addr);
            tx_payload.extend_from_slice(&amount.to_le_bytes());

            // Sign the payload using post-quantum CRYSTALS-Dilithium2 math
            let signature = vdag_crypto::sign_message(&tx_payload, &sender_keys.secret_key);

            let simulated_tx = Transaction {
                sender: sender_addr,
                recipient: recipient_addr,
                amount,
                signature,
            };

            // Inject the validated transaction into the active core node memory pool
            node_mempool.add_transaction(simulated_tx);
        }
        println!("[📥 Mempool Status] Pending Queue Size: {} txs", node_mempool.pending_transactions.len());

        // 4. Extract current unified Unix time
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // 5. Drain up to a maximum threshold of 10 unconfirmed transactions from memory for this block slot
        let transactions_to_confirm = node_mempool.drain_to_batch(10);
        let confirmed_tx_count = transactions_to_confirm.len();

        // 6. Build out the current BlockDAG node slot header metadata
        let header = BlockHeader {
            timestamp,
            parents: current_tips.clone(),
            tx_merkle_root: [0u8; 32], 
            nonce: 2026,
            height: block_height,
        };

        // 7. Extract the protocol block reward distribution structures
        let (miner_reward, dev_reward) = VeloBlock::calculate_subsidy_split(block_height);

        let next_block = VeloBlock {
            header,
            transactions: transactions_to_confirm,
            coinbase_miner_output: miner_reward,
            coinbase_dev_output: dev_reward,
        };

        // 8. Run strict consensus rules verification before locking the block into the local ledger state
        if next_block.verify_coinbase_rewards() {
            let block_hash = next_block.calculate_hash();
            println!(
                "[⏱️  Height {:<5}] Block Minted! Hash: {}... | Confirmed TXs: {} | Remaining Mempool: {}",
                block_height,
                hex::encode(&block_hash[0..6]),
                confirmed_tx_count,
                node_mempool.pending_transactions.len()
            );

            // Cascade the DAG pointers downward
            current_tips = vec![block_hash];
        } else {
            eprintln!("[⚠️  CRITICAL ERROR] Block validation rule breach encountered. Dropping block.");
        }
    }
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
