## Testing checklist for phase 1 (do this before calling it done)
 
- [x] Limit order resting, no cross → appears in `depth()`
- [ ] Limit order that fully crosses → `Trade` emitted, nothing rests
- [x] Limit order that partially crosses → `Trade` + remainder rests at its own price
- [x] Market order with insufficient opposite liquidity → fills what's available, remainder **discarded** (market orders never rest)
- [x] IOC: partial fill executes, remainder discarded, nothing left in book
- [ ] FOK: insufficient liquidity → **zero** state change, `Err` returned
- [x] FOK: sufficient liquidity → fills completely, behaves like GTC that happened to fully fill
- [x] Cancel a resting order → removed from book, `order_index` cleaned up, `Level.total_qty` invariant still holds
- [x] Cancel a non-existent id → `Err(OrderNotFound)`, no panic
- [x] Price-time priority: two orders at the same price, first one in gets filled first
- [x] `Level.total_qty` invariant holds after every operation ( this as a reusable test helper early)



Phase-1.5
- [ ] Multi-push + sequential pop_front returns orders in the right order (the one that would've caught the next-pointer bug immediately)
- [ ] Cancel a middle order (not head, not tail) — this exercises the splice-both-neighbors path we haven't actually walked through explicitly yet in this conversation
- [ ] Cancel the tail specifically — makes sure self.tail gets rewound to prev, not just self.head handling
- [ ] All the Limit/Market × GTC/IOC/FOK combinations from before, now running through Slab<OrderNode> instead of VecDeque
- [ ] Re-run the benchmark (release + criterion) — this is the actual payoff check for all this work; confirm cancel cost stops scaling with level depth