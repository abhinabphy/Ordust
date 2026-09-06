use std::{cmp::Reverse, collections::BTreeMap};

use ordust_core::types::*;
pub struct MarketBook {
    bids: BTreeMap<Reverse<Price>, Qty>,
    asks: BTreeMap<Price, Qty>, // qty == 0 means "remove this price level"
    last_update_id: u64,
}

pub struct DepthUpdate {
    pub first_update_id:u64,
    pub final_update_id:u64,
    pub bids:Vec<(Price,Qty)>,
    pub asks:Vec<(Price,Qty)>,
}

pub struct Snapshot {
    pub last_update_id: u64,
    pub bids:Vec<(Price,Qty)>,
    pub asks:Vec<(Price,Qty)>,
}

pub struct TradeTick {
    pub price:Price,
    pub qty:Qty,
    pub timestamp:Timestamp,
    pub is_buyer_maker:bool, // needed later by queue-model to attribute trade side

}

