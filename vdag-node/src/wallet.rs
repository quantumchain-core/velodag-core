use std::path::Path;

use argon2::Argon2;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use vdag_consensus::{Transaction, VeloBlock};
use vdag_crypto::VeloKeyPair;

#[derive(Debug, Serialize, Deserialize)]
struct WalletFile {
    version: u8,
    salt: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct WalletPayload {
    public_key: String,
    secret_key: String,
}

pub fn create(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    if Path::new(path).exists() {
        return Err(format!("wallet already exists: {path}").into());
    }

    let keys = VeloKeyPair::generate();
    let payload = WalletPayload {
        public_key: hex::encode(keys.public_key_bytes()),
        secret_key: hex::encode(keys.secret_key_bytes()),
    };
    let wallet = encrypt(&payload, &password()?)?;
    std::fs::write(path, serde_json::to_vec_pretty(&wallet)?)?;
    set_private_permissions(path)?;
    println!(
        "address=0x{}",
        hex::encode(VeloKeyPair::derive_address(&keys.public_key))
    );
    println!("wallet={path}");
    Ok(())
}

pub fn address(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let keys = load(path)?;
    println!(
        "0x{}",
        hex::encode(VeloKeyPair::derive_address(&keys.public_key))
    );
    Ok(())
}

pub fn verify(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let keys = load(path)?;
    let address = VeloKeyPair::derive_address(&keys.public_key);
    println!("wallet valid address=0x{}", hex::encode(address));
    Ok(())
}

pub fn backup(source: &str, destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    load(source)?;
    if Path::new(destination).exists() {
        return Err(format!("backup destination already exists: {destination}").into());
    }
    std::fs::copy(source, destination)?;
    set_private_permissions(destination)?;
    println!("encrypted wallet backup written to {destination}");
    Ok(())
}

pub fn restore(backup: &str, destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    load(backup)?;
    if Path::new(destination).exists() {
        return Err(format!("restore destination already exists: {destination}").into());
    }
    std::fs::copy(backup, destination)?;
    set_private_permissions(destination)?;
    load(destination)?;
    println!("wallet restored to {destination}");
    Ok(())
}

pub fn sign_transfer(
    path: &str,
    recipient: &str,
    amount: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let keys = load(path)?;
    let sender = VeloKeyPair::derive_address(&keys.public_key);
    let recipient = parse_address(recipient)?;
    let unsigned = Transaction {
        sender,
        recipient,
        amount,
        public_key: keys.public_key_bytes(),
        signature: Vec::new(),
    };
    let signature =
        vdag_crypto::sign_message(&VeloBlock::transaction_payload(&unsigned), &keys.secret_key);
    let transaction = Transaction {
        signature,
        ..unsigned
    };
    println!("{}", serde_json::to_string(&transaction)?);
    Ok(())
}

pub async fn submit(
    path: &str,
    recipient: &str,
    amount: u64,
    rpc_address: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let keys = load(path)?;
    let sender = VeloKeyPair::derive_address(&keys.public_key);
    let recipient = parse_address(recipient)?;
    let unsigned = Transaction {
        sender,
        recipient,
        amount,
        public_key: keys.public_key_bytes(),
        signature: Vec::new(),
    };
    let signature =
        vdag_crypto::sign_message(&VeloBlock::transaction_payload(&unsigned), &keys.secret_key);
    let transaction = Transaction {
        signature,
        ..unsigned
    };
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "submit_transaction",
        "params": {
            "sender": hex::encode(transaction.sender),
            "recipient": hex::encode(transaction.recipient),
            "amount": transaction.amount,
            "public_key": hex::encode(&transaction.public_key),
            "signature": hex::encode(&transaction.signature)
        }
    });
    let stream = TcpStream::connect(rpc_address).await?;
    let (reader, mut writer) = stream.into_split();
    writer.write_all(request.to_string().as_bytes()).await?;
    writer.write_all(b"\n").await?;
    let mut response = String::new();
    BufReader::new(reader).read_line(&mut response).await?;
    println!("{}", response.trim());
    Ok(())
}

fn load(path: &str) -> Result<VeloKeyPair, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let payload = if value.get("version").is_some() {
        let wallet: WalletFile = serde_json::from_value(value)?;
        decrypt(&wallet, &password()?)?
    } else {
        eprintln!("warning: plaintext wallet format; migrate it before mainnet use");
        serde_json::from_value::<WalletPayload>(value)?
    };
    let public_key = hex::decode(payload.public_key)?;
    let secret_key = hex::decode(payload.secret_key)?;
    Ok(VeloKeyPair::from_bytes(&public_key, &secret_key)?)
}

fn password() -> Result<String, Box<dyn std::error::Error>> {
    std::env::var("VDAG_WALLET_PASSWORD").map_err(|_| {
        "set VDAG_WALLET_PASSWORD in the terminal; never pass it as a CLI argument".into()
    })
}

fn encrypt(
    payload: &WalletPayload,
    password: &str,
) -> Result<WalletFile, Box<dyn std::error::Error>> {
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut nonce);
    let mut key_bytes = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), &salt, &mut key_bytes)
        .map_err(|error| format!("password key derivation failed: {error}"))?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key_bytes));
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            serde_json::to_vec(payload)?.as_ref(),
        )
        .map_err(|_| "wallet encryption failed")?;
    Ok(WalletFile {
        version: 1,
        salt: hex::encode(salt),
        nonce: hex::encode(nonce),
        ciphertext: hex::encode(ciphertext),
    })
}

fn decrypt(
    wallet: &WalletFile,
    password: &str,
) -> Result<WalletPayload, Box<dyn std::error::Error>> {
    if wallet.version != 1 {
        return Err(format!("unsupported wallet version: {}", wallet.version).into());
    }
    let salt = hex::decode(&wallet.salt)?;
    let nonce = hex::decode(&wallet.nonce)?;
    let ciphertext = hex::decode(&wallet.ciphertext)?;
    let mut key_bytes = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), &salt, &mut key_bytes)
        .map_err(|error| format!("password key derivation failed: {error}"))?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key_bytes));
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| "wallet decryption failed; check VDAG_WALLET_PASSWORD")?;
    Ok(serde_json::from_slice(&plaintext)?)
}

fn parse_address(value: &str) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    let value = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(value)?;
    Ok(bytes
        .try_into()
        .map_err(|_| "address must contain 32 bytes")?)
}

fn set_private_permissions(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}
