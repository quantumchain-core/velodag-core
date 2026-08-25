pub mod ghostdag;
pub mod pow;

use sha3::{Digest, Sha3_256};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use ghostdag::GhostdagData;

// --- CONSTANTS FOR VELODAG EMISSION (20-Year Supply Blueprint) ---
pub const INITIAL_BLOCK_REWARD: u64 = 83_238; 
pub const DEV_TAX_PERCENTAGE: u64 = 5;       
pub const BLOCKS_PER_ERA: u64 = 126_144_000; 

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub timestamp: u64,
    pub parents: Vec<[u8; 32]>, 
    pub tx_merkle_root: [u8; 32],
    pub nonce: u64,
    pub height: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub sender: [u8; 32],      
    pub recipient: [u8; 32],   
    pub amount: u64,           
    pub signature: Vec<u8>,    
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        let era = height / BLOCKS_PER_ERA;
        let total_subsidy = INITIAL_BLOCK_REWARD >> era;
        
        if total_subsidy == 0 {
            return (0, 0); 
        }

        let dev_share = (total_subsidy * DEV_TAX_PERCENTAGE) / 100;
        let miner_share = total_subsidy - dev_share;

        (miner_share, dev_share)
    }

    /// Strict protocol gatekeeper. Validates that the block rewards perfectly match consensus rules.
    pub fn verify_coinbase_rewards(&self) -> bool {
        let (expected_miner, expected_dev) = Self::calculate_subsidy_split(self.header.height);
        self.coinbase_miner_output == expected_miner && self.coinbase_dev_output == expected_dev
    }
}

#[derive(Debug, Clone)]
pub struct Mempool {
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

pub struct BlockchainStorage {
    blocks_tree: sled::Tree,
    ghostdag_tree: sled::Tree,
    db: sled::Db,
}

impl BlockchainStorage {
    /// Opens local storage and configures tree structures for explicit state isolation
    pub fn open() -> Self {
        let db = sled::open("velodag_ledger_data").expect("Failed to initialize storage database context");
        
        // Open named sub-trees to separate raw blocks from consensus scoring metadata
        let blocks_tree = db.open_tree(b"blocks").expect("Failed to open blocks data tree");
        let ghostdag_tree = db.open_tree(b"ghostdag").expect("Failed to open ghostdag metadata tree");
        
        BlockchainStorage { 
            blocks_tree, 
            ghostdag_tree,
            db 
        }
    }

    /// Serializes a VeloBlock into raw binary bytes and writes it permanently to disk
    pub fn save_block(&self, block_hash: &[u8; 32], block: &VeloBlock) -> Result<(), Box<dyn std::error::Error>> {
        let serialized_bytes = bincode::serialize(block)?;
        self.blocks_tree.insert(block_hash, serialized_bytes)?;
        self.db.flush()?; 
        Ok(())
    }

    /// Reads database bytes from disk using a block hash key and deserializes it back into a VeloBlock
    pub fn load_block(&self, block_hash: &[u8; 32]) -> Result<Option<VeloBlock>, Box<dyn std::error::Error>> {
        if let Some(bytes) = self.blocks_tree.get(block_hash)? {
            let block: VeloBlock = bincode::deserialize(&bytes)?;
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }

    /// Persists GHOSTDAG coloring meta-states directly to database disk blocks
    pub fn save_ghostdag_data(&self, block_hash: &[u8; 32], data: &GhostdagData) -> Result<(), Box<dyn std::error::Error>> {
        let serialized_bytes = bincode::serialize(data)?;
        self.ghostdag_tree.insert(block_hash, serialized_bytes)?;
        self.db.flush()?;
        Ok(())
    }

    /// Loads historical GHOSTDAG color frameworks mapping to an existing block hash identification string
    pub fn load_ghostdag_data(&self, block_hash: &[u8; 32]) -> Result<Option<GhostdagData>, Box<dyn std::error::Error>> {
        if let Some(bytes) = self.ghostdag_tree.get(block_hash)? {
            let data: GhostdagData = bincode::deserialize(&bytes)?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod consensus_tests {
    use super::*;

    #[test]
    fn test_subsidy_values_and_halving() {
        // First Era verification
        let (miner_0, dev_0) = VeloBlock::calculate_subsidy_split(0);
        assert_eq!(miner_0 + dev_0, INITIAL_BLOCK_REWARD);
        assert_eq!(dev_0, (INITIAL_BLOCK_REWARD * DEV_TAX_PERCENTAGE) / 100);

        // Verification after first 4-year cycle threshold
        let (miner_era1, dev_era1) = VeloBlock::calculate_subsidy_split(BLOCKS_PER_ERA + 1);
        assert_eq!(miner_era1 + dev_era1, INITIAL_BLOCK_REWARD >> 1);
    }
}
