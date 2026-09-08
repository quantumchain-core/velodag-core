// vdag-node/src/main.rs

pub mod behaviour;
pub mod difficulty_log;
pub mod network;
pub mod rpc;
pub mod sync;

use std::env;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tokio::time::{interval_at, Instant};
use tracing::{error, info, warn};

use libp2p::{
    futures::StreamExt,
    gossipsub::{self, IdentTopic},
    identity, mdns, noise, request_response,
    swarm::StreamProtocol,
    tcp, yamux, Multiaddr, Swarm, SwarmBuilder,
};

use vdag_consensus::{
    daa::DifficultyManager, ghostdag::GhostdagManager, pow::PowManager, BlockHeader,
    BlockchainStorage, LedgerState, Mempool, VeloBlock, DEV_TREASURY_ADDRESS,
};
use vdag_crypto::VeloKeyPair;

use behaviour::{SyncBehaviour, VeloBehaviour};
use difficulty_log::DifficultyLog;
use network::GOSSIP_TOPIC;
use sync::OrphanPool;

const IDENTITY_KEY_PATH: &str = "node_identity.key";
const BOOTSTRAP_FILE_PATH: &str = "bootstrap_peers.txt";
const ORPHAN_POOL_MAX: usize = 1024;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let storage_engine = BlockchainStorage::open();
    let args: Vec<String> = env::args().collect();

    // 1. Process Explorer CLI Flags
    if args.len() > 2 && args[1] == "--get-block" {
        run_explorer(&storage_engine, &args[2]);
        return Ok(());
    }

    // --dial <multiaddr> may be passed multiple times.
    let cli_dial_targets: Vec<String> = args
        .iter()
        .enumerate()
        .filter(|(_, a)| *a == "--dial")
        .filter_map(|(i, _)| args.get(i + 1).cloned())
        .collect();

    info!("==================================================");
    info!("🚀 Initializing VeloDAG Core Node [Ticker: VDAG] ");
    info!("==================================================");

    // 2. Node identity: persisted across restarts so Peer ID stays stable
    // (bootstrap lists / --dial targets otherwise go stale every run).
    let local_key = load_or_create_identity(IDENTITY_KEY_PATH)?;
    let local_peer_id = libp2p::PeerId::from(local_key.public());
    info!(%local_peer_id, "Local P2P Node Peer ID");

    let mut swarm: Swarm<VeloBehaviour> = SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|key| {
            // Gossipsub tuned for a small testnet mesh rather than the
            // mainnet-scale defaults, plus a hard cap on message size so a
            // peer can't force us to buffer huge payloads.
            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .max_transmit_size(2 * 1024 * 1024) // 2 MB cap per block+txs payload
                .mesh_n_low(1)
                .mesh_n(4)
                .mesh_n_high(8)
                .mesh_outbound_min(1) // must be <= mesh_n_low and <= mesh_n/2, or gossipsub refuses to start
                .validation_mode(gossipsub::ValidationMode::Strict)
                .build()
                .map_err(std::io::Error::other)?;

            let mut gossipsub = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )
            .map_err(std::io::Error::other)?;

            // Basic peer scoring: peers sending invalid/duplicate/spammy
            // gossip get penalized and eventually graylisted automatically.
            gossipsub
                .with_peer_score(
                    gossipsub::PeerScoreParams::default(),
                    gossipsub::PeerScoreThresholds::default(),
                )
                .map_err(std::io::Error::other)?;

            let mdns =
                mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())?;

            let sync: SyncBehaviour = request_response::json::Behaviour::new(
                [(
                    StreamProtocol::new("/velodag/sync/1"),
                    request_response::ProtocolSupport::Full,
                )],
                request_response::Config::default(),
            );

            Ok(VeloBehaviour {
                gossipsub,
                mdns,
                sync,
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    // Subscribe to global block propagation lanes
    let block_topic = IdentTopic::new(GOSSIP_TOPIC);
    swarm.behaviour_mut().gossipsub.subscribe(&block_topic)?;

    // Start listening on a randomized local TCP port interface
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // Dial explicit --dial targets plus anything in bootstrap_peers.txt
    // (one multiaddr per line, '#' comments allowed).
    let mut dial_targets = cli_dial_targets;
    dial_targets.extend(load_bootstrap_peers(BOOTSTRAP_FILE_PATH));
    for addr_str in dial_targets {
        match addr_str.parse::<Multiaddr>() {
            Ok(remote) => {
                if let Err(e) = swarm.dial(remote) {
                    warn!(%addr_str, error = %e, "Failed to dial");
                } else {
                    info!(%addr_str, "Dialing peer");
                }
            }
            Err(e) => warn!(%addr_str, error = %e, "Skipping invalid bootstrap/dial address"),
        }
    }

    // 3. Initialize Core Consensus Subsystems
    let mut ghostdag = GhostdagManager::new(3);
    let mut ledger_state = LedgerState::default();
    let difficulty_manager = DifficultyManager::new(1, 4);
    let mut current_difficulty_target = [0x0f; 32];
    let mut block_history: Vec<VeloBlock> = Vec::new();
    let mut orphans = OrphanPool::new(ORPHAN_POOL_MAX);
    // Records which difficulty target was active at each height, so blocks
    // validated out of real-time order (orphan replays, sync catch-up) are
    // checked against the target that was actually in force then, not
    // whatever "current" happens to be by the time they're processed.
    let mut difficulty_log = DifficultyLog::new();
    let genesis_hash = [0u8; 32];
    let persisted_ledger_state = match storage_engine.load_ledger_state() {
        Ok(state) => state,
        Err(e) => {
            error!(error = %e, "Failed to load persisted ledger state; replaying blocks");
            None
        }
    };

    // 4. Genesis Initialization Engine Check
    match storage_engine.load_block(&genesis_hash) {
        Ok(None) => {
            info!("[🧱 Genesis Engine] Minting Genesis Block 0...");
            let genesis_block = create_block(vec![], 0, [0u8; 32], 0, [0u8; 32], 0);
            ledger_state.apply_block(&genesis_block).unwrap();
            let genesis_dag_data = ghostdag.calculate_ghostdag_data(&genesis_block, genesis_hash);

            storage_engine
                .save_block(&genesis_hash, &genesis_block)
                .unwrap();
            storage_engine
                .save_ghostdag_data(&genesis_hash, &genesis_dag_data)
                .unwrap();
            storage_engine.save_ledger_state(&ledger_state).unwrap();

            ghostdag
                .block_store
                .insert(genesis_hash, genesis_block.clone());
            ghostdag
                .ghostdag_cache
                .insert(genesis_hash, genesis_dag_data);
            block_history.push(genesis_block);
        }
        Ok(Some(genesis_blk)) => {
            info!("[💾 Storage Engine] Resuming ledger context.");
            if persisted_ledger_state.is_none() {
                ledger_state.apply_block(&genesis_blk).unwrap();
            }
            let genesis_dag_data = ghostdag.calculate_ghostdag_data(&genesis_blk, genesis_hash);
            ghostdag
                .block_store
                .insert(genesis_hash, genesis_blk.clone());
            ghostdag
                .ghostdag_cache
                .insert(genesis_hash, genesis_dag_data);
            block_history.push(genesis_blk);

            match storage_engine.load_all_blocks() {
                Ok(stored_blocks) => {
                    for block in stored_blocks {
                        if block.header.height == 0 {
                            continue;
                        }
                        if persisted_ledger_state.is_none() {
                            ledger_state.apply_block(&block).unwrap();
                        }
                        let block_hash = block.calculate_hash();
                        let dag_data = ghostdag.calculate_ghostdag_data(&block, block_hash);
                        ghostdag.block_store.insert(block_hash, block.clone());
                        ghostdag.ghostdag_cache.insert(block_hash, dag_data);
                        block_history.push(block);
                    }
                }
                Err(e) => error!(error = %e, "Failed to replay stored blocks"),
            }

            if let Some(state) = persisted_ledger_state {
                ledger_state = state;
            } else {
                storage_engine.save_ledger_state(&ledger_state).unwrap();
            }
        }
        Err(e) => error!(error = %e, "[💾 Storage Engine Error] Initialization error"),
    }

    let miner_keys = VeloKeyPair::generate();
    let miner_address = VeloKeyPair::derive_address(&miner_keys.public_key);
    info!(address = %format!("0x{}", encode_hex(&miner_address[0..6])), "[🔒 Crypto Engine] Local Miner Live");

    let node_mempool = Arc::new(Mutex::new(Mempool::new()));
    let rpc_address = env::var("VDAG_RPC_ADDR").unwrap_or_else(|_| "127.0.0.1:8545".into());
    tokio::spawn(rpc::serve(rpc_address, Arc::clone(&node_mempool)));
    let mut current_tips = block_history
        .last()
        .map(|block| {
            if block.header.height == 0 {
                vec![genesis_hash]
            } else {
                vec![block.calculate_hash()]
            }
        })
        .unwrap_or_else(|| vec![genesis_hash]);
    let mut block_height = block_history
        .iter()
        .map(|block| block.header.height)
        .max()
        .unwrap_or(0);
    if let Some(last_block) = block_history.last() {
        current_difficulty_target = last_block.header.difficulty_target;
    }
    let mut sync_pending = false;
    let mut block_timer = interval_at(
        Instant::now() + Duration::from_secs(2),
        Duration::from_secs(1),
    );

    // 5. Unified Async Block Production & P2P Stream Selection Loop
    loop {
        tokio::select! {
            _ = block_timer.tick(), if !sync_pending => {
                block_height += 1;

                let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                let (miner_reward, dev_reward) = VeloBlock::calculate_subsidy_split(block_height);

                let mut next_block = create_block(
                    current_tips.clone(),
                    block_height,
                    miner_address,
                    miner_reward,
                    DEV_TREASURY_ADDRESS,
                    dev_reward,
                );
                next_block.header.timestamp = timestamp;
                next_block.header.difficulty_target = current_difficulty_target;
                next_block.transactions = node_mempool.lock().await.drain_to_batch(10);
                next_block.header.tx_merkle_root = VeloBlock::transaction_merkle_root(&next_block.transactions);

                if next_block.verify_coinbase_rewards() {
                    if let Err(reason) = ledger_state.apply_block(&next_block) {
                        warn!(height = block_height, %reason, "Local block rejected by ledger state");
                        continue;
                    }
                    if let Err(reason) = storage_engine.save_ledger_state(&ledger_state) {
                        warn!(height = block_height, %reason, "Failed to persist ledger state");
                        continue;
                    }
                    // Record the target we're about to mine against *before* mining,
                    // so any peer that later needs to validate this exact block
                    // (orphan replay, sync catch-up) checks it against the same
                    // target we used -- not whatever "current" has drifted to by then.
                    difficulty_log.record(block_height, current_difficulty_target);

                    let pow_manager = PowManager::new(current_difficulty_target);
                    let block_hash = pow_manager.mine_block(&mut next_block);
                    let dag_data = ghostdag.calculate_ghostdag_data(&next_block, block_hash);

                    ghostdag.block_store.insert(block_hash, next_block.clone());
                    ghostdag.ghostdag_cache.insert(block_hash, dag_data.clone());
                    block_history.push(next_block.clone());

                    if storage_engine.save_block(&block_hash, &next_block).is_ok() {
                        let _ = storage_engine.save_ghostdag_data(&block_hash, &dag_data);
                        info!(
                            height = block_height,
                            hash = %encode_hex(&block_hash[0..8]),
                            nonce = next_block.header.nonce,
                            "[⏱️ Block Mined Locally]"
                        );

                        if let Ok(encoded_payload) = bincode::serialize(&next_block) {
                            let _ = swarm.behaviour_mut().gossipsub.publish(block_topic.clone(), encoded_payload);
                        }
                    }

                    current_difficulty_target = difficulty_manager.calculate_next_target(&block_history, current_difficulty_target);
                    current_tips = vec![block_hash];
                }
            }

            network_event = swarm.select_next_some() => {
                let _ = network::handle_p2p_events(
                    network_event,
                    &mut swarm,
                    &storage_engine,
                    &mut ghostdag,
                    &mut orphans,
                    &mut block_history,
                    &mut difficulty_log,
                    &mut ledger_state,
                    genesis_hash,
                    &mut sync_pending,
                );
            }
        }
    }
}

// --- UTILITIES ---

fn create_block(
    parents: Vec<[u8; 32]>,
    height: u64,
    miner_address: [u8; 32],
    miner: u64,
    dev_address: [u8; 32],
    dev: u64,
) -> VeloBlock {
    VeloBlock {
        header: BlockHeader {
            timestamp: 0,
            parents,
            tx_merkle_root: [0u8; 32],
            nonce: 0,
            height,
            difficulty_target: [0x0f; 32],
        },
        transactions: vec![],
        coinbase_miner_address: miner_address,
        coinbase_miner_output: miner,
        coinbase_dev_address: dev_address,
        coinbase_dev_output: dev,
    }
}

fn run_explorer(storage: &BlockchainStorage, hash_str: &str) {
    if let Ok(hash_vec) = decode_hex(hash_str) {
        if hash_vec.len() == 32 {
            let mut target_hash = [0u8; 32];
            target_hash.copy_from_slice(&hash_vec);

            if let Ok(Some(block)) = storage.load_block(&target_hash) {
                // The explorer is a one-shot CLI report, not a running node's
                // log stream -- plain println! output (rather than tracing's
                // timestamped/leveled format) is the right fit here.
                println!("\n==================================================");
                println!("🧱 VELODAG BLOCK METADATA EXPLORER");
                println!("==================================================");
                println!(
                    "• Height: {} | Nonce: {}",
                    block.header.height, block.header.nonce
                );
                println!("• Confirmed TXs: {}", block.transactions.len());
                println!(
                    "• Miner Subsidy: {} | Dev Tax: {}",
                    block.coinbase_miner_output, block.coinbase_dev_output
                );

                if let Ok(Some(dag)) = storage.load_ghostdag_data(&target_hash) {
                    println!("• GHOSTDAG Score: {}", dag.blue_score);
                    println!(
                        "• Blue Count: {} | Red Count: {}",
                        dag.blues.len(),
                        dag.reds.len()
                    );
                }
                println!("==================================================\n");
            }
        }
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn decode_hex(s: &str) -> Result<Vec<u8>, std::num::ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}

/// Loads a persisted node identity from disk, or generates and saves a new
/// one. Keeps Peer ID stable across restarts so bootstrap lists and
/// --dial targets don't go stale every run.
fn load_or_create_identity(path: &str) -> Result<identity::Keypair, Box<dyn std::error::Error>> {
    if let Ok(bytes) = std::fs::read(path) {
        if let Ok(key) = identity::Keypair::from_protobuf_encoding(&bytes) {
            info!(%path, "🔑 Loaded existing node identity");
            return Ok(key);
        }
        warn!(%path, "⚠️ Found identity file but couldn't parse it -- generating a new identity");
    }
    let key = identity::Keypair::generate_ed25519();
    std::fs::write(path, key.to_protobuf_encoding()?)?;
    info!(%path, "🔑 Generated new node identity");
    Ok(key)
}

/// Reads a plain-text bootstrap peer list, one multiaddr per line.
/// Blank lines and lines starting with '#' are ignored. Missing file is
/// not an error -- it just means "no bootstrap peers configured".
fn load_bootstrap_peers(path: &str) -> Vec<String> {
    std::fs::read_to_string(path)
        .map(|content| {
            content
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .collect()
        })
        .unwrap_or_default()
}
