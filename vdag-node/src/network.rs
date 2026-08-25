// vdag-node/src/network.rs

use std::error::Error;
use libp2p::{
    gossipsub::{Event, Message, IdentTopic},
    swarm::SwarmEvent,
};
use vdag_consensus::{VeloBlock, BlockchainStorage, ghostdag::GhostdagManager};

/// NETWORK TOPOLOGY GATES: Handles all asynchronous incoming network packets from remote peers
pub fn handle_p2p_events(
    event: SwarmEvent<libp2p::gossipsub::BehaviourEvent>,
    storage_engine: &BlockchainStorage,
    ghostdag: &mut GhostdagManager,
) -> Result<(), Box<dyn Error>> {
    match event {
        SwarmEvent::Behaviour(Event::Message { propagation_source: _, message_id: _, message }) => {
            process_incoming_packet(message, storage_engine, ghostdag)?;
        }
        SwarmEvent::NewListenAddr { address, .. } => {
            println!("🌐 [P2P Network] Local Node Listening Live on: {}", address);
        }
        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            println!("🤝 [P2P Network] Remote Connection Settled with Peer: {}", peer_id);
        }
        _ => {}
    }
    Ok(())
}

/// Packet decoding and validation matrix
fn process_incoming_packet(
    message: Message,
    storage: &BlockchainStorage,
    ghostdag: &mut GhostdagManager,
) -> Result<(), Box<dyn Error>> {
    // Check if the received topic matches our global consensus transaction/block rail
    if message.topic == IdentTopic::new("vdag-blocks").hash() {
        // Deserialize the raw incoming network payload bytes back into a VeloBlock struct
        if let Ok(incoming_block) = bincode::deserialize::<VeloBlock>(&message.data) {
            let incoming_hash = incoming_block.calculate_hash();

            // Validate that we do not already have this block persisted on disk
            if storage.load_block(&incoming_hash)?.is_none() {
                println!(
                    "📥 [Network Influx] Received New Block from Peer! Height: {}, Hash: 0x{:02x}{:02x}...",
                    incoming_block.header.height, incoming_hash[0], incoming_hash[1]
                );

                // Calculate the topological sorting properties of the block relative to our DAG graph state
                let dag_data = ghostdag.calculate_ghostdag_data(&incoming_block, incoming_hash);

                // Commit the peer's block state directly down to physical disk
                storage.save_block(&incoming_hash, &incoming_block)?;
                storage.save_ghostdag_data(&incoming_hash, &dag_data)?;

                // Cache it into memory so our mining loop references it as a parent tip in the next interval
                ghostdag.block_store.insert(incoming_hash, incoming_block);
                ghostdag.ghostdag_cache.insert(incoming_hash, dag_data);
            }
        }
    }
    Ok(())
}
