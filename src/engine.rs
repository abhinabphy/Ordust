use crate::types::{Trade,Qty,OrderId};
use crate::{
    BookEvent, EngineError, Order, OrderBook,
    OrderType::{self, Limit},
    Side::{self, Buy,Sell},
};
use crate::{Price, TimeInForce};
pub struct Engine {
    book: OrderBook,
}
impl Engine {
    // public functions
    pub fn new() -> Self {
        return Engine {
            book: OrderBook::new(),
        };
    }
    /// Single entry point for all order submission.
    /// Validates → matches against opposite side → applies TIF policy to any remainder.
    pub fn submit_order(&mut self, mut order: Order) -> Result<Vec<BookEvent>, EngineError> {
        self.validate(&order)?;
        let mut events = vec![];
        events.push(BookEvent::Accepted { order_id: order.id });
        //Quick FOK check
        if order.tif == TimeInForce::FOK && !self.can_fully_fill(&order) {
            return Err(EngineError::FokNotFillable);
        }
        if self.crosses(&order) {
            let trades = self.match_against_book(&mut order);
            events.extend(trades.into_iter().map(BookEvent::Trade));
        }

        events.extend(self.apply_tif_to_remainder(order));

        Ok(events)
    }
    ///cancels order , returns bookevent else returns EngineError
     pub fn cancel_order(&mut self,id:OrderId) -> Result<Vec<BookEvent>, EngineError>{
       match self.book.remove_order(id) {
        Some(_order) => Ok(vec![BookEvent::Cancelled { order_id: id }]),
        None => Err(EngineError::OrderNotFound(id)),
    }
     }



    //Private functions
    fn validate(&self, order: &Order) -> Result<(), EngineError> {
        if order.remaining_qty.0 == 0 {
            return Err(EngineError::ZeroQty);
        };
        match order.order_type {
            OrderType::Limit => {
                if order.price.is_none() {
                    return Err(EngineError::MissingPrice);
                }
            }
            OrderType::Market => {
                if order.tif == TimeInForce::GTC {
                    return Err(EngineError::InvalidTifForMarket);
                }
            }
        }

        // //for now non unwraps to 0 (needs improvement)
        // if self.crosses(order) {
        //     return Err(EngineError::Crosses(
        //         order.side,
        //         order.price.unwrap_or(Price(0)),
        //     ));
        // }
        if let Some(_ord) = self.book.get_order(order.id) {
            return Err(EngineError::DuplicateOrderId(_ord.id));
        }
        Ok(())
    }
    //match against opposite side
    fn opposite_side(side: Side) -> Side {
        if side == Buy {
            return Side::Sell;
        } else {
            return Side::Buy;
        }
    }
    // Market always crosses (if opposite side non-empty).
    // Limit Buy crosses if order.price >= best_ask.
    // Limit Sell crosses if order.price <= best_bid.

    fn crosses(&self, order: &Order) -> bool {
        match order.order_type {
            // Market has no price to compare — it crosses as long as ANY opposite liquidity exists
            OrderType::Market => match order.side {
                Side::Buy => self.book.best_ask().is_some(),
                Side::Sell => self.book.best_bid().is_some(),
            },
            // Limit only crosses if its price is aggressive enough vs the current best
            OrderType::Limit => match order.side {
                Side::Buy => self
                    .book
                    .best_ask()
                    .map_or(false, |ask| order.price.unwrap() >= ask),
                Side::Sell => self
                    .book
                    .best_bid()
                    .map_or(false, |bid| order.price.unwrap() <= bid),
            },
        }
    }
    // Loop: while order.remaining_qty > 0 and crosses():
    //   take front() of best opposite level, trade min(remaining, resting.remaining)
    //   decrement both sides' remaining_qty, emit Trade
    //   if resting order fully filled -> pop_front it, prune_empty if level now empty

