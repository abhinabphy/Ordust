

# Ordust 🦀⚡

**Ordust** is an ultra-low latency, deterministic order matching engine implemented in safe Rust. Designed for high-frequency trading (HFT) and microstructural execution research, it leverages a custom `Slab`-allocated intrusive doubly-linked list arena to achieve deterministic $O(1)$ order unlinking and sub-100 nanosecond cancellations.

---

### Key Features

* **Price-Time Priority (FIFO):** Guarantees strict queue priority within each price level.
* **$O(1)$ Arena-Based Cancellations:** Orders live in a pre-allocated `Slab<OrderNode>` doubly-linked list, allowing instant pointer unlinking without full array shifts or tree traversals.
* **Flexible Order Types:** Full support for `Limit` and `Market` orders.
* **Time In Force (TIF) Policies:** Built-in enforcement for `GTC` (Good 'Til Cancelled), `IOC` (Immediate Or Cancel), and `FOK` (Fill Or Kill).
* **Comprehensive Event Stream:** Yields strongly-typed `BookEvent` records (`Accepted`, `Trade`, `RestingOnBook`, `Cancelled`) for real-time downstream processing.
* **Zero Unsafe Code:** Built entirely with safe Rust abstraction boundaries.

---

### Performance & Benchmarks

Measured using [`hotpath`](https://www.google.com/search?q=https://crates.io/crates/hotpath) profiling over a continuous 200,000 operation workload (100,000 order submissions + 100,000 cancellations):

| Operation | Latency (Avg) | Latency (P95) | Throughput | Complexity |
| --- | --- | --- | --- | --- |
| **`submit_order`** | **140 ns** | 291 ns | ~7.14M ops/sec | $O(\log P + M)$ |
| **`cancel_order`** | **65 ns** | 125 ns | ~15.38M ops/sec | $O(1)$ |

*Note: $P$ = distinct price levels, $M$ = matched executions.*

---

### Architecture Quick Look

```text
               +-----------------------------------+
               |        Engine / OrderBook         |
               +-----------------------------------+
                 /                               \
     bids: BTreeMap<Price, Level>     asks: BTreeMap<Price, Level>
                |                                  |
                +-----------------+----------------+
                                  |
                   Arena: Slab<OrderNode>
           [ Head <-> OrderNode <-> OrderNode <-> Tail ]

```
