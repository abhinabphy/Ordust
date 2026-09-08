use std::time::Duration;
use ordust_core::Side::{Buy, Sell};
use tokio::sync::watch;

use ordust_market_data::adapters::hyperliquidadapter::HyperliquidAdapter;
use ordust_market_data::reconciler::Reconciler;
use ordust_market_data::Snapshot;
use ordust_market_data::ws_client::{backoffconfig, RawMessage, WsClient};
use ordust_terminal::log_debug;
use ordust_terminal::tui::{OrderBookView, TerminalUi};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let _ = std::fs::remove_file("debug_engine.log");
    log_debug("=== ENGINE STARTUP ===");

    let backoff = backoffconfig {
        min_backoff: Duration::from_millis(200),
        max_backoff: Duration::from_secs(5),
        multiplier: 2.0,
    };

    let symbol = "BTC";
    let adapter = HyperliquidAdapter::new(symbol);
    let ws_url = "wss://api.hyperliquid.xyz/ws";
    let client = WsClient::new(ws_url, adapter, 10_000, backoff);

    log_debug("Attempting WebSocket connection...");
    let mut rx = client
        .connect_and_spawn()
        .await
        .map_err(|e| -> Box<dyn std::error::Error> {
            log_debug(&format!("FATAL: WebSocket connection failed: {:?}", e));
            Box::new(e)
        })?;

    log_debug("WebSocket connection spawned successfully.");

    let (ui_tx, ui_rx) = watch::channel(OrderBookView {
        symbol: symbol.to_string(),
        ..Default::default()
    });


    tokio::spawn(async move {
        log_debug("Background Reconciliation Task started. Awaiting stream...");
        let mut reconciler = Reconciler::new();
        let mut bids_buf = Vec::with_capacity(20);
        let mut asks_buf = Vec::with_capacity(20);
        let mut msg_counter = 0u64;

        while let Some(msg) = rx.recv().await {
            msg_counter += 1;
            match msg {
                RawMessage::Diff(depth) => {
                    log_debug(&format!(
                        "Msg #{}: Depth received | Bids: {}, Asks: {}, Timestamp ID: {}",
                        msg_counter,
                        depth.bids.len(),
                        depth.asks.len(),
                        depth.final_update_id
                    ));

              
                    let snapshot = Snapshot {
                        last_update_id: depth.final_update_id,
                        bids: depth.bids,
                        asks: depth.asks,
                    };

                    reconciler.on_snapshot(snapshot);

                    let book_synced = reconciler.book().is_some();
                    log_debug(&format!("Reconciler book exists: {}", book_synced));

                    let mut last_update_id = 0;
                    if let Some(book) = reconciler.book() {
                        last_update_id = book.last_update_id();

                        bids_buf.clear();
                        bids_buf.extend(
                            book.depth(Buy, 20)
                                .iter()
                                .map(|(p, q)| (p.0 as f64 / 1e10, q.0 as f64 / 1e10)),
                        );

                        asks_buf.clear();
                        asks_buf.extend(
                            book.depth(Sell, 20)
                                .iter()
                                .map(|(p, q)| (p.0 as f64 / 1e10, q.0 as f64 / 1e10)),
                        );
                    } else {
                        bids_buf.clear();
                        asks_buf.clear();
                    }

                    let view = OrderBookView {
                        symbol: symbol.to_string(),
                        is_synced: book_synced,
                        last_update_id,
                        bids: bids_buf.clone(),
                        asks: asks_buf.clone(),
                    };

                    let send_result = ui_tx.send(view);
                    log_debug(&format!(
                        "ui_tx.send status: {:?}, receiver count: {}",
                        send_result.is_ok(),
                        ui_tx.receiver_count()
                    ));
                }
                RawMessage::Trade(_) => {
                    log_debug("Trade tick received.");
                }
            }
        }
        log_debug("CRITICAL ERROR: rx.recv() loop terminated! WebSocket channel was closed.");
    });

    log_debug("Initializing Terminal UI...");
    let mut ui = TerminalUi::init()?;
    ui.run(ui_rx).await?;

    Ok(())
}