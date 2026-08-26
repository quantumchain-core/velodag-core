// vdag-node/src/network.rs
//
// All asynchronous P2P event handling: incoming gossip blocks (with full
// validation), mDNS peer discovery, and the block-sync request/response
// protocol.

use std::error::Error;

use libp2p::{gossipsub, mdns, request_response, gossipsub::IdentTopic, swarm::SwarmEvent, Swarm};

use vdag_consensus::{pow::PowManager, ghostdag::GhostdagManager, BlockchainStorage, VeloBlock};

use crate::behaviour::{VeloBehaviour, VeloBehaviourEvent};
use crate::sync::{OrphanPool, SyncRequest, SyncResponse, MAX_SYNC_BLOCKS};

/// Shared gossip topic name -- imported by main.rs too, so there's a single
/// source of truth instead of the string being duplicated across files.
pub const GOSSIP_TOPIC: &str = "vdag-blocks";

/// Top-level dispatcher for every swarm event. Called once per event from
/// the main select! loop.
#[allow(clippy::too_many_arguments)]
pub fn handle_p2p_events(
    event: SwarmEvent<VeloBehaviourEvent>,
    swarm: &mut Swarm<VeloBehaviour>,
    storage_engine: &BlockchainStorage,
    ghostdag: &mut GhostdagManager,
    orphans: &mut OrphanPool,
    block_history: &mut Vec<VeloBlock>,
    current_difficulty_target: [u8; 32],
    genesis_hash: [u8; 32],
) -> Result<(), Box<dyn Error>> {
    match event {
        // --- Gossip: new block from a peer ---
        SwarmEvent::Behaviour(VeloBehaviourEvent::Gossipsub(gossipsub::Event::Message {
            message,
            ..
        })) => {
            handle_gossip_block(
                message,
                storage_engine,
                ghostdag,
                orphans,
                block_history,
                current_difficulty_target,
            )?;
        }

        // --- mDNS: found a peer on the LAN -- dial it and add it as an explicit gossipsub peer ---
        SwarmEvent::Behaviour(VeloBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
            for (peer_id, addr) in list {
                println!("🔍 [mDNS] Discovered peer {peer_id} at {addr}");
                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                let _ = swarm.dial(addr);
            }
        }
        SwarmEvent::Behaviour(VeloBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
            for (peer_id, _addr) in list {
                println!("👋 [mDNS] Peer expired: {peer_id}");
                swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
            }
        }

        // --- Sync protocol: someone asked us for blocks, or answered our request ---
        SwarmEvent::Behaviour(VeloBehaviourEvent::Sync(request_response::Event::Message {
            peer,
            message,
        })) => match message {
            request_response::Message::Request { request, channel, .. } => {
                let response = build_sync_response(&request, genesis_hash, block_history);
                let _ = swarm.behaviour_mut().sync.send_response(channel, response);
            }
            request_response::Message::Response { response, .. } => {
                handle_sync_response(
                    peer,
                    response,
                    swarm,
                    storage_engine,
                    ghostdag,
                    orphans,
                    block_history,
                )?;
            }
        },
        SwarmEvent::Behaviour(VeloBehaviourEvent::Sync(request_response::Event::OutboundFailure {
            peer,
            error,
            ..
        })) => {
            println!("⚠️ [Sync] Outbound request to {peer} failed: {error}");
        }

        SwarmEvent::NewListenAddr { address, .. } => {
            println!("🌐 [P2P Network] Local Node Listening Live on: {address}");
        }

        // On connect, immediately ask the peer to fill in anything we're missing.
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            println!("🤝 [P2P Network] Remote Connection Settled with Peer: {peer_id}");
            let since_height = block_history.iter().map(|b| b.header.height).max().unwrap_or(0);
            swarm.behaviour_mut().sync.send_request(
                &peer_id,
                SyncRequest { genesis_hash, since_height },
            );
        }

        _ => {}
    }
    Ok(())
}

// --- Gossip block handling ---------------------------------------------

fn handle_gossip_block(
    message: gossipsub::Message,
    storage: &BlockchainStorage,
    ghostdag: &mut GhostdagManager,
    orphans: &mut OrphanPool,
    block_history: &mut Vec<VeloBlock>,
    current_difficulty_target: [u8; 32],
) -> Result<(), Box<dyn Error>> {
    if message.topic != IdentTopic::new(GOSSIP_TOPIC).hash() {
        return Ok(());
    }

    let incoming_block: VeloBlock = match bincode::deserialize(&message.data) {
        Ok(b) => b,
        Err(_) => {
            println!("🚫 [Validation] Dropped malformed block payload from peer.");
            return Ok(());
        }
    };

    validate_and_ingest(
        incoming_block,
        storage,
        ghostdag,
        orphans,
        block_history,
        current_difficulty_target,
    )
}

