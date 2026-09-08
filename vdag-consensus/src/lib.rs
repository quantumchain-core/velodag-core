pub mod daa;
pub mod ghostdag;
pub mod pow;

use ghostdag::GhostdagData;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::collections::{HashMap, HashSet};

// --- CONSTANTS FOR VELODAG EMISSION (20-Year Supply Blueprint) ---
pub const INITIAL_BLOCK_REWARD: u64 = 83_238;
pub const DEV_TAX_PERCENTAGE: u64 = 5;
pub const BLOCKS_PER_ERA: u64 = 126_144_000;
pub const DEV_TREASURY_ADDRESS: [u8; 32] = [0xdd; 32];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub timestamp: u64,
    pub parents: Vec<[u8; 32]>,
    pub tx_merkle_root: [u8; 32],
    pub nonce: u64,
    pub height: u64,
    pub difficulty_target: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub sender: [u8; 32],
    pub recipient: [u8; 32],
    pub amount: u64,
    pub public_key: Vec<u8>,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VeloBlock {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    pub coinbase_miner_address: [u8; 32],
    pub coinbase_miner_output: u64,
    pub coinbase_dev_address: [u8; 32],
    pub coinbase_dev_output: u64,
}

impl VeloBlock {
    /// Returns the canonical bytes signed by a transaction sender.
    pub fn transaction_payload(tx: &Transaction) -> Vec<u8> {
        let mut payload = Vec::with_capacity(32 + 32 + 8);
        payload.extend_from_slice(&tx.sender);
        payload.extend_from_slice(&tx.recipient);
        payload.extend_from_slice(&tx.amount.to_le_bytes());
        payload
    }

    /// Computes a deterministic transaction commitment for the block header.
    pub fn transaction_merkle_root(transactions: &[Transaction]) -> [u8; 32] {
        if transactions.is_empty() {
            return [0u8; 32];
        }

        let mut layer: Vec<[u8; 32]> = transactions.iter().map(Self::transaction_id).collect();

        while layer.len() > 1 {
            let mut next = Vec::with_capacity(layer.len().div_ceil(2));
            for pair in layer.chunks(2) {
                let right = pair.get(1).unwrap_or(&pair[0]);
                let mut hasher = Sha3_256::new();
                hasher.update(pair[0]);
                hasher.update(right);
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&hasher.finalize());
                next.push(hash);
            }
            layer = next;
        }

        layer[0]
    }

    /// Checks that the header commits to exactly the transactions in the block.
    pub fn verify_transaction_merkle_root(&self) -> bool {
        self.header.tx_merkle_root == Self::transaction_merkle_root(&self.transactions)
    }

    pub fn transaction_id(tx: &Transaction) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        hasher.update(Self::transaction_payload(tx));
        hasher.update(&tx.public_key);
        hasher.update(&tx.signature);
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hasher.finalize());
        hash
    }

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
        hasher.update(self.header.difficulty_target);
        hasher.update(self.coinbase_miner_address);
        hasher.update(self.coinbase_dev_address);

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LedgerState {
    balances: HashMap<[u8; 32], u64>,
    confirmed_transactions: HashSet<[u8; 32]>,
}

impl LedgerState {
    pub fn balance(&self, address: &[u8; 32]) -> u64 {
        self.balances.get(address).copied().unwrap_or(0)
    }

