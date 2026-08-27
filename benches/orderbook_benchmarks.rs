use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
// Replace `your_crate_name` with the package name defined in Cargo.toml
use Ordust::{Order, OrderId, OrderType, Price, Qty, Side, TimeInForce, engine};

fn make_test_order(id: u64, side: Side, price: u64, qty: u64) -> Order {
    Order {
        id: OrderId(id),
        side,
        order_type: OrderType::Limit,
        tif: TimeInForce::GTC,
        price: Some(Price(price)),
        qty: Qty(qty),
        remaining_qty: Qty(qty),
        timestamp: Ordust::Timestamp(1),
    }
}

/// Measures total throughput (orders/sec) for inserting 10k resting orders.
fn bench_resting_insertions(c: &mut Criterion) {
    let mut group = c.benchmark_group("Order Insertion");
    let num_orders = 10_000u64;

    group.throughput(Throughput::Elements(num_orders));

    group.bench_function("insert_10k_resting_bids", |b| {
        b.iter_batched(
            || {
                // Setup Phase: engine creation & order allocation happens OUTSIDE timing window
                let _engine = engine::Engine::new();
                let orders: Vec<Order> = (0..num_orders)
                    .map(|id| make_test_order(id, Side::Buy, 100, 10))
                    .collect();
                (_engine, orders)
            },
            |(mut engine, orders)| {
                // Hot Path: Only submit execution is timed
                for order in orders {
                    let _ = engine.submit_order(order);
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Measures execution latency for a aggressive taker crossing a maker limit order.
fn bench_cross_matching_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("Order Matching");
    group.throughput(Throughput::Elements(1));

    group.bench_function("single_order_cross_latency", |b| {
        b.iter_batched(
            || {
                let mut engine = engine::Engine::new();
                // Pre-populate the book with a resting Buy order
                let maker = make_test_order(0, Side::Buy, 100, 10);
                let _ = engine.submit_order(maker);

                // Construct incoming aggressive Sell order
                let taker = make_test_order(1, Side::Sell, 100, 10);
                (engine, taker)
            },
            |(mut engine, taker)| {
                let _ = engine.submit_order(taker);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Measures cancellation lookup and unlinking speed.
fn bench_cancellation_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("Order Cancellation");
    group.throughput(Throughput::Elements(1));

    group.bench_function("cancel_resting_order_O1", |b| {
        b.iter_batched(
            || {
                let mut engine = engine::Engine::new();
                let order = make_test_order(42, Side::Buy, 100, 10);
                let _ = engine.submit_order(order);
                (engine, OrderId(42))
            },
            |(mut engine, order_id)| {
                let _ = engine.cancel_order(order_id);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_resting_insertions,
    bench_cancellation_latency,
    bench_cross_matching_latency,
);
criterion_main!(benches);
