# Ordust
//A common mistake when benchmarking in Rust is creating Order objects inside the benchmark loop—this measures heap allocation time rather than matching engine speed.


## Performance & Profiling

Ordust is designed for ultra-low latency execution using a `Slab`-allocated doubly-linked list arena for price levels combined with $O(1)$ order index lookups. Profiling is conducted using [`hotpath`](https://crates.io/crates/hotpath) for real-time timing and memory allocation tracking across 200,000 operations (100,000 submissions + 100,000 cancellations).

---

### Key Metrics Summary

| Operation | Avg Latency | P95 Latency | Throughput | Allocations / Call |
| :--- | :--- | :--- | :--- | :--- |
| **Order Submission** (`submit_order`) | **140 ns** | 291 ns | ~7.14M ops/sec | 508 B |
| **Order Cancellation** (`cancel_order`) | **65 ns** | 125 ns | ~15.38M ops/sec | 48 B |

---

### Hotpath Execution Breakdown

#### 1. Latency Profile

> **Benchmark Workload:** 100,000 `GTC` Limit Orders submitted sequentially, followed by 100,000 instant cancellations by `OrderId`.

| Function | Calls | Avg Time | P95 Time | Total Time | % Total CPU |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `profile_engine::main` | 1 | 32.74 ms | 32.75 ms | 32.74 ms | 100.00% |
| `engine::submit_order` | 100,000 | **140 ns** | **291 ns** | 14.01 ms | 42.80% |
| `engine::cancel_order` | 100,000 | **65 ns** | **125 ns** | 6.56 ms | 20.04% |

#### 2. Allocation & Memory Footprint

> Total Allocated Memory: **53.1 MB** | Total Deallocated Memory: **53.1 MB** | Peak RSS: **73.2 MB**

| Function | Calls | Avg Alloc | P95 Alloc | Total Alloc | % Total Alloc |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `engine::submit_order` | 100,000 | 508 B | 240 B | 48.5 MB | 91.22% |
| `engine::cancel_order` | 100,000 | 48 B | 48 B | 4.6 MB | 8.62% |
| `profile_engine::main` | 1 | 87.0 KB | 87.1 KB | 87.0 KB | 0.16% |

---

### Performance Highlights & Analysis

* **Sub-100 ns Unlinking:** `cancel_order` executes in **65 ns** by performing $O(1)$ node removal directly inside the pre-allocated `Slab` arena, bypassing $O(\log N)$ tree lookups.
* **Allocation Bottleneck Identified:** The current allocation footprint (48 B for cancels, 508 B for submissions) is driven entirely by instantiating `Vec<BookEvent>` per function call. Zero-allocation paths can be unlocked by returning `SmallVec` or using an external event buffer.

---

### How to Reproduce

Run the profiling binary with release optimizations and `hotpath` instrumentation enabled:

```bash
# Clean previous build artifacts
cargo clean

# Run hotpath profile benchmark
cargo run --release --bin profile_engine --features hotpath,hotpath-alloc