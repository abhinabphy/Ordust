use crate::NodeId;
use crate::Side::{Buy, Sell};
use crate::types::{Order, OrderId, OrderType, Price, Qty, Side, TimeInForce};
use crate::{
    FillResult,
    price_level::{Level, OrderNode},
};
use slab::Slab;
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::f64::consts::E;
use std::ops::Sub;
#[derive(Debug, Clone)]
pub struct OrderBook {
    arena: Slab<OrderNode>,
    bids: BTreeMap<Reverse<Price>, Level>, // highest price first
    asks: BTreeMap<Price, Level>,          // lowest price first
    order_index: HashMap<OrderId, NodeId>, // O(1): which level is order X on?
}

impl OrderBook {
    pub fn new() -> Self {
        OrderBook {
            arena: Slab::with_capacity(1000),
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_index: HashMap::new(),
        }
    }

    pub fn best_bid(&self) -> Option<Price> {
        if let Some((best_bid, _)) = self.bids.first_key_value() {
            return Some(best_bid.0);
        }
        None
    }
    pub fn best_ask(&self) -> Option<Price> {
        if let Some((best_ask, _)) = self.asks.first_key_value() {
            return Some(*best_ask);
        }
        None
    }

    pub fn spread(&self) -> Option<Price> {
        let bid = self.best_bid()?;
        let ask = self.best_ask()?;

        if ask > bid {
            return Some(ask.sub(bid));
        }
        None
    }

    /// Insert a resting order. Caller (engine) guarantees it doesn't cross.
    pub fn insert_resting(&mut self, order: Order) {
        let p = order.price.expect("expected a price for order");
        let side = order.side;
        let id = order.id;
        let nodeid: NodeId;

        if order.side == Sell {
            let level = self.asks.entry(p).or_insert_with(|| Level::new(p));
            nodeid = level.push_back(&mut self.arena, order);
        } else {
            let level = self.bids.entry(Reverse(p)).or_insert_with(|| Level::new(p));
            nodeid = level.push_back(&mut self.arena, order);
        }
        //update order_index
        self.order_index.insert(id, nodeid);
    }

    /// Remove and return an order by id. Updates order_index and Level's total_qty.
    pub fn remove_order(&mut self, id: OrderId) -> Option<Order> {
        let removed_order: Option<Order>;
        let nodeid = self.order_index.get(&id)?;
        let side = self.arena.get(nodeid.0)?.order.side;
        //no need to remove a orderid with market type order
        let price = self.arena.get(nodeid.0)?.order.price?;
        if side == Buy {
            let level = self.bids.get_mut(&Reverse(price))?;
            removed_order = level.remove(&mut self.arena, *nodeid);
            if level.is_empty() {
                self.bids.remove(&Reverse(price));
            }
        } else {
            let level = self.asks.get_mut(&price).unwrap();
            removed_order = level.remove(&mut self.arena, *nodeid);
            if level.is_empty() {
                self.asks.remove(&price);
            }
        }

        self.order_index.remove_entry(&id)?;
        removed_order
    }

    pub fn get_order(&self, id: OrderId) -> Option<&Order> {
        if let Some(nodeid) = self.order_index.get(&id) {
            let ordernode = self.arena.get(nodeid.0)?;
            return Some(&ordernode.order);
            // if *side == Buy {
            //     let level = self.bids.get(&Reverse(*price)).unwrap();
            //     let order = level.get_order_by_ID(*nodeid,&self.arena);
            //     return order;
            // } else {
            //     let level = self.asks.get(&price).unwrap();
            //     let order = level.get_order_by_ID(*nodeid,&self.arena);
            //     return order;
            // }
        }
        None
    }

