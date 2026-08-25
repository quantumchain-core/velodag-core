use libp2p::{
    gossipsub, mdns, noise, tcp, yamux, SwarmBuilder, PeerId
};
use libp2p::swarm::SwarmEvent;
use futures::StreamExt;
use std::error::Error;
use std::time::Duration;

// Construct a custom behavior combining peer discovery (mDNS) and messaging (Gossipsub)
#[derive(libp2p::swarm::NetworkBehaviour)]
pub struct VeloNetworkBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
}

/// Initializes an open, secure P2P connection engine
pub async fn start_p2p_engine() -> Result<(), Box<dyn Error>> {
    // 1. Generate an identity keypair for this specific computer instance
    let local_key = libp2p::identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("[🌐 P2P Network] Generating node network ID: {}", local_peer_id);

    // 2. Configure a low-level encrypted TCP connection channel
    let mut swarm = SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|key| {
            // Build the Gossipsub broadcast network channel
            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(1))
                .validation_mode(gossipsub::ValidationMode::Strict)
                .build()?;
            let gossipsub = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )?;

            // Build the local multicast discovery module
            let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;

            Ok(VeloNetworkBehaviour { gossipsub, mdns })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    // 3. Define and subscribe to the primary block propagation network lane
    let block_topic = gossipsub::IdentTopic::new("vdag-blocks");
    swarm.behaviour_mut().gossipsub.subscribe(&block_topic)?;

    // 4. Force open an incoming network port on port 4001
    swarm.listen_on("/ip4/0.0.0.0/tcp/4001".parse()?)?;
    println!("[🌐 P2P Network] Listening for peer connections on standard port 4001...");

    // 5. Fire an asynchronous background task listener loop to handle network events
    tokio::spawn(async move {
        loop {
            match swarm.select_next_some().await {
                // CORRECTED: In libp2p 0.53, the macro events are nested inside an inner Event enum under your struct name
                SwarmEvent::Behaviour(VeloNetworkBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, multiaddr) in list {
                        println!("[📡 Discovery] Found active peer node: {} at {}", peer_id, multiaddr);
                        let _ = swarm.dial(multiaddr);
                    }
                }
                SwarmEvent::Behaviour(VeloNetworkBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                    for (peer_id, _multiaddr) in list {
                        println!("[📡 Discovery] Peer node connection lost: {}", peer_id);
                    }
                }
                SwarmEvent::Behaviour(VeloNetworkBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source,
                    message_id,
                    ..
                })) => {
                    println!(
                        "[📥 P2P Message] Received broadcast data hash: {} from peer: {}",
                        message_id, propagation_source
                    );
                }
                _ => {}
            }
        }
    });

    Ok(())
}
