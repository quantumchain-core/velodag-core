use std::path::Path;

use vdag_crypto::shielded::ShieldedTransaction;

pub const SHIELDED_WEIGHT: u64 = 10;

#[derive(Clone, Debug)]
pub struct ShieldedSnapshot {
    commitments: Vec<[u8; 32]>,
    nullifiers: Vec<[u8; 32]>,
}

/// Persistent shielded-state index. Commitments are append-only and nullifiers
/// are inserted atomically with the transaction-level checks in the RPC path.
/// Canonical replay should construct a fresh instance from the canonical block
/// set rather than applying orphan-branch mutations to the live instance.
pub struct ShieldedState {
    pub commitment_tree: Vec<[u8; 32]>,
    pub spent_nullifiers: sled::Tree,
    commitment_store: sled::Tree,
}

impl Clone for ShieldedState {
    fn clone(&self) -> Self {
        Self {
            commitment_tree: self.commitment_tree.clone(),
            spent_nullifiers: self.spent_nullifiers.clone(),
            commitment_store: self.commitment_store.clone(),
        }
    }
}

impl ShieldedState {
    pub fn root(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"velodag/shielded-root/v1");
        for commitment in &self.commitment_tree {
            hasher.update(commitment);
        }
        *hasher.finalize().as_bytes()
    }

    #[cfg(test)]
    pub fn test_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("velodag-shielded-{}", std::process::id()))
    }

    pub fn new(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let db = sled::open(path)?;
        let spent_nullifiers = db.open_tree("shielded_nullifiers")?;
        let commitment_store = db.open_tree("shielded_commitments")?;
        let mut commitment_tree = Vec::new();
        for item in commitment_store.iter() {
            let (_, value) = item?;
            if value.len() != 32 {
                return Err("invalid shielded commitment length in storage".into());
            }
            let mut commitment = [0u8; 32];
            commitment.copy_from_slice(&value);
            commitment_tree.push(commitment);
        }
        Ok(Self {
            commitment_tree,
            spent_nullifiers,
            commitment_store,
        })
    }

    pub fn add_commitment(
        &mut self,
        commitment: [u8; 32],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let index = self.commitment_tree.len() as u64;
        let key = index.to_be_bytes();
        self.commitment_store.insert(key, &commitment)?;
        self.commitment_store.flush()?;
        self.commitment_tree.push(commitment);
        Ok(())
    }

    pub fn is_nullifier_spent(
        &self,
        nullifier: &[u8; 32],
    ) -> Result<bool, Box<dyn std::error::Error>> {
        Ok(self.spent_nullifiers.contains_key(nullifier)?)
    }

    pub fn snapshot(&self) -> Result<ShieldedSnapshot, Box<dyn std::error::Error>> {
        let mut nullifiers = Vec::new();
        for item in self.spent_nullifiers.iter() {
            let (key, _) = item?;
            if key.len() != 32 {
                return Err("invalid shielded nullifier length in storage".into());
            }
            let mut nullifier = [0u8; 32];
            nullifier.copy_from_slice(&key);
            nullifiers.push(nullifier);
        }
        Ok(ShieldedSnapshot {
            commitments: self.commitment_tree.clone(),
            nullifiers,
        })
    }

    pub fn restore(
        &mut self,
        snapshot: ShieldedSnapshot,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.commitment_store.clear()?;
        self.spent_nullifiers.clear()?;
        for (index, commitment) in snapshot.commitments.iter().enumerate() {
            self.commitment_store
                .insert((index as u64).to_be_bytes(), commitment)?;
        }
        for nullifier in &snapshot.nullifiers {
            self.spent_nullifiers.insert(nullifier, &[1u8])?;
        }
        self.commitment_store.flush()?;
        self.spent_nullifiers.flush()?;
        self.commitment_tree = snapshot.commitments;
        Ok(())
    }

    pub fn add_nullifier(
        &self,
        nullifier: [u8; 32],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let result = self.spent_nullifiers.compare_and_swap(
            nullifier,
            None as Option<&[u8]>,
            Some(&[1u8]),
        )?;
        if result.is_err() {
            return Err("shielded nullifier already spent".into());
        }
        self.spent_nullifiers.flush()?;
        Ok(())
    }

    pub fn apply_transaction(
        &mut self,
        transaction: &ShieldedTransaction,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = self.snapshot()?;
        for nullifier in &transaction.nullifiers {
            if self.is_nullifier_spent(nullifier)? {
                return Err("shielded nullifier already spent".into());
            }
        }
        let result = (|| {
            for nullifier in &transaction.nullifiers {
                self.add_nullifier(*nullifier)?;
            }
            for commitment in &transaction.new_commitments {
                self.add_commitment(*commitment)?;
            }
            Ok::<(), Box<dyn std::error::Error>>(())
        })();
        if let Err(error) = result {
            self.restore(snapshot)?;
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nullifier_cannot_be_spent_twice() {
        let path = ShieldedState::test_path();
        let _ = std::fs::remove_dir_all(&path);
        let mut state = ShieldedState::new(&path).expect("shielded state");
        let transaction = ShieldedTransaction {
            root: state.root(),
            nullifiers: vec![[1u8; 32]],
            new_commitments: vec![[2u8; 32]],
            proof: vec![1],
            fee: 1,
        };
        state.apply_transaction(&transaction).expect("first spend");
        assert!(state.apply_transaction(&transaction).is_err());
        drop(state);
        let _ = std::fs::remove_dir_all(&path);
    }
}

impl std::fmt::Debug for ShieldedState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ShieldedState")
            .field("commitment_tree", &self.commitment_tree)
            .field("spent_nullifiers", &"sled::Tree")
            .finish()
    }
}
