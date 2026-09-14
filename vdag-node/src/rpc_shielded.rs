use blake3::Hasher;
use vdag_consensus::shielded_state::ShieldedState;
use vdag_crypto::shielded::{verify_shielded_proof, ShieldedTransaction};

const MAX_SHIELDED_PROOF_BYTES: usize = 2048;

pub fn submit_shielded_transaction(
    state: &mut ShieldedState,
    transaction: ShieldedTransaction,
) -> Result<[u8; 32], String> {
    if transaction.proof.len() >= MAX_SHIELDED_PROOF_BYTES {
        return Err("shielded proof exceeds 2047 bytes".into());
    }
    if transaction.fee == 0 {
        return Err("shielded transaction fee must be positive".into());
    }
    if transaction.root != state.root() {
        return Err("shielded transaction root does not match ledger state".into());
    }
    let mut public_inputs = vec![transaction.root.to_vec()];
    public_inputs.extend(
        transaction
            .new_commitments
            .iter()
            .map(|commitment| commitment.to_vec()),
    );
    public_inputs.extend(
        transaction
            .nullifiers
            .iter()
            .map(|nullifier| nullifier.to_vec()),
    );
    let mut fee_hasher = Hasher::new();
    fee_hasher.update(b"velodag/shielded-fee/v1");
    fee_hasher.update(&transaction.fee.to_le_bytes());
    public_inputs.push(fee_hasher.finalize().as_bytes().to_vec());
    if !verify_shielded_proof(&transaction.proof, &public_inputs) {
        return Err("invalid shielded proof".into());
    }

    let mut unique_nullifiers = std::collections::HashSet::new();
    for nullifier in &transaction.nullifiers {
        if !unique_nullifiers.insert(*nullifier) {
            return Err("duplicate shielded nullifier".into());
        }
        state
            .is_nullifier_spent(nullifier)
            .map_err(|error| error.to_string())
            .and_then(|spent| {
                if spent {
                    Err("shielded nullifier already spent".into())
                } else {
                    Ok(())
                }
            })?;
    }

    state
        .apply_transaction(&transaction)
        .map_err(|error| error.to_string())?;

    let encoded = bincode::serialize(&transaction).map_err(|error| error.to_string())?;
    let mut hasher = Hasher::new();
    hasher.update(b"velodag/shielded-transaction/v1");
    hasher.update(&encoded);
    Ok(*hasher.finalize().as_bytes())
}
