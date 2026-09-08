use crate::types::{DepthUpdate, TradeTick, WsError};

use futures_util::{SinkExt, StreamExt};

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

pub struct backoffconfig {
    pub min_backoff: Duration,
    pub max_backoff: Duration,
    pub multiplier: f64,
}

impl Default for backoffconfig {
    fn default() -> Self {
        Self {
            min_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            multiplier: 2.0,
        }
    }
}
pub struct WsClient<A: ExchangeAdapter> {
    url: String,
    adapter: A,
    channel_capacity: usize,
    backoff: backoffconfig,
}

#[derive(Debug, Clone)]
pub enum RawMessage {
    Diff(DepthUpdate),
    Trade(TradeTick),
}

pub trait ExchangeAdapter: Send + Sync + 'static {
    fn subscription_payload(&self) -> Option<String>;

    fn parse_message(&self, payload: &str) -> Result<Option<RawMessage>, WsError>;
}

impl<A: ExchangeAdapter> WsClient<A> {
    pub fn new(
        url: impl Into<String>,
        adapter: A,
        channel_capacity: usize,
        backoff: backoffconfig,
    ) -> Self {
        Self {
            url: url.into(),
            adapter,
            channel_capacity,
            backoff,
        }
    }

    pub async fn connect_and_spawn(self) -> Result<mpsc::Receiver<RawMessage>, WsError> {
        let (tx, rx) = mpsc::channel(self.channel_capacity);
        //let url = Url::parse(&self.url).map_err(|_e| WsError::InvalidUrl(self.url))?;

        tokio::spawn(async move {
            let mut current_backoff = self.backoff.min_backoff;
            'reconnect: loop {
                eprintln!("[WsClient] Connecting to {}..", &self.url.clone());
                //atemping to websocket handshake
                let ws_stream = match connect_async(&self.url.clone()).await {
                    Ok((stream, _)) => stream,
                    Err(e) => {
                        eprintln!("[WsClient] Connection Failed :{} .Retrying...", e);
                        sleep(current_backoff).await;
                        current_backoff = calculate_next_backoff(current_backoff, &self.backoff);
                        continue 'reconnect;
                    }
                };

                let (mut write, mut read) = ws_stream.split();
                //transmit payload
                if let Some(payload) = self.adapter.subscription_payload() {
                    if let Err(e) = write.send(Message::Text(payload.into())).await {
                        eprintln!("[WsClient] Subscription failed : {}.Retrying ..", e);
                        sleep(current_backoff).await;
                        current_backoff = calculate_next_backoff(current_backoff, &self.backoff);
                        continue 'reconnect;
                    }
                }

                current_backoff = self.backoff.min_backoff;
                eprintln!("[WsClient] connected successfully.");

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => match self.adapter.parse_message(&text) {
                            Ok(Some(raw_msg)) => {
                                if tx.send(raw_msg).await.is_err() {
                                    return;
                                }
                            }
                            Ok(None) => {}
                            Err(err) => {
                                eprintln!("[WsClient] parsing error: {:?}", err);
                            }
                        },
                        Ok(Message::Ping(_)) => {}
                        Ok(Message::Close(_)) => {
                            eprintln!("connection closed ");
                            break;
                        }
                        Err(e) => {
                            eprintln!("[WsClient] ERROR (Transport ig ){:?}", e);
                            break;
                        }
                        _ => {}
                    }
                }

                sleep(current_backoff).await;
                current_backoff = calculate_next_backoff(current_backoff, &self.backoff);
            }
        });
        Ok(rx)
    }
}

/// Computes capped exponential backoff
fn calculate_next_backoff(current: Duration, config: &backoffconfig) -> Duration {
    let next_secs = current.as_secs_f64() * config.multiplier;
    let next_duration = Duration::from_secs_f64(next_secs);
    next_duration.min(config.max_backoff)
}
