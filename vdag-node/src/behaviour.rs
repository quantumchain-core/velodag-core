// vdag-node/src/behaviour.rs
//
// Composed libp2p NetworkBehaviour for VeloDAG:
//   - gossipsub: block propagation (pub/sub)
//   - mdns:      LAN peer auto-discovery, so you don't have to manually
//                copy/paste multiaddrs between local test nodes
//   - sync:      request/response catch-up protocol for newly joined or
//                reconnected peers (see sync.rs)
//
// #[derive(NetworkBehaviour)] auto-generates `VeloBehaviourEvent`, an enum
// with one variant per field (Gossipsub(..), Mdns(..), Sync(..)) -- that's
// the type network.rs matches on.

use libp2p::{gossipsub, mdns, request_response, swarm::NetworkBehaviour};

use crate::sync::{SyncRequest, SyncResponse};

pub type SyncBehaviour = request_response::json::Behaviour<SyncRequest, SyncResponse>;

#[derive(NetworkBehaviour)]
pub struct VeloBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
    pub sync: SyncBehaviour,
}
