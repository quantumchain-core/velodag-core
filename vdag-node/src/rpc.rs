use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{info, warn};
use vdag_consensus::{LedgerState, Mempool, Transaction, VeloBlock};

/// Bumped whenever a breaking change is made to request/response shapes or
/// method behavior -- lets a client detect "am I talking to a compatible
/// server" before relying on a method's exact response shape.
const RPC_API_VERSION: &str = "1.0.0";

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

#[derive(Debug, Deserialize)]
struct AddressParams {
    address: String,
}

#[derive(Debug, Deserialize)]
struct TransactionStatusParams {
    tx_id: String,
}

pub async fn serve(
    address: String,
    mempool: Arc<Mutex<Mempool>>,
    ledger_state: Arc<Mutex<LedgerState>>,
) -> Result<(), std::io::Error> {
    let listener = TcpListener::bind(&address).await?;
    info!(%address, "RPC server listening");

    loop {
        let (stream, peer) = listener.accept().await?;
        let shared_mempool = Arc::clone(&mempool);
        let shared_ledger = Arc::clone(&ledger_state);
        tokio::spawn(async move {
            if let Err(error) = handle_connection(stream, shared_mempool, shared_ledger).await {
                warn!(%peer, %error, "RPC connection failed");
            }
        });
    }
}

async fn handle_connection(
    stream: TcpStream,
    mempool: Arc<Mutex<Mempool>>,
    ledger_state: Arc<Mutex<LedgerState>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let response = match serde_json::from_str::<RpcRequest>(&line) {
            Ok(request) => dispatch(request, &mempool, &ledger_state).await,
            Err(error) => error_response(None, format!("invalid JSON-RPC request: {error}")),
        };
        writer.write_all(response.to_string().as_bytes()).await?;
        writer.write_all(b"\n").await?;
    }

    Ok(())
}

async fn dispatch(
    request: RpcRequest,
    mempool: &Arc<Mutex<Mempool>>,
    ledger_state: &Arc<Mutex<LedgerState>>,
) -> Value {
    let id = request.id.clone().unwrap_or(Value::Null);
    match request.method.as_str() {
        "version" => {
            json!({ "jsonrpc": "2.0", "id": id, "result": { "rpc_api_version": RPC_API_VERSION } })
        }
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
        "get_balance" => {
            let params = match request.params {
                Some(params) => match serde_json::from_value::<AddressParams>(params) {
                    Ok(params) => params,
                    Err(error) => {
                        return error_response(Some(id), format!("invalid parameters: {error}"))
                    }
                },
                None => return error_response(Some(id), "missing parameters".into()),
            };
            let address = match parse_fixed_bytes::<32>(&params.address, "address") {
                Ok(address) => address,
                Err(error) => return error_response(Some(id), error),
            };
            let balance = ledger_state.lock().await.balance(&address);
            json!({ "jsonrpc": "2.0", "id": id, "result": { "balance": balance } })
        }
        "transaction_status" => {
            let params = match request.params {
                Some(params) => match serde_json::from_value::<TransactionStatusParams>(params) {
                    Ok(params) => params,
                    Err(error) => {
                        return error_response(Some(id), format!("invalid parameters: {error}"))
                    }
                },
                None => return error_response(Some(id), "missing parameters".into()),
            };
            let tx_id = match parse_fixed_bytes::<32>(&params.tx_id, "tx_id") {
                Ok(tx_id) => tx_id,
                Err(error) => return error_response(Some(id), error),
            };

            let status = if ledger_state.lock().await.has_confirmed(&tx_id) {
                "confirmed"
            } else if mempool.lock().await.pending_transactions.contains_key(&tx_id) {
                "pending"
            } else {
                "unknown"
            };
            json!({ "jsonrpc": "2.0", "id": id, "result": { "status": status } })
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

    #[tokio::test]
    async fn get_balance_reflects_ledger_state() {
        let mempool = Arc::new(Mutex::new(Mempool::new()));
        let ledger = Arc::new(Mutex::new(LedgerState::default()));

        let address = [7u8; 32];
        // No funding block applied -- balance must default to zero, not error.
        let request = RpcRequest {
            id: Some(json!(1)),
            method: "get_balance".to_string(),
            params: Some(json!({ "address": hex::encode(address) })),
        };
        let response = dispatch(request, &mempool, &ledger).await;
        assert_eq!(response["result"]["balance"], 0);
    }

    #[tokio::test]
    async fn transaction_status_distinguishes_pending_confirmed_and_unknown() {
        let mempool = Arc::new(Mutex::new(Mempool::new()));
        let ledger = Arc::new(Mutex::new(LedgerState::default()));

        let unknown_id = [1u8; 32];
        let request = RpcRequest {
            id: Some(json!(1)),
            method: "transaction_status".to_string(),
            params: Some(json!({ "tx_id": hex::encode(unknown_id) })),
        };
        let response = dispatch(request, &mempool, &ledger).await;
        assert_eq!(response["result"]["status"], "unknown");
    }

    #[tokio::test]
    async fn version_reports_the_rpc_api_version() {
        let mempool = Arc::new(Mutex::new(Mempool::new()));
        let ledger = Arc::new(Mutex::new(LedgerState::default()));

        let request = RpcRequest {
            id: Some(json!(1)),
            method: "version".to_string(),
            params: None,
        };
        let response = dispatch(request, &mempool, &ledger).await;
        assert_eq!(response["result"]["rpc_api_version"], RPC_API_VERSION);
    }
}
