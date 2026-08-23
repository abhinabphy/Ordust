## Testing checklist for phase 1 (do this before calling it done)
 
- [x] Limit order resting, no cross → appears in `depth()`
- [ ] Limit order that fully crosses → `Trade` emitted, nothing rests
- [x] Limit order that partially crosses → `Trade` + remainder rests at its own price
- [ ] Market order with insufficient opposite liquidity → fills what's available, remainder **discarded** (market orders never rest)
- [ ] IOC: partial fill executes, remainder discarded, nothing left in book
- [ ] FOK: insufficient liquidity → **zero** state change, `Err` returned
- [ ] FOK: sufficient liquidity → fills completely, behaves like GTC that happened to fully fill
- [ ] Cancel a resting order → removed from book, `order_index` cleaned up, `Level.total_qty` invariant still holds
- [ ] Cancel a non-existent id → `Err(OrderNotFound)`, no panic
- [ ] Price-time priority: two orders at the same price, first one in gets filled first
- [ ] `Level.total_qty` invariant holds after every operation (write this as a reusable test helper early)
