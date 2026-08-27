use Ordust::OrderType::Market;
use Ordust::engine::{Engine,CANCEL_CALLS};
use Ordust::types::*;
use std::sync::atomic::{AtomicU64, Ordering};

fn make_profile_order(id: u64) -> Order {
    Order {
        id: OrderId(id),
        side: Side::Buy,
        order_type: OrderType::Limit,
        tif: TimeInForce::GTC,
        price: Some(Price(100)),
        qty: Qty(10),
        remaining_qty: Qty(10),
        timestamp: Timestamp(id),
    }
}

#[cfg_attr(feature = "hotpath", hotpath::main)]
fn main() {
    let mut engine = Engine::new();

    for id in 0..100_000 {
        let _ = engine.submit_order(make_profile_order(id));
    }

    for id in 0..100_000 {
        let _ = engine.cancel_order(OrderId(id));
    }

    println!(
        "cancel_order invoked {} times",
        CANCEL_CALLS.load(Ordering::Relaxed)
    );
}