    /// Atomically validates and applies a block's transfers and coinbase outputs.
    pub fn apply_block(&mut self, block: &VeloBlock) -> Result<(), String> {
        let mut balances = self.balances.clone();
        let mut confirmed_transactions = self.confirmed_transactions.clone();

        if block.header.height == 0 {
            if !block.transactions.is_empty()
                || block.coinbase_miner_output != 0
                || block.coinbase_dev_output != 0
            {
                return Err("genesis block contains spendable outputs".into());
            }
            self.balances = balances;
            self.confirmed_transactions = confirmed_transactions;
            return Ok(());
        }

        if !block.verify_coinbase_rewards() {
            return Err("invalid coinbase amount".into());
        }
        if block.coinbase_dev_address != DEV_TREASURY_ADDRESS {
            return Err("invalid development treasury address".into());
        }

        let mut block_transactions = HashSet::new();
        for tx in &block.transactions {
            let tx_id = VeloBlock::transaction_id(tx);
            if !block_transactions.insert(tx_id) || !confirmed_transactions.insert(tx_id) {
                return Err("duplicate transaction".into());
            }

            if tx.amount == 0 {
                return Err("transaction amount must be positive".into());
            }

            let sender_balance = balances.get(&tx.sender).copied().unwrap_or(0);
            let remaining = sender_balance
                .checked_sub(tx.amount)
                .ok_or_else(|| "insufficient balance".to_string())?;
            balances.insert(tx.sender, remaining);
            let recipient_balance = balances.get(&tx.recipient).copied().unwrap_or(0);
            balances.insert(
                tx.recipient,
                recipient_balance
                    .checked_add(tx.amount)
                    .ok_or_else(|| "recipient balance overflow".to_string())?,
            );
        }

        let miner_balance = balances
            .get(&block.coinbase_miner_address)
            .copied()
            .unwrap_or(0);
        balances.insert(
            block.coinbase_miner_address,
            miner_balance
                .checked_add(block.coinbase_miner_output)
                .ok_or_else(|| "miner balance overflow".to_string())?,
        );

        let dev_balance = balances
            .get(&block.coinbase_dev_address)
            .copied()
            .unwrap_or(0);
        balances.insert(
            block.coinbase_dev_address,
            dev_balance
                .checked_add(block.coinbase_dev_output)
                .ok_or_else(|| "treasury balance overflow".to_string())?,
        );

        self.balances = balances;
        self.confirmed_transactions = confirmed_transactions;
        Ok(())
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
        let tx_id = VeloBlock::transaction_id(&tx);

        if self.pending_transactions.contains_key(&tx_id) {
            return false;
        }

        self.pending_transactions.insert(tx_id, tx);
        true
    }

    /// Pulls transactions out of the queue to package them cleanly inside a 1-second block
    pub fn drain_to_batch(&mut self, max_batch_size: usize) -> Vec<Transaction> {
        let mut batch = Vec::new();
        let keys: Vec<[u8; 32]> = self
            .pending_transactions
            .keys()
            .cloned()
            .take(max_batch_size)
            .collect();

        for key in keys {
            if let Some(tx) = self.pending_transactions.remove(&key) {
                batch.push(tx);
            }
        }
        batch
    }
}

impl Default for Mempool {
    fn default() -> Self {
        Self::new()
    }
}

pub struct BlockchainStorage {
    blocks_tree: sled::Tree,
    ghostdag_tree: sled::Tree,
    ledger_tree: sled::Tree,
    db: sled::Db,
}

impl BlockchainStorage {
    /// Opens local storage and configures tree structures for explicit state isolation
    pub fn open() -> Self {
        let db = sled::open("velodag_ledger_data")
            .expect("Failed to initialize storage database context");

        // Open named sub-trees to separate raw blocks from consensus scoring metadata
        let blocks_tree = db
            .open_tree(b"blocks")
            .expect("Failed to open blocks data tree");
        let ghostdag_tree = db
            .open_tree(b"ghostdag")
            .expect("Failed to open ghostdag metadata tree");
        let ledger_tree = db
            .open_tree(b"ledger")
            .expect("Failed to open ledger state tree");

        BlockchainStorage {
            blocks_tree,
            ghostdag_tree,
            ledger_tree,
            db,
        }
    }

    /// Serializes a VeloBlock into raw binary bytes and writes it permanently to disk
    pub fn save_block(
        &self,
        block_hash: &[u8; 32],
        block: &VeloBlock,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let serialized_bytes = bincode::serialize(block)?;
        self.blocks_tree.insert(block_hash, serialized_bytes)?;
        self.db.flush()?;
        Ok(())
    }

    /// Reads database bytes from disk using a block hash key and deserializes it back into a VeloBlock
    pub fn load_block(
        &self,
        block_hash: &[u8; 32],
    ) -> Result<Option<VeloBlock>, Box<dyn std::error::Error>> {
        if let Some(bytes) = self.blocks_tree.get(block_hash)? {
            let block: VeloBlock = bincode::deserialize(&bytes)?;
            Ok(Some(block))
        } else {
            Ok(None)
        }
    }

    /// Loads every persisted block for startup recovery, ordered by height.
    pub fn load_all_blocks(&self) -> Result<Vec<VeloBlock>, Box<dyn std::error::Error>> {
        let mut blocks = Vec::new();
        for entry in self.blocks_tree.iter() {
            let (_, bytes) = entry?;
            blocks.push(bincode::deserialize(&bytes)?);
        }
        blocks.sort_by_key(|block: &VeloBlock| block.header.height);
        Ok(blocks)
    }

