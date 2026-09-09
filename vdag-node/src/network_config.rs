use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BootstrapConfig {
    pub network: String,
    pub network_id: u64,
    pub genesis_hash: String,
    pub seeds: Vec<String>,
    pub signing_key: String,
    pub signature: String,
}

impl BootstrapConfig {
    pub fn canonical_payload(&self) -> Vec<u8> {
        let mut copy = self.clone();
        copy.signature.clear();
        copy.signing_key.clear();
        serde_json::to_vec(&copy).expect("bootstrap config must serialize deterministically")
    }

    pub fn verify_signature(&self) -> bool {
        if self.signing_key.is_empty() || self.signature.is_empty() {
            return false;
        }

        let signing_key = match hex::decode(&self.signing_key) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        let signature = match hex::decode(&self.signature) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };

        let mut key_bytes = [0u8; 32];
        if signing_key.len() != 32 {
            return false;
        }
        key_bytes.copy_from_slice(&signing_key);

        let verifying_key = match VerifyingKey::from_bytes(&key_bytes) {
            Ok(key) => key,
            Err(_) => return false,
        };

        let signature = match Signature::from_slice(&signature) {
            Ok(sig) => sig,
            Err(_) => return false,
        };

        verifying_key.verify(&self.canonical_payload(), &signature).is_ok()
    }

    pub fn with_signature(mut self, secret_key: &SigningKey) -> Self {
        self.signing_key = hex::encode(secret_key.verifying_key().to_bytes());
        let payload = self.canonical_payload();
        self.signature = hex::encode(secret_key.sign(&payload).to_bytes());
        self
    }
}

pub fn resolve_network_environment() -> String {
    env::var("VDAG_NETWORK")
        .unwrap_or_else(|_| "devnet".to_string())
        .to_ascii_lowercase()
}

pub fn resolve_network_id(network_name: &str) -> u64 {
    match network_name {
        "devnet" => vdag_consensus::DEVNET_NETWORK_ID,
        "testnet" => vdag_consensus::TESTNET_NETWORK_ID,
        "mainnet" => vdag_consensus::MAINNET_NETWORK_ID,
        _ => vdag_consensus::DEVNET_NETWORK_ID,
    }
}

pub fn fixed_genesis_hash_for_network(network_name: &str) -> [u8; 32] {
    let _ = network_name;
    vdag_consensus::fixed_genesis_hash()
}

pub fn default_bootstrap_config(network_name: &str) -> BootstrapConfig {
    let seeds = match network_name {
        "devnet" => vec!["/ip4/127.0.0.1/tcp/4001".to_string()],
        "testnet" => vec![
            "/ip4/127.0.0.1/tcp/4001".to_string(),
            "/ip4/127.0.0.1/tcp/4002".to_string(),
        ],
        _ => vec![],
    };

    BootstrapConfig {
        network: network_name.to_string(),
        network_id: resolve_network_id(network_name),
        genesis_hash: hex::encode(fixed_genesis_hash_for_network(network_name)),
        seeds,
        signing_key: String::new(),
        signature: String::new(),
    }
}

pub fn bootstrap_config_path(network_name: &str) -> String {
    env::var("VDAG_BOOTSTRAP_CONFIG")
        .unwrap_or_else(|_| format!("bootstrap.{}.json", network_name))
}

pub fn load_bootstrap_config(network_name: &str) -> Result<BootstrapConfig, String> {
    let path = bootstrap_config_path(network_name);
    load_bootstrap_config_from_path(&path, network_name)
}

pub fn load_bootstrap_config_from_path(
    path: &str,
    network_name: &str,
) -> Result<BootstrapConfig, String> {
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            let config: BootstrapConfig = serde_json::from_str(&contents)
                .map_err(|err| format!("failed to parse {path}: {err}"))?;
            if !config.verify_signature() {
                return Err(format!("bootstrap config signature verification failed for {path}"));
            }
            Ok(config)
        }
        Err(_) => Ok(default_bootstrap_config(network_name)),
    }
}

pub fn seed_addresses_for_environment(network_name: &str) -> Vec<String> {
    load_bootstrap_config(network_name)
        .unwrap_or_else(|_| default_bootstrap_config(network_name))
        .seeds
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    #[test]
    fn bootstrap_signature_round_trips() {
        let secret = SigningKey::from_bytes(&[7u8; 32]);
        let cfg = BootstrapConfig {
            network: "testnet".to_string(),
            network_id: 2,
            genesis_hash: "deadbeef".repeat(8),
            seeds: vec!["/ip4/127.0.0.1/tcp/4001".to_string()],
            signing_key: String::new(),
            signature: String::new(),
        }
        .with_signature(&secret);

        assert!(cfg.verify_signature());
    }

    #[test]
    fn bootstrap_config_loads_from_explicit_path() {
        let dir = std::env::temp_dir().join(format!(
            "velodag-bootstrap-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("bootstrap.testnet.json");
        let secret = SigningKey::from_bytes(&[9u8; 32]);
        let cfg = BootstrapConfig {
            network: "testnet".to_string(),
            network_id: 2,
            genesis_hash: "deadbeef".repeat(8),
            seeds: vec!["/ip4/127.0.0.1/tcp/4001".to_string()],
            signing_key: String::new(),
            signature: String::new(),
        }
        .with_signature(&secret);

        std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
        let loaded = load_bootstrap_config_from_path(path.to_str().unwrap(), "testnet").unwrap();
        assert_eq!(loaded.network, "testnet");
        assert_eq!(loaded.seeds.len(), 1);
        assert!(loaded.verify_signature());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
