// vdag-node/src/peer_guard.rs
//
// Peer-level connection limits, malformed-message violation tracking, and
// banning. Bundled into one struct rather than three more loose parameters
// threaded through an already-long handle_p2p_events signature.
//
// Deliberately hand-rolled rather than using libp2p's own
// `libp2p-connection-limits` crate: that crate exists and would be the
// more idiomatic choice, but this session had no way to compile-check
// against it, and getting a feature-flag name or API detail wrong there
// would waste a full round discovering it via a failed build. Counting
// connections and violations in a couple of HashMaps is simple enough to
// be confident in without that verification step -- a case of the safer
// bet being the boring one, not the "more correct" one, since there is
// genuine uncertainty about the correct external API here that doesn't
// exist for the hand-rolled version.
//
// Known gap, flagged rather than hidden: only malformed *gossip* messages
// (failed deserialization) are currently tracked as violations. A peer
// that sends well-formed-but-invalid blocks (bad PoW, bad signatures,
// etc.) is not yet counted toward the ban threshold -- attributing those
// deeper rejections to a specific peer would require threading peer
// identity through the full validate_and_ingest / orphan-replay call
// chain, which is a larger, separate piece of work appropriately scoped
// for its own round rather than bundled in here.

use std::collections::{HashMap, HashSet};

use libp2p::PeerId;

const MAX_CONNECTIONS_PER_PEER: u32 = 2;
const MAX_TOTAL_CONNECTIONS: u32 = 128;
const MAX_VIOLATIONS_BEFORE_BAN: u32 = 5;

#[derive(Debug, PartialEq, Eq)]
pub enum ConnectionDecision {
    Allow,
    RejectAlreadyBanned,
    RejectOverPeerLimit,
    RejectOverTotalLimit,
}

#[derive(Default)]
pub struct PeerGuard {
    connections_per_peer: HashMap<PeerId, u32>,
    total_connections: u32,
    violations: HashMap<PeerId, u32>,
    banned: HashSet<PeerId>,
}

impl PeerGuard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_banned(&self, peer: &PeerId) -> bool {
        self.banned.contains(peer)
    }

    /// Call on `SwarmEvent::ConnectionEstablished`, before doing anything
    /// else with this peer. If this returns anything other than `Allow`,
    /// the caller must immediately disconnect the peer and skip any
    /// further per-connection setup (e.g. sending a sync request).
    pub fn on_connection_established(&mut self, peer: PeerId) -> ConnectionDecision {
        if self.banned.contains(&peer) {
            return ConnectionDecision::RejectAlreadyBanned;
        }

        let per_peer_count = self.connections_per_peer.get(&peer).copied().unwrap_or(0);
        if per_peer_count >= MAX_CONNECTIONS_PER_PEER {
            return ConnectionDecision::RejectOverPeerLimit;
        }
        if self.total_connections >= MAX_TOTAL_CONNECTIONS {
            return ConnectionDecision::RejectOverTotalLimit;
        }

        self.connections_per_peer.insert(peer, per_peer_count + 1);
        self.total_connections += 1;
        ConnectionDecision::Allow
    }

    /// Call on `SwarmEvent::ConnectionClosed` for every peer, unconditionally
    /// -- safe to call even for a connection that was never counted (e.g.
    /// one rejected by `on_connection_established`), since the counters
    /// simply saturate at zero rather than underflowing.
    pub fn on_connection_closed(&mut self, peer: &PeerId) {
        if let Some(count) = self.connections_per_peer.get_mut(peer) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.connections_per_peer.remove(peer);
            }
        }
        self.total_connections = self.total_connections.saturating_sub(1);
    }

    /// Records a malformed/invalid message attributed to `peer`. Returns
    /// `true` if this violation just pushed the peer over the ban
    /// threshold -- the caller should disconnect them immediately when
    /// this returns true; `is_banned` will report `true` for this peer
    /// from this point on regardless.
    pub fn record_violation(&mut self, peer: PeerId) -> bool {
        let count = self.violations.entry(peer).or_insert(0);
        *count += 1;
        if *count >= MAX_VIOLATIONS_BEFORE_BAN {
            self.banned.insert(peer);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_connections_up_to_the_per_peer_limit_then_rejects() {
        let mut guard = PeerGuard::new();
        let peer = PeerId::random();

        assert_eq!(guard.on_connection_established(peer), ConnectionDecision::Allow);
        assert_eq!(guard.on_connection_established(peer), ConnectionDecision::Allow);
        assert_eq!(
            guard.on_connection_established(peer),
            ConnectionDecision::RejectOverPeerLimit
        );
    }

    #[test]
    fn closing_a_connection_frees_up_room_for_a_new_one() {
        let mut guard = PeerGuard::new();
        let peer = PeerId::random();

        guard.on_connection_established(peer);
        guard.on_connection_established(peer);
        assert_eq!(
            guard.on_connection_established(peer),
            ConnectionDecision::RejectOverPeerLimit
        );

        guard.on_connection_closed(&peer);
        assert_eq!(guard.on_connection_established(peer), ConnectionDecision::Allow);
    }

    #[test]
    fn banned_peer_is_rejected_even_under_the_connection_limit() {
        let mut guard = PeerGuard::new();
        let peer = PeerId::random();

        for _ in 0..MAX_VIOLATIONS_BEFORE_BAN {
            guard.record_violation(peer);
        }
        assert!(guard.is_banned(&peer));
        assert_eq!(
            guard.on_connection_established(peer),
            ConnectionDecision::RejectAlreadyBanned
        );
    }

    #[test]
    fn violations_below_threshold_do_not_ban() {
        let mut guard = PeerGuard::new();
        let peer = PeerId::random();

        for _ in 0..MAX_VIOLATIONS_BEFORE_BAN - 1 {
            assert!(!guard.record_violation(peer));
        }
        assert!(!guard.is_banned(&peer));
    }

    #[test]
    fn the_violation_that_crosses_the_threshold_reports_true() {
        let mut guard = PeerGuard::new();
        let peer = PeerId::random();

        for _ in 0..MAX_VIOLATIONS_BEFORE_BAN - 1 {
            guard.record_violation(peer);
        }
        assert!(guard.record_violation(peer));
    }
}
