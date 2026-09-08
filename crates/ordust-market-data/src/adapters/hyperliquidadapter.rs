use crate::types::{DepthUpdate, TradeTick, WsError};
use crate::ws_client::{ExchangeAdapter, RawMessage};
use ordust_core::{Price, Qty, Timestamp};
use serde::Deserialize;

pub struct HyperliquidAdapter {
    pub coin: String,
}

impl HyperliquidAdapter {
    pub fn new(coin: impl Into<String>) -> Self {
        Self { coin: coin.into() }
    }
}

// Inbound WebSocket JSON models
#[derive(Deserialize)]
#[serde(tag = "channel")]
enum HyperliquidWsMsg {
    #[serde(rename = "l2Book")]
    L2Book { data: L2BookData },
    #[serde(rename = "trades")]
    Trades { data: Vec<TradeData> },
    #[serde(other)]
    Noise, // Subscription ACKs, heartbeats, or unhandled channels
}

#[derive(Deserialize)]
struct L2BookData {
    time: u64,
    levels: (Vec<LevelData>, Vec<LevelData>), // (bids, asks)
}

#[derive(Deserialize)]
struct LevelData {
    px: String,
    sz: String,
}

#[derive(Deserialize)]
struct TradeData {
    px: String,
    sz: String,
    time: u64,
    #[serde(default)]
    side: String, // "B" (Buy taker) or "A" (Sell taker / Ask)
}

impl ExchangeAdapter for HyperliquidAdapter {
    fn subscription_payload(&self) -> Option<String> {
        Some(format!(
            r#"{{"method":"subscribe","subscription":{{"type":"l2Book","coin":"{}"}}}}"#,
            self.coin
        ))
    }

    fn parse_message(&self, payload: &str) -> Result<Option<RawMessage>, WsError> {
        let msg: HyperliquidWsMsg =
            serde_json::from_str(payload).map_err(|e| WsError::InvalidUrl(e.to_string()))?;

        match msg {
            // Inside parse_message in hyperliquidadapter.rs:

HyperliquidWsMsg::L2Book { data } => {
    let parse_level = |l: LevelData| -> Option<(Price, Qty)> {
        let price = l.px.parse::<f64>().ok()?;
        let qty = l.sz.parse::<f64>().ok()?;
        Some((
            Price((price * 1e10).round() as u64),
            Qty((qty * 1e10).round() as u64),
        ))
    };

    let bids = data.levels.0.into_iter().filter_map(parse_level).collect();
    let asks = data.levels.1.into_iter().filter_map(parse_level).collect();

    // Convert milliseconds -> nanoseconds (ms * 1,000,000)
    let timestamp_ns = data.time.saturating_mul(1_000_000);

    Ok(Some(RawMessage::Diff(DepthUpdate {
        first_update_id: timestamp_ns,
        final_update_id: timestamp_ns,
        bids,
        asks,
    })))
}

            HyperliquidWsMsg::Trades { data } => {
                if let Some(trade) = data.first() {
                    // Parse as f64 first to avoid ParseIntError on decimals
                    let price_f64 = trade
                        .px
                        .parse::<f64>()
                        .map_err(|e| WsError::ParseError(e.to_string()))?;
                    let qty_f64 = trade
                        .sz
                        .parse::<f64>()
                        .map_err(|e| WsError::ParseError(e.to_string()))?;

                    Ok(Some(RawMessage::Trade(TradeTick {
                        timestamp: Timestamp(trade.time),
                        price: Price((price_f64 * 1e10).round() as u64),
                        qty: Qty((qty_f64 * 1e10).round() as u64),
                        is_buyer_maker: trade.side == "A" || trade.side == "SELL",
                    })))
                } else {
                    Ok(None)
                }
            }
            HyperliquidWsMsg::Noise => Ok(None),
        }
    }
}