    pub fn save_ledger_state(&self, state: &LedgerState) -> Result<(), Box<dyn std::error::Error>> {
        let serialized_bytes = bincode::serialize(state)?;
        self.ledger_tree.insert(b"current", serialized_bytes)?;
        self.db.flush()?;
        Ok(())
    }

    pub fn load_ledger_state(&self) -> Result<Option<LedgerState>, Box<dyn std::error::Error>> {
        if let Some(bytes) = self.ledger_tree.get(b"current")? {
            Ok(Some(bincode::deserialize(&bytes)?))
        } else {
            Ok(None)
        }
    }

    /// Persists GHOSTDAG coloring meta-states directly to database disk blocks
    pub fn save_ghostdag_data(
        &self,
        block_hash: &[u8; 32],
        data: &GhostdagData,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let serialized_bytes = bincode::serialize(data)?;
        self.ghostdag_tree.insert(block_hash, serialized_bytes)?;
        self.db.flush()?;
        Ok(())
    }

    /// Loads historical GHOSTDAG color frameworks mapping to an existing block hash identification string
    pub fn load_ghostdag_data(
        &self,
        block_hash: &[u8; 32],
    ) -> Result<Option<GhostdagData>, Box<dyn std::error::Error>> {
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

    #[test]
    fn transaction_merkle_root_commits_to_transaction_contents() {
        let first = Transaction {
            sender: [1u8; 32],
            recipient: [2u8; 32],
            amount: 10,
            public_key: vec![3u8; 4],
            signature: vec![4u8; 4],
        };
        let second = Transaction {
            sender: [5u8; 32],
            recipient: [6u8; 32],
            amount: 20,
            public_key: vec![7u8; 4],
            signature: vec![8u8; 4],
        };

        let root = VeloBlock::transaction_merkle_root(&[first.clone(), second.clone()]);
        assert_ne!(root, [0u8; 32]);
        assert_ne!(
            root,
            VeloBlock::transaction_merkle_root(&[
                first,
                Transaction {
                    amount: 21,
                    ..second
                }
            ])
        );
        assert_eq!(VeloBlock::transaction_merkle_root(&[]), [0u8; 32]);
    }

    #[test]
    fn ledger_state_tracks_rewards_and_rejects_invalid_spends() {
        let miner = [1u8; 32];
        let recipient = [2u8; 32];
        let mut state = LedgerState::default();
        let (miner_reward, dev_reward) = VeloBlock::calculate_subsidy_split(1);

        let reward_block = VeloBlock {
            header: BlockHeader {
                timestamp: 1,
                parents: vec![[0u8; 32]],
                tx_merkle_root: [0u8; 32],
                nonce: 0,
                height: 1,
                difficulty_target: [0xff; 32],
            },
            transactions: vec![],
            coinbase_miner_address: miner,
            coinbase_miner_output: miner_reward,
            coinbase_dev_address: DEV_TREASURY_ADDRESS,
            coinbase_dev_output: dev_reward,
        };
        state.apply_block(&reward_block).unwrap();
        assert_eq!(state.balance(&miner), miner_reward);

        let tx = Transaction {
            sender: miner,
            recipient,
            amount: miner_reward,
            public_key: vec![],
            signature: vec![],
        };
        let (next_miner_reward, next_dev_reward) = VeloBlock::calculate_subsidy_split(2);
        let spend_block = VeloBlock {
            header: BlockHeader {
                timestamp: 2,
                parents: vec![reward_block.calculate_hash()],
                tx_merkle_root: VeloBlock::transaction_merkle_root(std::slice::from_ref(&tx)),
                nonce: 0,
                height: 2,
                difficulty_target: [0xff; 32],
            },
            transactions: vec![tx.clone()],
            coinbase_miner_address: miner,
            coinbase_miner_output: next_miner_reward,
            coinbase_dev_address: DEV_TREASURY_ADDRESS,
            coinbase_dev_output: next_dev_reward,
        };
        state.apply_block(&spend_block).unwrap();
        assert_eq!(state.balance(&miner), next_miner_reward);
        assert_eq!(state.balance(&recipient), miner_reward);
        assert!(state.apply_block(&spend_block).is_err());
    }
}
