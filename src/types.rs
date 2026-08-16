use std::ops::AddAssign;
use std::ops::Sub;
use thiserror::Error;

//Custom Types
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Price(pub u64); // integer ticks, NOT float — avoid rounding bugs

impl Sub for Price {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        Self(self.0 - other.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Qty(pub u64); // base-asset units, smallest denomination

impl AddAssign for Qty {
    fn add_assign(&mut self, rhs: Self) {
        self.0 = self.0 + rhs.0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderId(pub u64); // Hash+Eq required — used as HashMap key

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(pub u64); // nanos. Caller-supplied, engine never calls SystemTime.

//Ord on Price/Timestamp is required , sort by both later (price priority, then time priority within a price).

//Enums
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
}
///GTC (Good 'Til Canceled): The order stays open in the market until someone buys or sells it, or you manually cancel it.
///
///IOC (Immediate or Cancel): The system tries to fill the order right now. If it cannot match part or all of it immediately, it cancels the leftover part.
///
///FOK (Fill or Kill): The system must fill the entire order right this second. If it cannot match 100% of the volume right away, it cancels the whole order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    GTC,
    IOC,
    FOK,
}
//NOTE:Design choice: qty vs remaining_qty as two fields, not one mutated field.
// want to report "filled 60 of 100" later (for  MM bot's fill tracking) —
//if  keep one mutable field lose the original size.

///id & side: Unique identifier for tracking, and the direction (Buy/Bid vs Sell/Ask).
///
/// price: Wrapped in an Option. It is Some(Price) for Limit orders, but None for Market orders because they aggressively execute at whatever price is currently available.
///
/// timestamp: Critical for fairness. If two orders sit at the exact same price level, the matching engine uses this timestamp to execute the older order first (FIFO priority).
#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    pub id: OrderId,
    pub side: Side,
    pub order_type: OrderType,
    pub tif: TimeInForce,
    pub price: Option<Price>, // None for Market orders, Some for Limit
    pub qty: Qty,             // original quantity, never mutated
    pub remaining_qty: Qty,   // decremented on each partial fill
    pub timestamp: Timestamp, // for FIFO priority within a price level
}

//NOTE:Design note: trade price = maker's price, always. This matters a lot — it's the rule real exchanges use,
//and  MM bot will eventually reason about "did I get price improvement" based on this convention.
#[derive(Debug, Clone)]
pub struct Trade {
    pub maker_order_id: OrderId, // the resting order that got hit
    pub taker_order_id: OrderId, // the incoming order that crossed the spread
    pub price: Price,            // always the MAKER's price — standard convention
    pub qty: Qty,
    pub timestamp: Timestamp,
}

#[derive(Debug, Clone)]
pub struct FillResult {
    pub maker_order_id: OrderId,
    pub price: Price,
    pub filled_qty: Qty,
    pub maker_fully_filled: bool,
}

///
#[derive(Debug, Clone)]
pub enum BookEvent {
    Accepted {
        order_id: OrderId,
    },
    Rejected {
        order_id: OrderId,
        reason: EngineError,
    },
    Trade(Trade),
    Cancelled {
        order_id: OrderId,
    },
    RestingOnBook {
        order_id: OrderId,
        price: Price,
        remaining_qty: Qty,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EngineError {
    #[error("order not found: {0:?}")]
    OrderNotFound(OrderId),
    #[error("limit order missing a price")]
    MissingPrice,
    #[error("zero quantity order")]
    ZeroQty,
    #[error("FOK order could not be fully filled")]
    FokNotFillable,
    #[error("duplicate order id: {0:?}")]
    DuplicateOrderId(OrderId),
    #[error("order crosses with side : {0:?}, and with price :{1:?}")]
    Crosses(Side, Price),
    #[error("invalid tif combination")]
    InvalidTifForMarket,
}
