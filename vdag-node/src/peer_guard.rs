// vdag-node/src/peer_guard.rs
//
// Peer-level connection limits, malformed-message/invalid-block violation
// tracking, and banning. Bundled into one struct rather than several more
// loose parameters threaded through an already-long handle_p2p_events
// signature.
//
// Deliberately hand-rolled rather than using libp2p's own
// `libp2p-connection-limits` crate: that crate exists and would be the
// more idiomatic choice, but this session had no way to compile-check
// against it, and getting a feature-flag name or API detail wrong there
// would waste a full round discovering it via a failed build. Counting
// connections and violations in a couple of HashMaps is simple enough to
// be confident in without that verification step.
//
// Bans expire after BAN_DURATION rather than lasting for the life of the
// process. A permanent ban is too risky once real, independently-operated
// machines are involved -- a transient bug or a brief bad connection
// shouldn't permanently lock a legitimate peer out with no recovery path.
// An explicit `unban` is also provided for manual recovery if needed
// sooner than the automatic expiry.
//
// Known simplification, flagged rather than hidden: violation counts do
// NOT decay over time on their own (only the resulting ban does). A peer
// that had a couple of old, long-past violations is still just one fresh
// violation away from a ban, rather than those old counts aging out. Given
// the current threshold (5) and the fact that a ban itself now recovers,
// this was judged an acceptable simplification rather than adding a second
// time-based decay mechanism on top of ban expiry in the same round.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use libp2p::PeerId;

const MAX_CONNECTIONS_PER_PEER: u32 = 2;
const MAX_TOTAL_CONNECTIONS: u32 = 128;
const MAX_VIOLATIONS_BEFORE_BAN: u32 = 5;
/// How long a ban lasts before the peer is automatically allowed to
/// reconnect. One hour is long enough to matter as a real deterrent,
/// short enough that a mistaken or transient ban doesn't permanently
/// strand a legitimate operator.
const BAN_DURATION: Duration = Duration::from_secs(60 * 60);

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
    banned_until: HashMap<PeerId, Instant>,
}

impl PeerGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether `peer` is currently banned. Expired bans are treated as not
    /// banned (and lazily cleaned up here) rather than requiring a
    /// separate sweep pass.
    pub fn is_banned(&mut self, peer: &PeerId) -> bool {
        match self.banned_until.get(peer) {
            Some(&expiry) if Instant::now() < expiry => true,
            Some(_) => {
                // Ban has expired -- clean it up and give the peer a clean
                // slate, including their violation count, so an old,
                // already-served ban doesn't leave them one violation from
                // an instant re-ban.
                self.banned_until.remove(peer);
                self.violations.remove(peer);
                false
            }
            None => false,
        }
    }

    /// Manually clears a ban immediately, without waiting for expiry.
    pub fn unban(&mut self, peer: &PeerId) {
        self.banned_until.remove(peer);
        self.violations.remove(peer);
    }

    /// Call on `SwarmEvent::ConnectionEstablished`, before doing anything
    /// else with this peer. If this returns anything other than `Allow`,
    /// the caller must immediately disconnect the peer and skip any
    /// further per-connection setup (e.g. sending a sync request).
    pub fn on_connection_established(&mut self, peer: PeerId) -> ConnectionDecision {
        if self.is_banned(&peer) {
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

    /// Records a malformed message or invalid block attributed to `peer`.
    /// Returns `true` if this violation just pushed the peer over the ban
    /// threshold -- the caller should disconnect them immediately when
    /// this returns true; `is_banned` will report `true` for this peer
    /// until the ban expires or is manually cleared.
    pub fn record_violation(&mut self, peer: PeerId) -> bool {
        let count = self.violations.entry(peer).or_insert(0);
        *count += 1;
        if *count >= MAX_VIOLATIONS_BEFORE_BAN {
            self.banned_until.insert(peer, Instant::now() + BAN_DURATION);
            true
        } else {
            false
        }
    }

    /// Peers currently under a ban, for diagnostics/operator visibility.
    /// Does not trigger expiry cleanup (use `is_banned` per-peer for that).
    pub fn currently_banned(&self) -> HashSet<PeerId> {
        let now = Instant::now();
        self.banned_until
            .iter()
            .filter(|(_, &expiry)| now < expiry)
            .map(|(&peer, _)| peer)
            .collect()
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

    /// Regression test for the recovery mechanism itself: manually
    /// unbanning a peer clears both the ban and their violation count,
    /// giving them a genuine clean slate rather than leaving them one
    /// violation away from an instant re-ban.
    #[test]
    fn unban_clears_both_the_ban_and_the_violation_count() {
        let mut guard = PeerGuard::new();
        let peer = PeerId::random();

        for _ in 0..MAX_VIOLATIONS_BEFORE_BAN {
            guard.record_violation(peer);
        }
        assert!(guard.is_banned(&peer));

        guard.unban(&peer);
        assert!(!guard.is_banned(&peer));

        // A single fresh violation must not immediately re-ban -- proves
        // the old count was actually cleared, not just the ban flag.
        assert!(!guard.record_violation(peer));
    }

    #[test]
    fn currently_banned_reports_active_bans() {
        let mut guard = PeerGuard::new();
        let peer = PeerId::random();

        assert!(guard.currently_banned().is_empty());
        for _ in 0..MAX_VIOLATIONS_BEFORE_BAN {
            guard.record_violation(peer);
        }
        assert!(guard.currently_banned().contains(&peer));
    }
    }
    