    fn match_against_book(&mut self, taker_order: &mut Order) -> Vec<Trade> {
        let mut trades = Vec::new();
        let opposite_side = Self::opposite_side(taker_order.side);
        while taker_order.remaining_qty.0 > 0 && self.crosses(taker_order) {
            let Some(fill) = self
                .book
                .match_at_best(opposite_side, taker_order.remaining_qty)
            else {
                break;
            };

            taker_order.remaining_qty.0 -= fill.filled_qty.0;
            trades.push(Trade {
                maker_order_id: fill.maker_order_id,
                taker_order_id: taker_order.id,
                price: fill.price,
                qty: fill.filled_qty,
                timestamp: taker_order.timestamp,
            })
        }
        trades
    }

    // GTC: if remaining_qty > 0, insert_resting
    // IOC: if remaining_qty > 0, DISCARD it (no book insert), emit nothing extra
    // FOK: special — see note below
    fn apply_tif_to_remainder(&mut self, order: Order) -> Vec<BookEvent> {
        if order.remaining_qty.0 == 0 {
            return vec![];
        }
        match order.tif {
            TimeInForce::GTC => {
                let (side, price) = (order.side, order.price.unwrap());
                let remaining = order.remaining_qty;
                let id = order.id;
                self.book.insert_resting(order);
                vec![BookEvent::RestingOnBook {
                    order_id: id,
                    price,
                    remaining_qty: remaining,
                }]
            }
            TimeInForce::IOC => {
                vec![BookEvent::Cancelled { order_id: order.id }]
            }
            TimeInForce::FOK => {
                vec![] //this in unreachable code with remianing>0
                //can_fully_fill() already guarantees a full full before !!!
            }
        }
    }

    fn can_fully_fill(&self, order: &Order) -> bool {
        let opposite = Self::opposite_side(order.side);
        let mut accumulated = 0u64;

        for (price, level) in self.book.iter_side(opposite) {
            if let Some(limit) = order.price {
                let still_crosses = match order.side {
                    Side::Buy => price.0 <= limit.0,
                    Side::Sell => price.0 >= limit.0,
                };
                if !still_crosses {
                    break;
                }
            }
            accumulated += level.total_qty().0;
            if accumulated >= order.remaining_qty.0 {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OrderType::Market, TimeInForce::GTC, book::tests::make_order};
    #[test]
    fn can_use_make_order_from_book_tests() {
        let order = make_order(1, Buy, 100, 10, 1,Limit,GTC);

        assert_eq!(order.id, OrderId(1));
        assert_eq!(order.side, Buy);
        assert_eq!(order.price, Some(Price(100)));
        assert_eq!(order.qty, Qty(10));
    }
  /// Limit order resting, no cross → appears in `depth()`
    #[test]
    fn test_limit_resting_order_no_cross() -> Result<(), EngineError> {
        let mut idcount=0;
        let mut engine=Engine::new();
        let order = make_order(idcount, Buy, 100, 10, 1,Limit,GTC);
        let mut results=engine.submit_order(order)?;
        // dbg!(results);
        assert_eq!(engine.book.best_bid(),Some(Price(100)));
        idcount+=1;
        let order=make_order(idcount,Side::Sell,100,8,2,Limit,GTC);
         results.extend(engine.submit_order(order)?);

        dbg!(results);
        dbg!(engine.book);
        Ok(())
    }
    #[test]
    fn test_market_order_insufficient_liquidity() -> Result<(),EngineError> {
         let mut idcount=0;
        let mut engine=Engine::new();
        let order = make_order(idcount, Buy, 100, 10, 1,Limit,GTC);
        let mut results=engine.submit_order(order)?;
        // dbg!(results);
        assert_eq!(engine.book.best_bid(),Some(Price(100)));
        idcount+=1;
        let order=make_order(idcount,Side::Sell,100,12,2,Market,TimeInForce::IOC);
         results.extend(engine.submit_order(order)?);

        dbg!(results);

        dbg!(engine.book);

        Ok(())

    }
}


