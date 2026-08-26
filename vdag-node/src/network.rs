// vdag-node/src/network.rs
//
// All asynchronous P2P event handling: incoming gossip blocks (with full
// validation), mDNS peer discovery, and the block-sync request/response
// protocol.
//
// One important property of this file: gossip blocks, orphan replays, and
// sync catch-up blocks all funnel through the *same* `validate_and_ingest`
// function. Earlier versions took a shortcut for orphan/sync paths that
// skipped re-validation or checked PoW against the wrong (current, rather
// than historical) difficulty target -- that gap is closed by looking up
// each block's height in `DifficultyLog` rather than assuming "current" is
// correct.

use std::error::Error;

use libp2p::{gossipsub, mdns, request_response, gossipsub::IdentTopic, swarm::SwarmEvent, Swarm};
use tracing::{info, warn};

use vdag_consensus::{pow::PowManager, ghostdag::GhostdagManager, BlockchainStorage, VeloBlock};

use crate::behaviour::{VeloBehaviour, VeloBehaviourEvent};
use crate::difficulty_log::DifficultyLog;
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
    difficulty_log: &mut DifficultyLog,
    live_current_target: [u8; 32],
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
                difficulty_log,
                live_current_target,
            )?;
        }

        // --- mDNS: found a peer on the LAN -- dial it and add it as an explicit gossipsub peer ---
        SwarmEvent::Behaviour(VeloBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
            for (peer_id, addr) in list {
                info!(%peer_id, %addr, "mDNS discovered peer");
                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                let _ = swarm.dial(addr);
            }
        }
        SwarmEvent::Behaviour(VeloBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
            for (peer_id, _addr) in list {
                info!(%peer_id, "mDNS peer expired");
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
                    difficulty_log,
                    live_current_target,
                )?;
            }
        },
        SwarmEvent::Behaviour(VeloBehaviourEvent::Sync(request_response::Event::OutboundFailure {
            peer,
            error,
            ..
        })) => {
            warn!(%peer, %error, "Sync request failed");
        }

        SwarmEvent::NewListenAddr { address, .. } => {
            info!(%address, "Local node listening");
        }

        // On connect, immediately ask the peer to fill in anything we're missing.
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            info!(%peer_id, "Connection established");
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
    difficulty_log: &mut DifficultyLog,
    live_current_target: [u8; 32],
) -> Result<(), Box<dyn Error>> {
    if message.topic != IdentTopic::new(GOSSIP_TOPIC).hash() {
        return Ok(());
    }

    let incoming_block: VeloBlock = match bincode::deserialize(&message.data) {
        Ok(b) => b,
        Err(_) => {
            warn!("Dropped malformed block payload from peer");
            return Ok(());
        }
    };

    validate_and_ingest(
        incoming_block,
        storage,
        ghostdag,
        orphans,
        block_history,
        difficulty_log,
        live_current_target,
    )
}

/// Full consensus validation before a block is allowed anywhere near local
/// state: reward-split check, proof-of-work check (against the target that
/// was actually active at that block's height, per `difficulty_log`), then
/// parent-existence check (orphaning it if a parent is missing).
///
/// This is the single validation path used for live gossip blocks, orphan
/// replays, and sync catch-up blocks alike -- no path takes a shortcut.
fn validate_and_ingest(
    block: VeloBlock,
    storage: &BlockchainStorage,
    ghostdag: &mut GhostdagManager,
    orphans: &mut OrphanPool,
    block_history: &mut Vec<VeloBlock>,
    difficulty_log: &mut DifficultyLog,
    live_current_target: [u8; 32],
) -> Result<(), Box<dyn Error>> {
    let hash = block.calculate_hash();

    if storage.load_block(&hash)?.is_some() {
        return Ok(()); // already known, nothing to do
    }

    // 1. Coinbase / dev-tax split must match consensus rules exactly.
    if !block.verify_coinbase_rewards() {
        warn!(height = block.header.height, "Rejected block: bad coinbase split");
        return Ok(());
    }

    // 2. Proof-of-work must satisfy the target that was actually in force
    //    at this block's height -- not necessarily today's "current"
    //    target, since this block may be an orphan replay or a sync
    //    catch-up block mined under an older difficulty. Genesis is exempt
    //    (never gossiped/mined via PoW).
    if block.header.height > 0 {
        let target = difficulty_log.get(block.header.height).unwrap_or(live_current_target);
        let pow = PowManager::new(target);
        if !pow.verify_pow(&block) {
            warn!(height = block.header.height, "Rejected block: insufficient PoW");
            return Ok(());
        }
        difficulty_log.record(block.header.height, target);
    }

    // 3. Every parent must already be known locally, or this block gets
    //    parked as an orphan until the missing parent arrives.
    for parent in &block.header.parents {
        if ghostdag.block_store.get(parent).is_none() {
            info!(height = block.header.height, "Missing parent, buffering as orphan");
            orphans.insert(*parent, block);
            return Ok(());
        }
    }

    let ingested_hash = ingest_block_only(block, storage, ghostdag, block_history)?;

    // A block landing may unblock orphans that were waiting specifically on
    // it. They go back through this same validate_and_ingest path, so
    // they're checked against their own height's recorded target rather
    // than being ingested blind.
    let ready = orphans.take_ready(&ingested_hash);
    for orphan in ready {
        validate_and_ingest(
            orphan,
            storage,
            ghostdag,
            orphans,
            block_history,
            difficulty_log,
            live_current_target,
        )?;
    }

    Ok(())
}

