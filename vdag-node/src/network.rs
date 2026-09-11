// vdag-node/src/network.rs
//
// All asynchronous P2P event handling: incoming gossip blocks (with full
// validation), mDNS peer discovery, and the block-sync request/response
// protocol.
//
// One important property of this file: gossip blocks, orphan replays, and
// sync catch-up blocks all funnel through the *same* `validate_and_ingest`
// function -- no path takes a shortcut. That includes the difficulty
// target: a block's header *declares* the target it claims to have been
// mined under, but that declaration is only checked for real against
// `current_difficulty_target`, a value tracked identically here and in the
// local mining loop by independently running the same DAA calculation.
// Trusting a block's self-declared target without this check would let a
// peer gossip an arbitrarily easy target and have it pass PoW verification
// against its own say-so.

use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};

use libp2p::{gossipsub, gossipsub::IdentTopic, mdns, request_response, swarm::SwarmEvent, Swarm};
use tracing::{info, warn};

use vdag_consensus::{
    daa::DifficultyManager, ghostdag::GhostdagManager, pow::PowManager, BlockchainStorage,
    LedgerState, VeloBlock,
};

use crate::behaviour::{VeloBehaviour, VeloBehaviourEvent};
use crate::difficulty_log::DifficultyLog;
use crate::sync::{OrphanPool, SyncRequest, SyncResponse, MAX_SYNC_BLOCKS};

/// Shared gossip topic name -- imported by main.rs too, so there's a single
/// source of truth instead of the string being duplicated across files.
pub const GOSSIP_TOPIC: &str = "vdag-blocks";

/// Hard caps protecting against a malicious or broken peer forcing
/// excessive memory/CPU use with an oversized payload, a block stuffed
/// with more transactions than could plausibly be legitimate, or a
/// timestamp claiming to be far in the future (which would otherwise let
/// a peer manipulate difficulty in future windows -- the same class of
/// problem as the DAA overflow bug, approached from a different angle).
const MAX_GOSSIP_MESSAGE_BYTES: usize = 2 * 1024 * 1024; // 2 MB
const MAX_TRANSACTIONS_PER_BLOCK: usize = 5_000;
const MAX_FUTURE_DRIFT_SECS: u64 = 30;

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
    ledger_state: &mut LedgerState,
    genesis_hash: [u8; 32],
    sync_pending: &mut bool,
    difficulty_manager: &DifficultyManager,
    current_difficulty_target: &mut [u8; 32],
    local_network_id: u64,
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
                ledger_state,
                difficulty_manager,
                current_difficulty_target,
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
                swarm
                    .behaviour_mut()
                    .gossipsub
                    .remove_explicit_peer(&peer_id);
            }
        }

        // --- Sync protocol: someone asked us for blocks, or answered our request ---
        SwarmEvent::Behaviour(VeloBehaviourEvent::Sync(request_response::Event::Message {
            peer,
            message,
        })) => match message {
            request_response::Message::Request {
                request, channel, ..
            } => {
                let response = build_sync_response(&request, genesis_hash, block_history, local_network_id);
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
                    ledger_state,
                    difficulty_manager,
                    current_difficulty_target,
                )?;
                *sync_pending = false;
            }
        },
        SwarmEvent::Behaviour(VeloBehaviourEvent::Sync(
            request_response::Event::OutboundFailure { peer, error, .. },
        )) => {
            *sync_pending = false;
            warn!(%peer, %error, "Sync request failed");
        }

        SwarmEvent::NewListenAddr { address, .. } => {
            info!(%address, "Local node listening");
        }

        // On connect, immediately ask the peer to fill in anything we're missing.
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            info!(%peer_id, "Connection established");
            *sync_pending = true;
            let since_height = block_history
                .iter()
                .map(|b| b.header.height)
                .max()
                .unwrap_or(0);
            swarm.behaviour_mut().sync.send_request(
                &peer_id,
                SyncRequest {
                    network_id: local_network_id,
                    genesis_hash,
                    since_height,
                },
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
    ledger_state: &mut LedgerState,
    difficulty_manager: &DifficultyManager,
    current_difficulty_target: &mut [u8; 32],
) -> Result<(), Box<dyn Error>> {
    if message.topic != IdentTopic::new(GOSSIP_TOPIC).hash() {
        return Ok(());
    }

    // Reject oversized payloads before spending any CPU on deserialization
    // -- the whole point of this check is to fail cheaply, before the
    // expensive part.
    if message.data.len() > MAX_GOSSIP_MESSAGE_BYTES {
        warn!(
            size = message.data.len(),
            max = MAX_GOSSIP_MESSAGE_BYTES,
            "Dropped oversized gossip payload from peer"
        );
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
        ledger_state,
        difficulty_manager,
        current_difficulty_target,
    )
}

