// vdag-node/src/sync.rs
//
// Block-sync request/response protocol types, plus an orphan block pool.
//
// SYNC PROTOCOL:
// When a connection is established with a peer, we send a SyncRequest asking
// for any blocks past our current tip height. The genesis_hash field doubles
// as a lightweight network-magic check: if a peer's genesis doesn't match
// ours, we know it's on a different chain/testnet and refuse to sync with it.
//
// ORPHAN POOL:
// At 1-second block times, it's common for a block to arrive over gossip
// before one of its parents does (network jitter, processing order, etc).
// Rather than dropping such a block, we park it here keyed by the parent
// hash it's waiting on, and replay it once that parent is accepted.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use vdag_consensus::VeloBlock;

/// Caps how many blocks we hand back in a single sync response, so a
/// malicious/buggy peer can't force us to serialize the entire chain
/// history in one shot.
pub const MAX_SYNC_BLOCKS: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequest {
    /// Acts as a network-magic check -- peers on a different genesis are
    /// rejected rather than silently corrupting our DAG.
    pub genesis_hash: [u8; 32],
    /// "Send me everything you have past this height."
    pub since_height: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncResponse {
    Blocks(Vec<VeloBlock>),
    /// Returned instead of blocks when the requester's genesis_hash doesn't
    /// match ours.
    GenesisMismatch,
}

/// Blocks buffered because at least one parent wasn't found locally yet.
///
/// NOTE: this is a testnet-grade simplification -- a block is keyed by only
/// the *first* missing parent encountered. A block missing multiple parents
/// will be re-attempted when any one of them resolves (harmless: it will
/// just fail the parent check again and get re-buffered), so correctness is
/// preserved, but it does mean occasional redundant validation passes.
pub struct OrphanPool {
    waiting_on: HashMap<[u8; 32], Vec<VeloBlock>>,
    max_size: usize,
    current_size: usize,
}

impl OrphanPool {
    pub fn new(max_size: usize) -> Self {
        Self {
            waiting_on: HashMap::new(),
            max_size,
            current_size: 0,
        }
    }

    /// Buffer `block`, which is missing `missing_parent` locally.
    pub fn insert(&mut self, missing_parent: [u8; 32], block: VeloBlock) {
        if self.current_size >= self.max_size {
            // Simple backpressure: refuse new orphans rather than growing
            // unbounded under a flood of blocks with bad/missing parents.
            println!("⚠️ [Orphan Pool] Full ({} entries) -- dropping orphan.", self.max_size);
            return;
        }
        self.waiting_on.entry(missing_parent).or_default().push(block);
        self.current_size += 1;
    }

    /// Call after `resolved_hash` has been accepted into the DAG. Returns
    /// any orphans that were specifically waiting on it, for the caller to
    /// re-attempt validation/ingestion.
    pub fn take_ready(&mut self, resolved_hash: &[u8; 32]) -> Vec<VeloBlock> {
        match self.waiting_on.remove(resolved_hash) {
            Some(ready) => {
                self.current_size = self.current_size.saturating_sub(ready.len());
                ready
            }
            None => Vec::new(),
        }
    }
}
