use pqcrypto_dilithium::dilithium2::{
    detached_sign, verify_detached_signature, generate_keypair, 
    PublicKey, SecretKey, DetachedSignature
};
use pqcrypto_traits::sign::{PublicKey as TraitPK, SecretKey as TraitSK};
use sha3::{Digest, Sha3_256};

pub struct VeloKeyPair {
    pub public_key: PublicKey,
    pub secret_key: SecretKey,
}

impl VeloKeyPair {
    /// Generates a fresh, post-quantum CRYSTALS-Dilithium2 keypair
    pub fn generate() -> Self {
        let (pk, sk) = generate_keypair();
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

#[cfg(test)]
mod tests {
    use super::*;

    // FIXED: Changed '@test' to standard Rust macro formatting '#[test]'
    #[test]
    fn test_quantum_crypto_flow() {
        let keys = VeloKeyPair::generate();
        let address = VeloKeyPair::derive_address(&keys.public_key);
        
        let tx_data = b"transfer_100_vdag_to_recipient";
        let signature = sign_message(tx_data, &keys.secret_key);
        
        let is_valid = verify_signature(tx_data, &signature, &keys.public_key);
        assert!(is_valid, "Cryptographic signature verification failed!");
        println!("🚀 Quantum safe address derived successfully: {:?}", address);
    }
}