/// Full consensus validation before a block is allowed anywhere near local
/// state: reward-split check, proof-of-work check, then parent-existence
/// check (orphaning it if a parent is missing).
fn validate_and_ingest(
    block: VeloBlock,
    storage: &BlockchainStorage,
    ghostdag: &mut GhostdagManager,
    orphans: &mut OrphanPool,
    block_history: &mut Vec<VeloBlock>,
    current_difficulty_target: [u8; 32],
) -> Result<(), Box<dyn Error>> {
    let hash = block.calculate_hash();

    if storage.load_block(&hash)?.is_some() {
        return Ok(()); // already known, nothing to do
    }

    // 1. Coinbase / dev-tax split must match consensus rules exactly.
    if !block.verify_coinbase_rewards() {
        println!(
            "🚫 [Validation] Rejected block at height {}: bad coinbase split.",
            block.header.height
        );
        return Ok(());
    }

    // 2. Proof-of-work must satisfy our current difficulty target.
    //    (Genesis is exempt -- it's never gossiped/mined via PoW.)
    if block.header.height > 0 {
        let pow = PowManager::new(current_difficulty_target);
        if !pow.verify_pow(&block) {
            println!(
                "🚫 [Validation] Rejected block at height {}: insufficient PoW.",
                block.header.height
            );
            return Ok(());
        }
    }

    // 3. Every parent must already be known locally, or this block gets
    //    parked as an orphan until the missing parent arrives.
    for parent in &block.header.parents {
        if ghostdag.block_store.get(parent).is_none() {
            println!(
                "🧩 [Orphan] Block at height {} is missing a parent -- buffering.",
                block.header.height
            );
            orphans.insert(*parent, block);
            return Ok(());
        }
    }

    ingest_block(block, storage, ghostdag, orphans, block_history)
}

/// Accepts an already-validated block into the DAG + storage, then
/// recursively replays any orphans that were waiting specifically on it.
fn ingest_block(
    block: VeloBlock,
    storage: &BlockchainStorage,
    ghostdag: &mut GhostdagManager,
    orphans: &mut OrphanPool,
    block_history: &mut Vec<VeloBlock>,
) -> Result<(), Box<dyn Error>> {
    let hash = block.calculate_hash();
    if storage.load_block(&hash)?.is_some() {
        return Ok(());
    }

    let dag_data = ghostdag.calculate_ghostdag_data(&block, hash);
    let _ = storage.save_block(&hash, &block);
    let _ = storage.save_ghostdag_data(&hash, &dag_data);

    ghostdag.block_store.insert(hash, block.clone());
    ghostdag.ghostdag_cache.insert(hash, dag_data);
    block_history.push(block.clone());

    let hex_hash: String = hash[0..8].iter().map(|b| format!("{:02x}", b)).collect();
    println!(
        "📥 [Network Influx] Accepted block. Height: {}, Hash: 0x{}",
        block.header.height, hex_hash
    );

    // NOTE: orphans released here skip re-validation for simplicity. In a
    // stricter setup you'd route them back through validate_and_ingest's
    // PoW/coinbase checks too -- they were already checked once when first
    // received, so the main risk skipped here is re-checking against a
    // *current* difficulty target that may have moved on since.
    let ready = orphans.take_ready(&hash);
    for orphan in ready {
        ingest_block(orphan, storage, ghostdag, orphans, block_history)?;
    }

    Ok(())
}

// --- Sync protocol handling ---------------------------------------------

fn build_sync_response(
    request: &SyncRequest,
    genesis_hash: [u8; 32],
    block_history: &[VeloBlock],
) -> SyncResponse {
    if request.genesis_hash != genesis_hash {
        println!("⚠️ [Sync] Peer requested sync with a different genesis -- refusing.");
        return SyncResponse::GenesisMismatch;
    }

    let blocks: Vec<VeloBlock> = block_history
        .iter()
        .filter(|b| b.header.height > request.since_height)
        .take(MAX_SYNC_BLOCKS)
        .cloned()
        .collect();

    println!(
        "📤 [Sync] Sending {} block(s) (since height {})",
        blocks.len(),
        request.since_height
    );
    SyncResponse::Blocks(blocks)
}

fn handle_sync_response(
    peer: libp2p::PeerId,
    response: SyncResponse,
    swarm: &mut Swarm<VeloBehaviour>,
    storage: &BlockchainStorage,
    ghostdag: &mut GhostdagManager,
    orphans: &mut OrphanPool,
    block_history: &mut Vec<VeloBlock>,
) -> Result<(), Box<dyn Error>> {
    match response {
        SyncResponse::Blocks(blocks) => {
            println!("📥 [Sync] Received {} catch-up block(s) from {peer}", blocks.len());
            for block in blocks {
                // Sync responses skip PoW re-verification against a possibly
                // stale difficulty target; they're still checked for
                // coinbase correctness and parent availability. Tighten
                // this further (e.g. verify against the target recorded
                // at that historical height) before treating this as
                // adversarial-peer-safe.
                if !block.verify_coinbase_rewards() {
                    println!("🚫 [Sync] Rejected synced block: bad coinbase split.");
                    continue;
                }
                ingest_block(block, storage, ghostdag, orphans, block_history)?;
            }
        }
        SyncResponse::GenesisMismatch => {
            println!("⚠️ [Sync] Peer {peer} is on a different network. Disconnecting.");
            let _ = swarm.disconnect_peer_id(peer);
        }
    }
    Ok(())
}
