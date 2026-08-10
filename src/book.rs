use crate::Level;
use crate::Side::{Buy,Sell};
use crate::types::{Order, OrderId, Price, Side,OrderType,TimeInForce,Qty};
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::ops::Sub;
#[derive(Debug, Clone)]
pub struct OrderBook {
    bids: BTreeMap<Reverse<Price>, Level>, // highest price first
    asks: BTreeMap<Price, Level>,          // lowest price first
    order_index: HashMap<OrderId, (Side, Price)>, // O(1): which level is order X on?
}

impl OrderBook {
    pub fn new() -> Self {
        OrderBook {
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

        if bid > ask {
            return Some(bid.sub(ask));
        }
        None
    }

    /// Insert a resting order. Caller (engine) guarantees it doesn't cross.
    pub fn insert_resting(&mut self, order: Order) {
        let p = order.price.unwrap();
        let qty=order.remaining_qty;
        if order.side == Sell {
                let mut level = self.asks.entry(p).or_insert_with(|| Level::new(p));
                level.push_back(order.clone());
                level.total_qty+=qty;

            }
         else {
            let mut level = self.bids.entry(Reverse(p)).or_insert_with(|| Level::new(p));
                level.push_back(order.clone());
                level.total_qty+=qty;
        }
        //update order_index
        self.order_index.insert(order.id, (order.side,order.price.unwrap()));
    }

    /// Remove and return an order by id. Updates order_index and Level's total_qty.
    pub fn remove_order(&mut self, id: OrderId) -> Option<Order>{
        let  (side,price)=self.order_index[&id];
        if side==Buy{
           let mut level=self.bids.get_mut(&Reverse(price)).unwrap();
           level.remove(id)
        }
        else{
           let mut level=self.asks.get_mut(&price).unwrap();
           level.remove(id)
        }
    }


}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Side::{Buy, Sell}, Timestamp};

    #[test]
    fn insert_resting_adds_buy_order_to_the_bid_level() {
        let mut book = OrderBook::new();
        let mut order = Order {
            id: OrderId(1),
            side: Buy,
            order_type: OrderType::Limit,
            tif: TimeInForce::GTC,
            price: Some(Price(100)),
            qty: Qty(10),
            remaining_qty: Qty(10),
            timestamp: Timestamp(1),
        };

        book.insert_resting(order.clone());
        assert_eq!(book.best_bid(), Some(Price(100)));

        assert_eq!(book.best_ask(), None);
        assert!(book.bids.contains_key(&Reverse(Price(100))));
        assert!(!book.asks.contains_key(&Price(100)));
        order.side=Sell;
        order.price=Some(Price(90));
        book.insert_resting(order.clone());

        assert_eq!(book.spread(),Some(Price(10)));
    }
}
