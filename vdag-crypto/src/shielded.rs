use blake3::Hasher;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShieldedNote {
    pub commitment: [u8; 32],
    pub value: u64,
    pub rho: [u8; 32],
    pub rcm: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShieldedTransaction {
    #[serde(default)]
    pub root: [u8; 32],
    pub nullifiers: Vec<[u8; 32]>,
    pub new_commitments: Vec<[u8; 32]>,
    pub proof: Vec<u8>,
    pub fee: u64,
}

#[cfg(feature = "halo2")]
pub fn create_shielded_tx(
    input_values: [u64; 2],
    output_notes: [ShieldedNote; 2],
    fee: u64,
    root: [u8; 32],
    spending_keys: [[u8; 32]; 2],
    input_rhos: [[u8; 32]; 2],
) -> Result<ShieldedTransaction, String> {
    if output_notes
        .iter()
        .any(|note| note.commitment != note_commitment(note.value, note.rho, note.rcm))
    {
        return Err("output note commitment does not match note contents".into());
    }
    let nullifiers = [
        nullifier(&spending_keys[0], input_rhos[0]),
        nullifier(&spending_keys[1], input_rhos[1]),
    ]
    .to_vec();
    let new_commitments = output_notes.clone().map(|note| note.commitment).to_vec();
    let proof = crate::halo2_circuit::prove_shielded_tx(
        input_values,
        output_notes.map(|note| note.value),
        fee,
        root,
        new_commitments.clone(),
        nullifiers.clone(),
    );
    if proof.is_empty() {
        return Err("failed to construct shielded proof".into());
    }
    Ok(ShieldedTransaction {
        root,
        nullifiers,
        new_commitments,
        proof,
        fee,
    })
}

#[cfg(feature = "halo2")]
pub fn verify_shielded_proof(proof: &[u8], inputs: &[Vec<u8>]) -> bool {
    crate::halo2_circuit::verify_shielded_proof(proof, inputs)
}

#[cfg(not(feature = "halo2"))]
pub fn verify_shielded_proof(_proof: &[u8], _inputs: &[Vec<u8>]) -> bool {
    false
}

pub fn note_commitment(value: u64, rho: [u8; 32], rcm: [u8; 32]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(b"velodag/shielded-note/v1");
    hasher.update(&value.to_le_bytes());
    hasher.update(&rho);
    hasher.update(&rcm);
    *hasher.finalize().as_bytes()
}

pub fn nullifier(sk: &[u8; 32], rho: [u8; 32]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(b"velodag/shielded-nullifier/v1");
    hasher.update(sk);
    hasher.update(&rho);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_commitment_is_deterministic_and_binds_inputs() {
        let rho = [1u8; 32];
        let rcm = [2u8; 32];
        let first = note_commitment(7, rho, rcm);
        assert_eq!(first, note_commitment(7, rho, rcm));
        assert_ne!(first, note_commitment(8, rho, rcm));
    }

    #[test]
    fn nullifier_is_deterministic_and_binds_secret_and_rho() {
        let sk = [3u8; 32];
        let rho = [4u8; 32];
        let first = nullifier(&sk, rho);
        assert_eq!(first, nullifier(&sk, rho));
        assert_ne!(first, nullifier(&[5u8; 32], rho));
        assert_ne!(first, nullifier(&sk, [6u8; 32]));
    }

    #[cfg(feature = "halo2")]
    #[test]
    fn valid_shielded_transaction_has_a_verifiable_proof() {
        let output_notes = [
            ShieldedNote {
                value: 9,
                rho: [7u8; 32],
                rcm: [8u8; 32],
                commitment: note_commitment(9, [7u8; 32], [8u8; 32]),
            },
            ShieldedNote {
                value: 10,
                rho: [9u8; 32],
                rcm: [10u8; 32],
                commitment: note_commitment(10, [9u8; 32], [10u8; 32]),
            },
        ];
        let transaction = create_shielded_tx(
            [10, 10],
            output_notes,
            1,
            [1u8; 32],
            [[2u8; 32]; 2],
            [[3u8; 32]; 2],
        )
        .expect("valid shielded transaction");
        let mut inputs = vec![transaction.root.to_vec()];
        inputs.extend(transaction.new_commitments.iter().map(|value| value.to_vec()));
        inputs.extend(transaction.nullifiers.iter().map(|value| value.to_vec()));
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"velodag/shielded-fee/v1");
        hasher.update(&transaction.fee.to_le_bytes());
        inputs.push(hasher.finalize().as_bytes().to_vec());
        assert!(verify_shielded_proof(&transaction.proof, &inputs));
    }

    #[cfg(feature = "halo2")]
    #[test]
    fn bad_shielded_proof_is_rejected() {
        let proof = crate::halo2_circuit::prove_shielded_tx(
            [10, 10],
            [9, 9],
            1,
            [1u8; 32],
            vec![[2u8; 32]; 2],
            vec![[3u8; 32]; 2],
        );
        assert!(proof.is_empty());
    }
}
