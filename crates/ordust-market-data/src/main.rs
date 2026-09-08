use std::time::Duration;
use ordust_market_data::types::{DepthUpdate, TradeTick};
use ordust_market_data::{
    backoffconfig, hyperliquidadapter::HyperliquidAdapter, RawMessage, WsClient,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Define reconnection backoff parameters using PascalCase struct name
    let backoff = backoffconfig{
        min_backoff: Duration::from_millis(200),
        max_backoff: Duration::from_secs(5),
        multiplier: 2.0,
    };

    // 2. Instantiate the concrete HyperliquidAdapter (not the ExchangeAdapter trait)
    let adapter =HyperliquidAdapter::new("SOL");
    let ws_url = "wss://api.hyperliquid.xyz/ws";
    let channel_capacity = 10_000; // Bounded mpsc capacity

    // 3. Create client and spawn background WebSocket task
    let client = WsClient::new(ws_url, adapter, channel_capacity, backoff);
    let mut rx = client.connect_and_spawn().await?;

    println!("Hyperliquid client running. Waiting for stream updates...");

    // 4. Consume normalized events from the receiver pipe
    while let Some(msg) = rx.recv().await {
        match msg {
            RawMessage::Diff(depth) => {
                println!(
                    "[L2 Book Update] ts: {} | Bids: {} levels | Asks: {} levels | Top Bid: {:?}",
                    depth.first_update_id,
                    depth.bids.len(),
                    depth.asks.len(),
                    depth.bids.first()
                );
            }
            RawMessage::Trade(trade) => {
                println!(
                    "[Trade Tick] ts: {:?} | Price: {:?} | Qty: {:?} | Buyer Is Maker: {}",
                    trade.timestamp, trade.price, trade.qty, trade.is_buyer_maker
                );
            }
        }
    }

    println!("Stream closed.");
    Ok(())
}