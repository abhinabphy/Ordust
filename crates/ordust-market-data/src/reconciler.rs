use crate::{MarketBook, types::{DepthUpdate,Snapshot}};

#[derive(Debug,Clone)]
pub enum SyncState {
    Buffering (Vec<DepthUpdate>),
    Synced {book:MarketBook},
}

pub enum ReconcileError {
    GapDetected {expected:u64, got:u64},
    SnapshotTooOld,//no buffered event bridges the snapshots lastUpdateId
}
#[derive(Debug,Clone)]
pub struct Reconciler{
    state:SyncState,
}

impl Reconciler {
    pub fn new() -> Self {
        Reconciler {
            state: SyncState::Buffering(Vec::new()),
        }
    }
    
    /// Every message off the websocket passes through here first, always —
    /// buffered if not yet synced, applied-with-gap-check if synced.
    pub fn on_diff(&mut self,update:DepthUpdate) -> Result<(),ReconcileError>{
       
       if
        todo!()
    }

    /// Called once a REST snapshot arrives. Drops stale buffered events,
    /// validates the first applicable event bridges the snapshot correctly,
    /// transitions Buffering -> Synced.
    pub fn on_snapshot(&mut self, snapshot: Snapshot) -> Result<(), ReconcileError>{
        todo!()
    }

    /// On any ReconcileError: caller must discard `state` entirely and
    /// re-request a fresh snapshot. Do not attempt to "heal" a gap.
    pub fn book(&self) -> Option<&MarketBook>{
        todo!()
    }



}
