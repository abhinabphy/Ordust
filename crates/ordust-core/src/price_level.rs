use crate::{Order, OrderId, Price, Qty};
use slab::Slab;
use std::cmp::min;

///mapping for the key to ordernode where the order lives
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);
///this is  where the order lives with prev and next pointers in the list
#[derive(Debug, Clone)]
pub struct OrderNode {
    pub order: Order,
    prev: Option<NodeId>,
    next: Option<NodeId>,
}
///
#[derive(Debug, Clone)]
pub struct Level {
    price: Price,
    head: Option<NodeId>,
    tail: Option<NodeId>,
    total_qty: Qty, // running sum
}

impl Level {
    pub fn new(price: Price) -> Self {
        Level {
            price: price,
            head: None,
            tail: None,
            total_qty: Qty(0),
        }
    }
    pub fn push_back(self: &mut Self, arena: &mut Slab<OrderNode>, order: Order) -> NodeId {
        self.total_qty.0 += order.remaining_qty.0;

        debug_assert_eq!(
            order.price,
            Some(self.price),
            "order price must match this level's price"
        );

        let currordernode = OrderNode {
            order: order,
            prev: self.tail,
            next: None,
        };

        let slab_key = arena.insert(currordernode);
        let currorderid = NodeId(slab_key);

        if let Some(old_tail) = self.tail {
            if let Some(old_tail_node) = arena.get_mut(old_tail.0) {
                old_tail_node.next = Some(currorderid);
            }
        } else {
            // list was empty — new node is also the head
            self.head = Some(currorderid);
        }

        self.tail = Some(currorderid);
        currorderid
    }
    pub fn front<'a>(&self, arena: &'a Slab<OrderNode>) -> Option<&'a Order> {
        let ordernode = arena.get(self.head?.0);
        if let Some(_ordernode) = ordernode {
            return Some(&_ordernode.order);
        }

        None
    }

    // pub fn front_mut(&mut self) -> Option<&mut Order> {
    //     self.orders.front_mut()
    // }

    /// Pop the front order once fully filled. Frees its arena slot.
    pub fn pop_front(&mut self, arena: &mut Slab<OrderNode>) -> Option<Order> {
        let head_id = self.head?;
        let node = arena.try_remove(head_id.0)?;

        self.head = node.next;

        match self.head {
            Some(new_head_id) => {
                if let Some(new_head) = arena.get_mut(new_head_id.0) {
                    new_head.prev = None;
                }
            }
            None => {
                self.tail = None;
            }
        }

        self.total_qty.0 -= node.order.remaining_qty.0;
        Some(node.order)
    }

    /// remove via O(1) operation
    /// if
    /// prev or next pointer are null of removed order , head and tail
    /// are respectively ordernode->next and ordernode->prev
    /// else
    /// prev pointer's next is ordernode's next and nextnode's prev is ordernode's prev
    pub fn remove(&mut self, arena: &mut Slab<OrderNode>, id: NodeId) -> Option<Order> {
        let ordernode = arena.try_remove(id.0)?;
        match ordernode.prev {
            Some(prev_id) => {
                if let Some(prev_node) = arena.get_mut(prev_id.0) {
                    prev_node.next = ordernode.next;
                }
            }
            None => {
                self.head = ordernode.next;
            }
        }

        match ordernode.next {
            Some(next_id) => {
                if let Some(next_node) = arena.get_mut(next_id.0) {
                    next_node.prev = ordernode.prev;
                }
            }
            None => {
                self.tail = ordernode.prev;
            }
        }

        self.total_qty.0 = self
            .total_qty
            .0
            .checked_sub(ordernode.order.remaining_qty.0)
            .expect("total_qty underflow: level accounting is out of sync");

        Some(ordernode.order)
    }

    pub fn total_qty(&self) -> Qty {
        self.total_qty
    }
    pub fn is_empty(&self) -> bool {
        self.total_qty.0 == 0
    }

    pub fn get_order_by_id<'a>(&self, id: NodeId, arena: &'a Slab<OrderNode>) -> Option<&'a Order> {
        let ordernode = arena.get(id.0);
        match ordernode {
            Some(_ordernode) => Some(&_ordernode.order),
            None => None,
        }
    }
    /// Consume up to `qty` from the front order. Returns (order_id, filled_qty, fully_filled)
    pub fn consume_qty(
        &mut self,
        qty: Qty,
        arena: &mut Slab<OrderNode>,
    ) -> Option<(OrderId, Qty, bool)> {
        let id: OrderId;
        let filledstatus: bool;
        let fill_amount: Qty;

        {
            let node = arena.get_mut(self.head?.0)?;
            fill_amount = Qty(min(node.order.remaining_qty.0, qty.0));
            node.order.remaining_qty.0 -= fill_amount.0;
            (id, filledstatus) = (node.order.id, node.order.remaining_qty.0 == 0);
        }
        self.total_qty.0 -= fill_amount.0;
        //now can call pop_front as remaining_qty.0 is already zeroed
        if filledstatus {
            self.pop_front(arena);
        }
        Some((id, fill_amount, filledstatus))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn make_test_order(id: u64, qty: u64) -> Order {
        Order {
            id: OrderId(id),
            side: Side::Buy,
            order_type: OrderType::Limit,
            tif: TimeInForce::GTC,
            price: Some(Price(100)),
            qty: Qty(qty),
            remaining_qty: Qty(qty),
            timestamp: Timestamp(id),
        }
    }

    #[test]
    fn test_remove_middle_order_splices_neighbors() {
        let mut level = Level::new(Price(100));
        let mut arena = Slab::new();

        // 1. Insert 3 orders (Head -> Middle -> Tail)
        let id1 = level.push_back(&mut arena, make_test_order(1, 10));
        let id2 = level.push_back(&mut arena, make_test_order(2, 20)); // Middle
        let id3 = level.push_back(&mut arena, make_test_order(3, 30));

        assert_eq!(level.total_qty(), Qty(60));

        // 2. Remove middle node
        let cancelled = level.remove(&mut arena, id2);

        // --- Assertions ---
        // A. Verify returned order & level metrics
        assert_eq!(cancelled.map(|o| o.id), Some(OrderId(2)));
        assert_eq!(level.total_qty(), Qty(40));

        // B. Verify head and tail remain untouched
        assert_eq!(level.head, Some(id1));
        assert_eq!(level.tail, Some(id3));

        // C. Verify middle node is removed from Slab
        assert!(arena.get(id2.0).is_none());

        // D. Inspect direct pointer updates on neighboring nodes
        let node1 = arena.get(id1.0).expect("head node should exist");
        assert_eq!(node1.prev, None);
        assert_eq!(node1.next, Some(id3)); // Pointer now points directly to node 3

        let node3 = arena.get(id3.0).expect("tail node should exist");
        assert_eq!(node3.prev, Some(id1)); // Pointer now points directly back to node 1
        assert_eq!(node3.next, None);

        // E. Verify matching queue sequence (pop_front yields Node 1 then Node 3)
        let pop1 = level.pop_front(&mut arena);
        assert_eq!(pop1.map(|o| o.id), Some(OrderId(1)));

        let pop2 = level.pop_front(&mut arena);
        assert_eq!(pop2.map(|o| o.id), Some(OrderId(3)));

        assert!(level.is_empty());
    }
}
