use std::mem;

use ordust_core::OrderBook;

use crate::{
    MarketBook,
    SyncState::{Buffering, Synced},
    types::{DepthUpdate, Snapshot},
};

#[derive(Debug, Clone)]
pub enum SyncState {
    Buffering(Vec<DepthUpdate>),
    Synced { book: MarketBook },
}
#[derive(Debug)]
pub enum ReconcileError {
    GapDetected { expected: u64, got: u64 },
    SnapshotTooOld, //no buffered event bridges the snapshots lastUpdateId
}
#[derive(Debug, Clone)]
pub struct Reconciler {
    state: SyncState,
}

impl Reconciler {
    pub fn new() -> Self {
        Reconciler {
            state: SyncState::Buffering(Vec::new()),
        }
    }

    /// Every message off the websocket passes through here first, always —
    /// buffered if not yet synced, applied-with-gap-check if synced.
    pub fn on_diff(&mut self, update: DepthUpdate) -> Result<(), ReconcileError> {
        match &mut self.state {
            Buffering(bufferqueue) => {
                bufferqueue.push(update);
                Ok(())
            }
            Synced { book } => {
                let expected = book.last_update_id() + 1;
                if update.first_update_id == expected {
                    book.apply_diff(&update);
                    Ok(())
                } else {
                    self.state = SyncState::Buffering(Vec::new());
                    Err(ReconcileError::GapDetected {
                        expected: expected,
                        got: update.first_update_id,
                    })
                }
            }
        }
    }

    /// Called once a REST snapshot arrives. Drops stale buffered events,
    /// validates the first applicable event bridges the snapshot correctly,
    /// transitions Buffering -> Synced.
    pub fn on_snapshot(&mut self, snapshot: Snapshot) -> Result<(), ReconcileError> {
        let bufferqueue = match &mut self.state {
            SyncState::Buffering(buf) => buf,
            SyncState::Synced { .. } => return Ok(()),
        };
        let seq = snapshot.last_update_id + 1;
        let straddle_idx = bufferqueue
            .iter()
            .position(|d| d.first_update_id <= seq && d.final_update_id >= seq);
        let straddle_idx = match straddle_idx {
            Some(idx) => idx,
            None => {
                *bufferqueue = Vec::new();
                return Err(ReconcileError::SnapshotTooOld);
            }
        };
        let mut marketbook = MarketBook::new();
        marketbook.apply_snapshot(snapshot);

        for depths in bufferqueue[straddle_idx..].iter() {
            let expected = marketbook.last_update_id() + 1;
            let got=depths.first_update_id;
            if depths.first_update_id <= expected && depths.final_update_id >= expected {
                marketbook.apply_diff(depths);
            } else {
                self.state = SyncState::Buffering(Vec::new());
                return Err(ReconcileError::GapDetected {
                     expected,
                     got
                });
            }
        }
        // 5. Successfully bridged and caught up
        self.state = SyncState::Synced { book: marketbook };
        Ok(())
    }

    /// On any ReconcileError: caller must discard `state` entirely and
    /// re-request a fresh snapshot. Do not attempt to "heal" a gap.
    pub fn book(&self) -> Option<&MarketBook> {
        match &self.state{
            SyncState::Synced { book }=>Some(book),
            SyncState::Buffering(_)=>None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DepthUpdate, Snapshot};
    use ordust_core::types::*;

    // Helper functions to construct lightweight test payloads
   fn make_snapshot(last_update_id: u64) -> Snapshot {
    Snapshot {
        last_update_id,
        bids: vec![(Price(100), Qty(1))],
        asks: vec![(Price(101), Qty(2))],
    }
}

    fn make_diff(first_update_id: u64, final_update_id: u64) -> DepthUpdate {
        DepthUpdate {
            first_update_id,
            final_update_id,
            bids: vec![],
            asks: vec![],
        }
    }

    #[test]
    fn test_successful_snapshot_bridging() {
        let mut reconciler = Reconciler::new();

        // 1. Buffer messages before snapshot arrives
        // Message 1: Stale (will be ignored)
        reconciler.on_diff(make_diff(90, 95)).unwrap();
        // Message 2: Straddle event (snapshot is at 100, so target_seq = 101)
        reconciler.on_diff(make_diff(96, 105)).unwrap();
        // Message 3: Contiguous update following straddle
        reconciler.on_diff(make_diff(106, 110)).unwrap();

        assert!(reconciler.book().is_none(), "Should be in Buffering state");

        // 2. Apply snapshot as of ID 100
        let result = reconciler.on_snapshot(make_snapshot(100));

        assert!(result.is_ok());
        let book = reconciler.book().expect("Reconciler should now be Synced");
        assert_eq!(book.last_update_id(), 110);
    }

    #[test]
    fn test_stale_snapshot_too_old() {
        let mut reconciler = Reconciler::new();

        // Buffer starts at 105, missing the 101 target sequence
        reconciler.on_diff(make_diff(105, 110)).unwrap();

        // Snapshot is at 100 (target_seq = 101)
        let err = reconciler.on_snapshot(make_snapshot(100)).unwrap_err();

        assert!(matches!(err, ReconcileError::SnapshotTooOld));
        assert!(reconciler.book().is_none(), "State must remain Buffering on error");
    }

    #[test]
    fn test_sequence_gap_during_snapshot_bridging() {
        let mut reconciler = Reconciler::new();

        // Straddle event is valid, but the subsequent buffered update drops a range
        reconciler.on_diff(make_diff(95, 102)).unwrap(); // Straddles 101
        reconciler.on_diff(make_diff(105, 110)).unwrap(); // Gap! Expected 103, got 105

        let err = reconciler.on_snapshot(make_snapshot(100)).unwrap_err();

        assert!(matches!(
            err,
            ReconcileError::GapDetected {
                expected: 103,
                got: 105
            }
        ));
        assert!(reconciler.book().is_none(), "Must drop state back to Buffering");
    }

    #[test]
    fn test_sequence_gap_on_live_diff() {
        let mut reconciler = Reconciler::new();

        // Sync first: snapshot at 100, buffer covers [95..102]
        reconciler.on_diff(make_diff(95, 102)).unwrap();
        reconciler.on_snapshot(make_snapshot(100)).unwrap();
        assert_eq!(reconciler.book().unwrap().last_update_id(), 102);

        // Send contiguous live diff (103)
        reconciler.on_diff(make_diff(103, 105)).unwrap();
        assert_eq!(reconciler.book().unwrap().last_update_id(), 105);

        // Send gapped live diff (expected 106, got 108)
        let err = reconciler.on_diff(make_diff(108, 112)).unwrap_err();

        assert!(matches!(
            err,
            ReconcileError::GapDetected {
                expected: 106,
                got: 108
            }
        ));
        // Ensure state reset wiped the book
        assert!(reconciler.book().is_none());
    }
}