    /// Top-of-book access for matching: immutable ref to the best level on a side.
    pub fn best_level(&mut self, side: Side) -> Option<&Level> {
        if side == Buy {
            return self.bids.iter().next().map(|(_, level)| level);
        } else {
            return self.asks.iter().next().map(|(_, level)| level);
        }
    }
    ///droping a level once its empty as empty level
    pub fn prune_empty(&mut self, side: Side, price: Price) {
        match side {
            Buy => {
                let level = self.bids.get(&Reverse(price));
                if !level.is_none() && level.unwrap().is_empty() {
                    //then remove this level from bid side
                    self.bids.remove(&Reverse(price));
                }
            }
            Sell => {
                let level = self.asks.get(&price);
                if !level.is_none() && level.unwrap().is_empty() {
                    //then remove this level from ask side
                    self.asks.remove(&price);
                }
            }
        }
    }
    pub fn match_at_best(&mut self, opposite_side: Side, qty: Qty) -> Option<FillResult> {
        match opposite_side {
            Side::Buy => {
                let (&Reverse(price), level) = self.bids.iter_mut().next()?;
                let (maker_id, filled_amount, filledstatus) =
                    level.consume_qty(qty, &mut self.arena)?;
                if filledstatus {
                    self.order_index.remove(&maker_id);
                }
                if level.is_empty() {
                    self.bids.remove(&Reverse(price));
                }
                Some(FillResult {
                    maker_order_id: maker_id,
                    price,
                    filled_qty: filled_amount,
                    maker_fully_filled: filledstatus,
                })
            }
            Side::Sell => {
                let (&price, level) = self.asks.iter_mut().next()?;
                let (maker_id, filled_amount, filledstatus) =
                    level.consume_qty(qty, &mut self.arena)?;
                if filledstatus {
                    self.order_index.remove(&maker_id);
                }
                if level.is_empty() {
                    self.asks.remove(&price);
                }
                Some(FillResult {
                    maker_order_id: maker_id,
                    price,
                    filled_qty: filled_amount,
                    maker_fully_filled: filledstatus,
                })
            }
        }
    }
    //match against opposite side
    fn opposite_side(side: Side) -> Side {
        if side == Buy {
            return Side::Sell;
        } else {
            return Side::Buy;
        }
    }
    // Snapshot for display/testing — N levels deep, (price, aggregate qty)
    pub fn depth(&self, side: Side, n: usize) -> Vec<(Price, Qty)> {
        match side {
            Buy => self
                .bids
                .iter()
                .take(n)
                .map(|(Reverse(price), level)| (*price, level.total_qty()))
                .collect(),
            Sell => self
                .asks
                .iter()
                .take(n)
                .map(|(price, level)| (*price, level.total_qty()))
                .collect(),
        }
    }

    // Lazy, borrowed traversal of one side, best price first. No allocation.
    pub fn iter_side(&self, side: Side) -> Box<dyn Iterator<Item = (Price, &Level)> + '_> {
        match side {
            Side::Buy => Box::new(self.bids.iter().map(|(&Reverse(p), l)| (p, l))),
            Side::Sell => Box::new(self.asks.iter().map(|(&p, l)| (p, l))),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        OrderType::Limit,
        Side::{Buy, Sell},
        Timestamp,
    };

    pub(crate) fn make_order(
        id: u64,
        side: Side,
        price: u64,
        qty: u64,
        timestamp: u64,
        ordtype: OrderType,
        tiftype: TimeInForce,
    ) -> Order {
        Order {
            id: OrderId(id),
            side,
            order_type: ordtype,
            tif: tiftype,
            price: Some(Price(price)),
            qty: Qty(qty),
            remaining_qty: Qty(qty),
            timestamp: Timestamp(timestamp),
        }
    }

    #[test]
    fn insert_resting_adds_buy_order_to_the_bid_level() {
        let mut book = OrderBook::new();

        // 1. Buy @ 100
        let order = make_order(1, Buy, 100, 10, 1, Limit, TimeInForce::GTC);

        book.insert_resting(order);

        assert_eq!(book.best_bid(), Some(Price(100)));
        assert_eq!(book.best_ask(), None);

        assert!(book.bids.contains_key(&Reverse(Price(100))));
        assert!(!book.asks.contains_key(&Price(100)));

        // 3. Sell @ 120
        let order = make_order(3, Sell, 120, 10, 1, Limit, TimeInForce::GTC);

        book.insert_resting(order);
        assert_eq!(book.order_index.contains_key(&OrderId(3)), true);

        // Remove Sell @120
        book.remove_order(OrderId(3));

        assert_eq!(book.order_index.contains_key(&OrderId(3)), false);

        // 4. Sell @ 130
        let order = make_order(4, Sell, 130, 10, 1, Limit, TimeInForce::GTC);

        book.insert_resting(order.clone());

        println!("{:#?}", book.depth(Buy, usize::MAX));
    }
}
