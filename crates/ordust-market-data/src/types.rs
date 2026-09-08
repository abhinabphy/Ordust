use core::error;
use std::{cmp::Reverse, collections::BTreeMap};
use thiserror::Error;

use ordust_core::types::*;
#[derive(Debug, Clone)]
pub struct DepthUpdate {
    pub first_update_id: u64,
    pub final_update_id: u64,
    pub bids: Vec<(Price, Qty)>,
    pub asks: Vec<(Price, Qty)>,
}
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub last_update_id: u64,
    pub bids: Vec<(Price, Qty)>,
    pub asks: Vec<(Price, Qty)>,
}
#[derive(Debug, Clone)]
pub struct TradeTick {
    pub price: Price,
    pub qty: Qty,
    pub timestamp: Timestamp,
    pub is_buyer_maker: bool, // needed later by queue-model to attribute trade side
}

#[derive(Debug, Error)]

pub enum WsError {
    #[error("invalid url {0:?}")]
    InvalidUrl(String),
    #[error("connection failed for  {0:?}")]
    ConnectionFailed(String),
    #[error("send failed while transmitting adapter subscription payload ")]
    SendFailed(String),
    #[error("parsing error for {0:?}")]
    ParseError(String)
}
