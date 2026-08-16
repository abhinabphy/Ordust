use crate::types::Trade;
use crate::{
    BookEvent, EngineError, Order, OrderBook,
    OrderType::{self, Limit},
    Side::{self, Buy},
};
use crate::{Price, TimeInForce};
pub struct Engine {
    book: OrderBook,
}
impl Engine {
    // public functions
    pub fn new(_book: OrderBook) -> Self {
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

        //for now non unwraps to 0 (needs improvement)
        if self.crosses(order) {
            return Err(EngineError::Crosses(
                order.side,
                order.price.unwrap_or(Price(0)),
            ));
        }
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
