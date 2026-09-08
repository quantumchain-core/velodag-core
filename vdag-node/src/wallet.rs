use std::path::Path;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use vdag_consensus::{Transaction, VeloBlock};
use vdag_crypto::VeloKeyPair;

#[derive(Debug, Serialize, Deserialize)]
struct WalletFile {
    public_key: String,
    secret_key: String,
}

pub fn create(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    if Path::new(path).exists() {
        return Err(format!("wallet already exists: {path}").into());
    }

    let keys = VeloKeyPair::generate();
    let wallet = WalletFile {
        public_key: hex::encode(keys.public_key_bytes()),
        secret_key: hex::encode(keys.secret_key_bytes()),
    };
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
    let wallet: WalletFile = serde_json::from_slice(&bytes)?;
    let public_key = hex::decode(wallet.public_key)?;
    let secret_key = hex::decode(wallet.secret_key)?;
    Ok(VeloKeyPair::from_bytes(&public_key, &secret_key)?)
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