/// Writes an already-validated block into the DAG + storage. Does not
/// validate anything itself -- callers must have already run it through
/// `validate_and_ingest`'s checks.
fn ingest_block_only(
    block: VeloBlock,
    storage: &BlockchainStorage,
    ghostdag: &mut GhostdagManager,
    block_history: &mut Vec<VeloBlock>,
) -> Result<[u8; 32], Box<dyn Error>> {
    let hash = block.calculate_hash();
    let dag_data = ghostdag.calculate_ghostdag_data(&block, hash);
    let _ = storage.save_block(&hash, &block);
    let _ = storage.save_ghostdag_data(&hash, &dag_data);

    ghostdag.block_store.insert(hash, block.clone());
    ghostdag.ghostdag_cache.insert(hash, dag_data);
    block_history.push(block.clone());

    let hex_hash: String = hash[0..8].iter().map(|b| format!("{:02x}", b)).collect();
    info!(height = block.header.height, hash = %hex_hash, "Accepted block");

    Ok(hash)
}

// --- Sync protocol handling ---------------------------------------------

fn build_sync_response(
    request: &SyncRequest,
    genesis_hash: [u8; 32],
    block_history: &[VeloBlock],
) -> SyncResponse {
    if request.genesis_hash != genesis_hash {
        warn!("Peer requested sync with a different genesis -- refusing");
        return SyncResponse::GenesisMismatch;
    }

    let blocks: Vec<VeloBlock> = block_history
        .iter()
        .filter(|b| b.header.height > request.since_height)
        .take(MAX_SYNC_BLOCKS)
        .cloned()
        .collect();

    info!(count = blocks.len(), since_height = request.since_height, "Sending sync response");
    SyncResponse::Blocks(blocks)
}

#[allow(clippy::too_many_arguments)]
fn handle_sync_response(
    peer: libp2p::PeerId,
    response: SyncResponse,
    swarm: &mut Swarm<VeloBehaviour>,
    storage: &BlockchainStorage,
    ghostdag: &mut GhostdagManager,
    orphans: &mut OrphanPool,
    block_history: &mut Vec<VeloBlock>,
    difficulty_log: &mut DifficultyLog,
    live_current_target: [u8; 32],
) -> Result<(), Box<dyn Error>> {
    match response {
        SyncResponse::Blocks(blocks) => {
            info!(count = blocks.len(), %peer, "Received sync catch-up blocks");
            for block in blocks {
                // Routed through the same validate_and_ingest as everything
                // else: coinbase check, PoW checked against the recorded
                // target for that block's own height (falling back to the
                // live target only if we have no record for it), and
                // parent-existence / orphan handling.
                validate_and_ingest(
                    block,
                    storage,
                    ghostdag,
                    orphans,
                    block_history,
                    difficulty_log,
                    live_current_target,
                )?;
            }
        }
        SyncResponse::GenesisMismatch => {
            warn!(%peer, "Peer is on a different network -- disconnecting");
            let _ = swarm.disconnect_peer_id(peer);
        }
    }
    Ok(())
}
