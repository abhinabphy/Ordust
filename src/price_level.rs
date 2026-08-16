use crate::{Order, OrderId, Price, Qty};
use std::cmp::min;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct Level {
    price: Price,
    orders: VecDeque<Order>,
    total_qty: Qty, // running sum
}

impl Level {
    pub fn new(price: Price) -> Self {
        Level {
            price: price,
            orders: VecDeque::new(),
            total_qty: Qty(0),
        }
    }
    pub fn push_back(self: &mut Self, order: Order) {
        self.total_qty.0 += order.remaining_qty.0;
        self.orders.push_back(order);
    }
    pub fn front(&self) -> Option<&Order> {
        self.orders.front()
    }

    pub fn front_mut(&mut self) -> Option<&mut Order> {
        self.orders.front_mut()
    }
    /// Pop the front order once fully filled.
    pub fn pop_front(&mut self) -> Option<Order> {
        let o = self.orders.pop_front();
        self.total_qty.0 -= o.as_ref().unwrap().remaining_qty.0;
        o
    }

    /// O(n) scan-and-remove by id — this is the thing phase 2 will optimize.
    pub fn remove(&mut self, id: OrderId) -> Option<Order> {
        let mut index = usize::MAX;
        for (i, _ord) in self.orders.iter().enumerate() {
            if (_ord.id == id) {
                index = i;
            }
        }
        self.orders.remove(index)
    }

    pub fn total_qty(&self) -> Qty {
        self.total_qty
    }
    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    pub fn get_order_by_ID(&self, id: OrderId) -> Option<&Order> {
        let order = self.orders.iter().find(|&x| x.id == id);
        order
    }
    /// Consume up to `qty` from the front order. Returns (order_id, filled_qty, fully_filled)
    pub fn consume_qty(&mut self, qty: Qty) -> Option<(OrderId, Qty, bool)> {
        let id: OrderId;
        let filledstatus: bool;
        let fill_amount: Qty;

        {
            let front_ord = self.front_mut()?;
            fill_amount = Qty(min(front_ord.remaining_qty.0, qty.0));
            front_ord.remaining_qty.0 -= fill_amount.0;
            (id, filledstatus) = (front_ord.id, front_ord.remaining_qty.0 == 0);
        }
        self.total_qty.0 -= fill_amount.0;
        if filledstatus {
            self.orders.pop_front();
        }
        Some((id, fill_amount, filledstatus))
    }
}
