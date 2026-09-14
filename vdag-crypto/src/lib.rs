use pqcrypto_dilithium::dilithium2::{
    detached_sign, keypair, verify_detached_signature, DetachedSignature, PublicKey, SecretKey,
};
// Explicitly import signature traits to expose .as_bytes() and .from_bytes()
use pqcrypto_traits::sign::{
    DetachedSignature as DetachedSignatureTrait, PublicKey as PublicKeyTrait,
    SecretKey as SecretKeyTrait,
};
use curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT;
use curve25519_dalek::ristretto::CompressedRistretto;
use curve25519_dalek::scalar::Scalar;
use curve25519_dalek::traits::Identity;
use rand::{rngs::OsRng, RngCore};
use sha3::{Digest, Sha3_256};

#[cfg(feature = "shielded")]
pub mod shielded;
#[cfg(feature = "halo2")]
pub mod halo2_circuit;

// ProductionProof is temporary. Real privacy uses ShieldedTransaction.

/// Non-interactive Schnorr proof that the producer knows the secret used for
/// the producer's public statement. This is a proof-of-knowledge gate for
/// block production, not a shielded-transaction proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionProof {
    pub public_key: Vec<u8>,
    pub public_statement: [u8; 32],
    pub commitment: [u8; 32],
    pub response: [u8; 32],
}

fn producer_scalar(secret_key: &SecretKey) -> Scalar {
    let digest = Sha3_256::digest(secret_key.as_bytes());
    Scalar::from_bytes_mod_order(digest.into())
}

fn challenge(public_statement: &[u8; 32], commitment: &[u8; 32], miner_address: &[u8; 32]) -> Scalar {
    let mut hasher = sha3::Sha3_512::new();
    hasher.update(public_statement);
    hasher.update(commitment);
    hasher.update(miner_address);
    Scalar::from_bytes_mod_order_wide(&hasher.finalize().into())
}

pub fn create_production_proof(
    key_pair: &VeloKeyPair,
    miner_address: &[u8; 32],
) -> ProductionProof {
    let secret = producer_scalar(&key_pair.secret_key);
    let public_statement = (secret * RISTRETTO_BASEPOINT_POINT).compress().to_bytes();
    let mut nonce_bytes = [0u8; 64];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Scalar::from_bytes_mod_order_wide(&nonce_bytes);
    let commitment = (nonce * RISTRETTO_BASEPOINT_POINT).compress().to_bytes();
    let proof_challenge = challenge(&public_statement, &commitment, miner_address);
    let response = nonce + proof_challenge * secret;
    ProductionProof {
        public_key: key_pair.public_key_bytes(),
        public_statement,
        commitment,
        response: response.to_bytes(),
    }
}

pub fn verify_production_proof(proof: &ProductionProof, miner_address: &[u8; 32]) -> bool {
    let public_key = match PublicKey::from_bytes(&proof.public_key) {
        Ok(public_key) => public_key,
        Err(_) => return false,
    };
    if VeloKeyPair::derive_address(&public_key) != *miner_address {
        return false;
    }
    let public_statement = match CompressedRistretto(proof.public_statement).decompress() {
        Some(point) if point != curve25519_dalek::ristretto::RistrettoPoint::identity() => point,
        _ => return false,
    };
    let commitment = match CompressedRistretto(proof.commitment).decompress() {
        Some(point) => point,
        None => return false,
    };
    let response = Scalar::from_canonical_bytes(proof.response);
    let Some(response) = Option::<Scalar>::from(response) else {
        return false;
    };
    let proof_challenge = challenge(&proof.public_statement, &proof.commitment, miner_address);
    (response * RISTRETTO_BASEPOINT_POINT)
        == commitment + proof_challenge * public_statement
}

pub struct VeloKeyPair {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
}

impl VeloKeyPair {
    /// Generates a fresh, post-quantum CRYSTALS-Dilithium2 keypair
    pub fn generate() -> Self {
        let (pk, sk) = keypair();
        VeloKeyPair {
            public_key: pk,
            secret_key: sk,
        }
    }

    /// Derives the unique VeloDAG wallet address using SHA3-256 of the Dilithium2 public key
    /// Returns a 32-byte array representing the unique user identity
    pub fn derive_address(pk: &PublicKey) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        hasher.update(pk.as_bytes());
        let result = hasher.finalize();

        let mut address = [0u8; 32];
        address.copy_from_slice(&result);
        address
    }

    /// Returns the public key in its canonical serialized form for transactions.
    pub fn public_key_bytes(&self) -> Vec<u8> {
        self.public_key.as_bytes().to_vec()
    }

    pub fn secret_key_bytes(&self) -> Vec<u8> {
        self.secret_key.as_bytes().to_vec()
    }

    pub fn from_bytes(public_key: &[u8], secret_key: &[u8]) -> Result<Self, String> {
        let public_key = PublicKey::from_bytes(public_key).map_err(|e| e.to_string())?;
        let secret_key = SecretKey::from_bytes(secret_key).map_err(|e| e.to_string())?;
        Ok(Self {
            public_key,
            secret_key,
        })
    }
}

/// Cryptographically signs a transaction or message payload using the private key
pub fn sign_message(message: &[u8], sk: &SecretKey) -> Vec<u8> {
    let sig = detached_sign(message, sk);
    sig.as_bytes().to_vec()
}

/// Verifies a signature against a public key and original message payload
pub fn verify_signature(message: &[u8], signature_bytes: &[u8], pk: &PublicKey) -> bool {
    if let Ok(sig) = DetachedSignature::from_bytes(signature_bytes) {
        verify_detached_signature(&sig, message, pk).is_ok()
    } else {
        false
    }
}

/// Verifies a transaction signature and proves that the public key owns the sender address.
pub fn verify_transaction_signature(
    sender: &[u8; 32],
    recipient: &[u8; 32],
    amount: u64,
    nonce: u64,
    public_key_bytes: &[u8],
    signature_bytes: &[u8],
) -> bool {
    let public_key = match PublicKey::from_bytes(public_key_bytes) {
        Ok(public_key) => public_key,
        Err(_) => return false,
    };

    if VeloKeyPair::derive_address(&public_key) != *sender {
        return false;
    }

    let mut payload = Vec::with_capacity(32 + 32 + 8 + 8);
    payload.extend_from_slice(sender);
    payload.extend_from_slice(recipient);
    payload.extend_from_slice(&amount.to_le_bytes());
    payload.extend_from_slice(&nonce.to_le_bytes());
    verify_signature(&payload, signature_bytes, &public_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantum_crypto_flow() {
        let keys = VeloKeyPair::generate();
        let address = VeloKeyPair::derive_address(&keys.public_key);

        let tx_data = b"transfer_100_vdag_to_recipient";
        let signature = sign_message(tx_data, &keys.secret_key);

        let is_valid = verify_signature(tx_data, &signature, &keys.public_key);
        assert!(is_valid, "Cryptographic signature verification failed!");
        println!(
            "🚀 Quantum safe address derived successfully: {:?}",
            address
        );
    }

    #[test]
    fn production_proof_requires_secret_knowledge_and_binds_address() {
        let keys = VeloKeyPair::generate();
        let address = VeloKeyPair::derive_address(&keys.public_key);
        let proof = create_production_proof(&keys, &address);
        assert!(verify_production_proof(&proof, &address));
        assert!(!verify_production_proof(&proof, &[9u8; 32]));

        let mut tampered = proof.clone();
        tampered.response[0] ^= 1;
        assert!(!verify_production_proof(&tampered, &address));
    }
}