/// Full consensus validation before a block is allowed anywhere near local
/// state: reward-split check, transaction commitment/signature checks,
/// difficulty-target authenticity check, proof-of-work check, then
/// parent-existence check (orphaning it if a parent is missing).
///
/// The difficulty-target check is the important one: a block's header
/// *declares* the target it was mined under, but that declaration is only
/// trustworthy once it's checked against what the DAA actually says should
/// apply at this point in the chain (`current_difficulty_target`, tracked
/// identically here and in the local mining loop). Without this check, a
/// peer could gossip a block declaring a trivially easy target and it would
/// pass PoW verification against its own say-so -- checking a block against
/// a target it chose itself proves nothing.
///
/// This is the single validation path used for live gossip blocks, orphan
/// replays, and sync catch-up blocks alike -- no path takes a shortcut.
#[allow(clippy::too_many_arguments)]
fn validate_and_ingest(
    block: VeloBlock,
    storage: &BlockchainStorage,
    ghostdag: &mut GhostdagManager,
    orphans: &mut OrphanPool,
    block_history: &mut Vec<VeloBlock>,
    difficulty_log: &mut DifficultyLog,
    ledger_state: &mut LedgerState,
    difficulty_manager: &DifficultyManager,
    current_difficulty_target: &mut [u8; 32],
) -> Result<(), Box<dyn Error>> {
    let hash = block.calculate_hash();

    if storage.load_block(&hash)?.is_some() {
        return Ok(()); // already known, nothing to do
    }

    // Cheapest check first: reject a block stuffed with an implausible
    // number of transactions before doing any real validation work on it.
    if block.transactions.len() > MAX_TRANSACTIONS_PER_BLOCK {
        warn!(
            height = block.header.height,
            tx_count = block.transactions.len(),
            max = MAX_TRANSACTIONS_PER_BLOCK,
            "Rejected block: too many transactions"
        );
        return Ok(());
    }

    // 1. Coinbase / dev-tax split must match consensus rules exactly.
    if !block.verify_coinbase_rewards() {
        warn!(
            height = block.header.height,
            "Rejected block: bad coinbase split"
        );
        return Ok(());
    }

    // Transaction bytes are committed by the header and each sender must
    // prove ownership of the address included in the transaction.
    if !block.verify_transaction_merkle_root() {
        warn!(
            height = block.header.height,
            "Rejected block: transaction commitment mismatch"
        );
        return Ok(());
    }
    if block.transactions.iter().any(|tx| {
        !vdag_crypto::verify_transaction_signature(
            &tx.sender,
            &tx.recipient,
            tx.amount,
            tx.nonce,
            &tx.public_key,
            &tx.signature,
        )
    }) {
        warn!(
            height = block.header.height,
            "Rejected block: invalid transaction signature"
        );
        return Ok(());
    }

    // 2. Proof-of-work must satisfy the target the DAA actually expects at
    //    this point in the chain -- NOT whatever the block itself claims.
    //    A block declaring an easy target it made up would otherwise pass
    //    PoW verification trivially against its own self-selected value.
    if block.header.height > 0 {
        let expected_target = *current_difficulty_target;
        if block.header.difficulty_target != expected_target {
            warn!(
                height = block.header.height,
                "Rejected block: declared difficulty target does not match the expected consensus target"
            );
            return Ok(());
        }

        let pow = PowManager::new(expected_target);
        if !pow.verify_pow(&block) {
            warn!(
                height = block.header.height,
                "Rejected block: insufficient PoW"
            );
            return Ok(());
        }
        difficulty_log.record(block.header.height, expected_target);
    }

    // 3. Every parent must already be known locally (orphaning this block
    //    if not), and once parents are confirmed known: height must be
    //    exactly one more than the tallest parent (no gaps, no claiming a
    //    height that doesn't follow from the DAG), and timestamp must not
    //    precede the tallest parent's timestamp nor claim to be from more
    //    than MAX_FUTURE_DRIFT_SECS in the future. The future-drift check
    //    matters beyond plausibility: an implausible timestamp is exactly
    //    the kind of input that previously broke the DAA's difficulty
    //    calculation (see the overflow fix in daa.rs) -- rejecting it here
    //    stops a manipulated timestamp from ever reaching that math.
    let mut max_parent_height: Option<u64> = None;
    let mut max_parent_timestamp: Option<u64> = None;
    for parent in &block.header.parents {
        match ghostdag.block_store.get(parent) {
            Some(parent_block) => {
                max_parent_height = Some(
                    max_parent_height.map_or(parent_block.header.height, |h| {
                        h.max(parent_block.header.height)
                    }),
                );
                max_parent_timestamp = Some(
                    max_parent_timestamp.map_or(parent_block.header.timestamp, |t| {
                        t.max(parent_block.header.timestamp)
                    }),
                );
            }
            None => {
                info!(
                    height = block.header.height,
                    "Missing parent, buffering as orphan"
                );
                orphans.insert(*parent, block);
                return Ok(());
            }
        }
    }

    if block.header.height == 0 {
        if !block.header.parents.is_empty() {
            warn!("Rejected block: height 0 must have no parents");
            return Ok(());
        }
    } else {
        if block.header.parents.is_empty() {
            warn!(
                height = block.header.height,
                "Rejected block: non-genesis block has no parents"
            );
            return Ok(());
        }

        let expected_height = max_parent_height.unwrap() + 1;
        if block.header.height != expected_height {
            warn!(
                height = block.header.height,
                expected_height, "Rejected block: height is not parent height + 1"
            );
            return Ok(());
        }

        if let Some(parent_ts) = max_parent_timestamp {
            if block.header.timestamp < parent_ts {
                warn!(
                    height = block.header.height,
                    "Rejected block: timestamp precedes its parent's timestamp"
                );
                return Ok(());
            }
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if block.header.timestamp > now.saturating_add(MAX_FUTURE_DRIFT_SECS) {
            warn!(
                height = block.header.height,
                timestamp = block.header.timestamp,
                now,
                "Rejected block: timestamp too far in the future"
            );
            return Ok(());
        }
    }

    if let Err(reason) = ledger_state.apply_block(&block) {
        warn!(height = block.header.height, %reason, "Rejected block: invalid ledger state");
        return Ok(());
    }
    if let Err(reason) = storage.save_ledger_state(ledger_state) {
        warn!(height = block.header.height, %reason, "Rejected block: could not persist ledger state");
        return Ok(());
    }

    let ingested_hash = ingest_block_only(block, storage, ghostdag, block_history)?;

    // Ledger authority: recompute from the canonical chain rather than
    // trusting the incremental apply_block call above, which only reflects
    // arrival order. The apply_block call above remains useful as a cheap
    // pre-admission filter (reject obviously-bad blocks before they're even
    // ingested), but what actually gets persisted and relied upon from here
    // on is derived fresh from get_linear_sort of the real canonical tip --
    // not from "whatever order blocks were processed in."
    let canonical_tip_after = ghostdag.select_canonical_tip().unwrap_or(ingested_hash);
    match ghostdag.recompute_ledger(&canonical_tip_after) {
        Ok(recomputed) => {
            *ledger_state = recomputed;
            if let Err(reason) = storage.save_ledger_state(ledger_state) {
                warn!(%reason, "Failed to persist recomputed ledger state");
            }
        }
        Err(reason) => {
            warn!(%reason, "Ledger recompute failed after ingesting an already-validated block -- this indicates a consistency bug, not a normal rejection");
        }
    }

    // Difficulty: same fix, canonical order instead of arrival order.
    let canonical_blocks = ghostdag.canonical_block_order(&canonical_tip_after);
    *current_difficulty_target =
        difficulty_manager.calculate_next_target(&canonical_blocks, *current_difficulty_target);

    // A block landing may unblock orphans that were waiting specifically on
    // it. They go back through this same validate_and_ingest path, so
    // they're checked against their own height's expected target rather
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
            ledger_state,
            difficulty_manager,
            current_difficulty_target,
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
    local_network_id: u64,
) -> SyncResponse {
    if request.network_id != local_network_id || request.genesis_hash != genesis_hash {
        warn!(
            requested_network = request.network_id,
            local_network = local_network_id,
            "Peer requested sync on a different network or genesis -- refusing"
        );
        return SyncResponse::GenesisMismatch;
    }

    let blocks: Vec<VeloBlock> = block_history
        .iter()
        .filter(|b| b.header.height > request.since_height)
        .take(MAX_SYNC_BLOCKS)
        .cloned()
        .collect();

    info!(
        count = blocks.len(),
        since_height = request.since_height,
        "Sending sync response"
    );
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
    ledger_state: &mut LedgerState,
    difficulty_manager: &DifficultyManager,
    current_difficulty_target: &mut [u8; 32],
) -> Result<(), Box<dyn Error>> {
    match response {
        SyncResponse::Blocks(blocks) => {
            info!(count = blocks.len(), %peer, "Received sync catch-up blocks");
            for block in blocks {
                // Routed through the same validate_and_ingest as everything
                // else: coinbase check, transaction checks, difficulty
                // target authenticity check, PoW, and parent-existence /
                // orphan handling.
                validate_and_ingest(
                    block,
                    storage,
                    ghostdag,
                    orphans,
                    block_history,
                    difficulty_log,
                    ledger_state,
                    difficulty_manager,
                    current_difficulty_target,
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
