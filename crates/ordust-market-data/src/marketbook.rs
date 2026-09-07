use crate::types::{DepthUpdate, Snapshot};
use ordust_core::{
    Side::{Buy, Sell},
    types::*,
};
use std::cmp::Reverse;
use std::collections::BTreeMap;

// impl MarketBook {
//     pub fn new() -> Self;
//     pub fn apply_snapshot(&mut self, snapshot: Snapshot);
//     pub fn apply_diff(&mut self, update: &DepthUpdate);  // no return — reconciler owns validity checking

//     pub fn best_bid(&self) -> Option<(Price, Qty)>;
//     pub fn best_ask(&self) -> Option<(Price, Qty)>;
//     pub fn depth(&self, side: Side, n: usize) -> Vec<(Price, Qty)>;
//     pub fn last_update_id(&self) -> u64;
// }
#[derive(Debug, Clone)]
pub struct MarketBook {
    bids: BTreeMap<Reverse<Price>, Qty>,
    asks: BTreeMap<Price, Qty>, // qty == 0 means "remove this price level"
    last_update_id: u64,
}

impl MarketBook {
    pub fn new() -> Self {
        MarketBook {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            last_update_id: 0,
        }
    }
    ///Clears both maps entirely and populates baseline depth
    /// from a REST snapshot. Sets the initial last_update_id
    pub fn apply_snapshot(&mut self, snapshot: Snapshot) {
        self.bids.clear();
        self.asks.clear();

        for (price, qty) in snapshot.bids {
            if qty.0 > 0 {
                self.bids.insert(Reverse(price), qty);
            }
        }
        for (price, qty) in snapshot.asks {
            if qty.0 > 0 {
                self.asks.insert(price, qty);
            }
        }
        self.last_update_id = snapshot.last_update_id;
    }
    ///Processes streaming WS updates. Overwrites existing price level quantities
    /// or removes levels where qty == 0. Updates last_update_id to final_update_id
    pub fn apply_diff(&mut self, update: &DepthUpdate) {
        for (price, qty) in &update.bids {
            if qty.0 > 0 {
                self.bids.insert(Reverse(*price), *qty);
            } else {
                self.bids.remove(&Reverse(*price));
            }
        }
        for (price, qty) in &update.asks {
            if qty.0 > 0 {
                self.asks.insert(*price, *qty);
            } else {
                self.asks.remove(price);
            }
        }
        self.last_update_id = update.final_update_id;
    }

    pub fn depth(&self, side: Side, n: usize) -> Vec<(Price, Qty)> {
        match side {
            Sell => self.asks.iter().take(n).map(|(p, q)| (*p, *q)).collect(),
            Buy => self
                .bids
                .iter()
                .take(n)
                .map(|(Reverse(p), q)| (*p, *q))
                .collect(),
        }
    }

    pub fn best_bid(&self) -> Option<(Price, Qty)> {
        self.bids
            .iter()
            .next()
            .map(|(Reverse(price), qty)| (*price, *qty))
    }
    pub fn best_ask(&self) -> Option<(Price, Qty)> {
        self.asks.iter().next().map(|(price, qty)| (*price, *qty))
    }

    pub fn last_update_id(&self) -> u64 {
        self.last_update_id
    }
}
