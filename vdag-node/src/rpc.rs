use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{info, warn};
use vdag_consensus::{Mempool, Transaction, VeloBlock};

#[derive(Debug, Deserialize)]
struct RpcRequest {
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct SubmitTransactionParams {
    sender: String,
    recipient: String,
    amount: u64,
    nonce: u64,
    public_key: String,
    signature: String,
}

pub async fn serve(address: String, mempool: Arc<Mutex<Mempool>>) -> Result<(), std::io::Error> {
    let listener = TcpListener::bind(&address).await?;
    info!(%address, "RPC server listening");

    loop {
        let (stream, peer) = listener.accept().await?;
        let shared_mempool = Arc::clone(&mempool);
        tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, shared_mempool).await {
                warn!(%peer, %error, "RPC connection failed");
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    mempool: Arc<Mutex<Mempool>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let response = match serde_json::from_str::<RpcRequest>(&line) {
            Ok(request) => dispatch(request, &mempool).await,
            Err(error) => error_response(None, format!("invalid JSON-RPC request: {error}")),
        };
        writer.write_all(response.to_string().as_bytes()).await?;
        writer.write_all(b"\n").await?;
    }

    Ok(())
}

async fn dispatch(request: RpcRequest, mempool: &Arc<Mutex<Mempool>>) -> Value {
    let id = request.id.clone().unwrap_or(Value::Null);
    match request.method.as_str() {
        "submit_transaction" => {
            let params = match request.params {
                Some(params) => match serde_json::from_value::<SubmitTransactionParams>(params) {
                    Ok(params) => params,
                    Err(error) => {
                        return error_response(Some(id), format!("invalid parameters: {error}"))
                    }
                },
                None => return error_response(Some(id), "missing parameters".into()),
            };

            match build_transaction(params) {
                Ok(transaction) => {
                    let transaction_id = VeloBlock::transaction_id(&transaction);
                    let accepted = mempool.lock().await.add_transaction(transaction);
                    if accepted {
                        json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": { "accepted": true, "tx_id": hex::encode(transaction_id) }
                        })
                    } else {
                        error_response(Some(id), "duplicate transaction".into())
                    }
                }
                Err(error) => error_response(Some(id), error),
            }
        }
        "mempool_size" => {
            let size = mempool.lock().await.pending_transactions.len();
            json!({ "jsonrpc": "2.0", "id": id, "result": { "size": size } })
        }
        _ => error_response(Some(id), format!("unknown method: {}", request.method)),
    }
}

fn build_transaction(params: SubmitTransactionParams) -> Result<Transaction, String> {
    if params.amount == 0 {
        return Err("amount must be positive".into());
    }

    let sender = parse_fixed_bytes::<32>(&params.sender, "sender")?;
    let recipient = parse_fixed_bytes::<32>(&params.recipient, "recipient")?;
    let public_key =
        hex::decode(&params.public_key).map_err(|_| "invalid public_key hex".to_string())?;
    let signature =
        hex::decode(&params.signature).map_err(|_| "invalid signature hex".to_string())?;

    if !vdag_crypto::verify_transaction_signature(
        &sender,
        &recipient,
        params.amount,
        params.nonce,
        &public_key,
        &signature,
    ) {
        return Err("invalid signature or sender address ownership".into());
    }

    Ok(Transaction {
        sender,
        recipient,
        amount: params.amount,
        nonce: params.nonce,
        public_key,
        signature,
    })
}

fn parse_fixed_bytes<const N: usize>(value: &str, field: &str) -> Result<[u8; N], String> {
    let bytes = hex::decode(value).map_err(|_| format!("invalid {field} hex"))?;
    bytes
        .try_into()
        .map_err(|_| format!("{field} must contain exactly {N} bytes"))
}

fn error_response(id: Option<Value>, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": { "message": message }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vdag_crypto::VeloKeyPair;

    #[test]
    fn accepts_a_valid_signed_transaction() {
        let keys = VeloKeyPair::generate();
        let sender = VeloKeyPair::derive_address(&keys.public_key);
        let recipient = [9u8; 32];
        let transaction = Transaction {
            sender,
            recipient,
            amount: 1,
            nonce: 0,
            public_key: keys.public_key_bytes(),
            signature: Vec::new(),
        };
        let signature = vdag_crypto::sign_message(
            &VeloBlock::transaction_payload(&transaction),
            &keys.secret_key,
        );

        let params = SubmitTransactionParams {
            sender: hex::encode(sender),
            recipient: hex::encode(recipient),
            amount: 1,
            nonce: 0,
            public_key: hex::encode(&transaction.public_key),
            signature: hex::encode(signature),
        };

        assert!(build_transaction(params).is_ok());
    }

    #[test]
    fn rejects_a_signature_from_the_wrong_address() {
        let keys = VeloKeyPair::generate();
        let params = SubmitTransactionParams {
            sender: hex::encode([1u8; 32]),
            recipient: hex::encode([2u8; 32]),
            amount: 1,
            nonce: 0,
            public_key: hex::encode(keys.public_key_bytes()),
            signature: hex::encode(vdag_crypto::sign_message(
                b"wrong payload",
                &keys.secret_key,
            )),
        };

        assert!(build_transaction(params).is_err());
    }
}
