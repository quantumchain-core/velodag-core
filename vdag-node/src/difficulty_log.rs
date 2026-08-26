// vdag-node/src/difficulty_log.rs
//
// Closes a validation gap: the live gossip path always checks an incoming
// block's PoW against `current_difficulty_target` (correct, since gossip
// blocks arrive close to real time). But orphan replays and sync catch-up
// blocks can be validated *after* the difficulty target has already moved
// on to a later value -- checking them against "current" would be checking
// them against the wrong rule.
//
// This log records, for every height we've locally confirmed a target for,
// exactly which target was in force. Validation then looks up the target
// for a block's specific height instead of assuming "current" is correct.

use std::collections::HashMap;

#[derive(Default)]
pub struct DifficultyLog {
    targets_by_height: HashMap<u64, [u8; 32]>,
}

impl DifficultyLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the target that was in force at `height`. Idempotent --
    /// first recording for a height wins, since that's the one blocks at
    /// that height were actually mined/validated against.
    pub fn record(&mut self, height: u64, target: [u8; 32]) {
        self.targets_by_height.entry(height).or_insert(target);
    }

    /// Looks up the target that was active at `height`, if known.
    pub fn get(&self, height: u64) -> Option<[u8; 32]> {
        self.targets_by_height.get(&height).copied()
    }
}
