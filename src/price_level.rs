use crate::{Order, OrderId, Price, Qty};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct Level {
    pub price: Price,
    pub orders: VecDeque<Order>,
    pub total_qty: Qty, // running sum
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
        self.orders.pop_front()
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
}
