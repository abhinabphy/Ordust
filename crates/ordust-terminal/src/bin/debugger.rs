//made by ai for debugging
use std::time::Duration;
use ordust_core::Side::{Buy, Sell};

use ordust_market_data::adapters::hyperliquidadapter::HyperliquidAdapter;
use ordust_market_data::reconciler::Reconciler;
use ordust_market_data::Snapshot;
use ordust_market_data::ws_client::{backoffconfig, RawMessage, WsClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== RECONCILER CLI DEBUGGER ===");
    println!("Connecting to Hyperliquid WebSocket...");

    let backoff = backoffconfig {
        min_backoff: Duration::from_millis(200),
        max_backoff: Duration::from_secs(5),
        multiplier: 2.0,
    };

    let symbol = "BTC";
    let adapter = HyperliquidAdapter::new(symbol);
    let ws_url = "wss://api.hyperliquid.xyz/ws";
    let client = WsClient::new(ws_url, adapter, 10_000, backoff);

    let mut rx = client
        .connect_and_spawn()
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    println!("WebSocket connected. Streaming updates...\n");

    let mut reconciler = Reconciler::new();
    let mut msg_counter = 0u64;

    while let Some(msg) = rx.recv().await {
        match msg {
            RawMessage::Diff(depth) => {
                msg_counter += 1;

                let first_bid = depth.bids.first().map(|(p, q)| (p.0, q.0));
                let first_ask = depth.asks.first().map(|(p, q)| (p.0, q.0));

                println!(
                    "Tick #{:<3} | Bids: {:<2} | Asks: {:<2} | Top Raw Bid: {:?} | Top Raw Ask: {:?}",
                    msg_counter,
                    depth.bids.len(),
                    depth.asks.len(),
                    first_bid,
                    first_ask
                );

                let snapshot = Snapshot {
                    last_update_id: depth.final_update_id,
                    bids: depth.bids,
                    asks: depth.asks,
                };

                // Inspect error returned by on_snapshot
                if let Err(e) = reconciler.on_snapshot(snapshot) {
                    println!("    reconciler.on_snapshot() FAILED: {:?}", e);
                } else {
                    println!("   reconciler.on_snapshot() SUCCESS");
                }

                if let Some(book) = reconciler.book() {
                    let top_bids = book.depth(Buy, 1);
                    let top_asks = book.depth(Sell, 1);
                    println!(
                        "   Book Synced -> Best Bid: ${:.2} | Best Ask: ${:.2}",
                        top_bids.first().map(|(p, _)| p.0 as f64 / 1e10).unwrap_or(0.0),
                        top_asks.first().map(|(p, _)| p.0 as f64 / 1e10).unwrap_or(0.0)
                    );
                } else {
                    println!("   ⚠️ MarketBook is still NONE");
                }
                println!("--------------------------------------------------");
            }
            RawMessage::Trade(_) => {}
        }
    }

    Ok(())
}