use crate::{Order, OrderId, Price, Qty};
use slab::Slab;
use std::cmp::min;
use std::collections::VecDeque;

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

    pub fn get_order_by_ID<'a>(&self, id: NodeId, arena: &'a Slab<OrderNode>) -> Option<&'a Order> {
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
