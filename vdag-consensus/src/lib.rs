use sha3::{Digest, Sha3_256};

// --- CONSTANTS FOR VALODAG EMISSION (As calculated in our 20-Year Blueprint) ---
pub const INITIAL_BLOCK_REWARD: u64 = 83_238; // 0.083238 VDAG (represented in atomic units/satoshis to avoid floating-point errors)
pub const DEV_TAX_PERCENTAGE: u64 = 5;       // 5% Consensus-enforced Development Treasury Tax
pub const BLOCKS_PER_ERA: u64 = 126_144_000; // Halving happens exactly every 4 years (31,536,000 seconds/year * 4)

#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub timestamp: u64,
    pub parents: Vec<[u8; 32]>, // Directed Acyclic Graph pointers (allows multiple parent blocks)
    pub tx_merkle_root: [u8; 32],
    pub nonce: u64,
    pub height: u64,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub sender: [u8; 32],      // Dilithium2 public key hash (Wallet Address)
    pub recipient: [u8; 32],   // Recipient wallet address
    pub amount: u64,           // Amount in atomic units
    pub signature: Vec<u8>,    // CRYSTALS-Dilithium2 signature signature payload
}

#[derive(Debug, Clone)]
pub struct VeloBlock {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    pub coinbase_miner_output: u64,
    pub coinbase_dev_output: u64,
}

impl VeloBlock {
    /// Computes a unique cryptographic SHA3-256 identification hash for the block
    pub fn calculate_hash(&self) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        hasher.update(self.header.timestamp.to_le_bytes());
        hasher.update(self.header.height.to_le_bytes());
        for parent in &self.header.parents {
            hasher.update(parent);
        }
        hasher.update(self.header.tx_merkle_root);
        hasher.update(self.header.nonce.to_le_bytes());
        
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Calculated dynamic block emissions and enforces the 5% dev tax split
    pub fn calculate_subsidy_split(height: u64) -> (u64, u64) {
        // Calculate how many halvings have occurred based on the block height
        let era = height / BLOCKS_PER_ERA;
        
        // Right shift operation to cleanly half the reward each era without float values
        let total_subsidy = INITIAL_BLOCK_REWARD >> era;
        
        if total_subsidy == 0 {
            return (0, 0); // Hard-cap limit hit, max supply achieved
        }

        // Calculate the consensus tax splits
        let dev_share = (total_subsidy * DEV_TAX_PERCENTAGE) / 100;
        let miner_share = total_subsidy - dev_share;

        (miner_share, dev_share)
    }

    /// Strict protocol gatekeeper. Validates that the block rewards perfectly match consensus rules.
    pub fn verify_coinbase_rewards(&self) -> bool {
        let (expected_miner, expected_dev) = Self::calculate_subsidy_split(self.header.height);
        
        // Block will be instantly rejected by network peers if miner modifies rewards
        self.coinbase_miner_output == expected_miner && self.coinbase_dev_output == expected_dev
    }
}
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Mempool {
    // Stores unconfirmed transactions keyed by their unique identifier
    pub pending_transactions: HashMap<[u8; 32], Transaction>,
}

impl Mempool {
    pub fn new() -> Self {
        Mempool {
            pending_transactions: HashMap::new(),
        }
    }

    /// Inserts a newly received transaction into the unconfirmed queue
    pub fn add_transaction(&mut self, tx: Transaction) -> bool {
        let mut hasher = Sha3_256::new();
        hasher.update(&tx.sender);
        hasher.update(&tx.recipient);
        hasher.update(&tx.amount.to_le_bytes());
        hasher.update(&tx.signature);
        
        let mut tx_id = [0u8; 32];
        tx_id.copy_from_slice(&hasher.finalize());

        // Avoid transaction duplicates
        if self.pending_transactions.contains_key(&tx_id) {
            return false;
        }

        self.pending_transactions.insert(tx_id, tx);
        true
    }

    /// Pulls transactions out of the queue to package them cleanly inside a 1-second block
    pub fn drain_to_batch(&mut self, max_batch_size: usize) -> Vec<Transaction> {
        let mut batch = Vec::new();
        let keys: Vec<[u8; 32]> = self.pending_transactions.keys().cloned().take(max_batch_size).collect();
        
        for key in keys {
            if let Some(tx) = self.pending_transactions.remove(&key) {
                batch.push(tx);
            }
        }
        batch
    }
}

