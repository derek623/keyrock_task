use crate::market_data_source::*;
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

const SUCCESSFULLY_CONNECTED: &str = "bts:subscription_succeeded";
const CHANNEL_PREFIX: &str = "order_book_";

#[derive(Debug, Deserialize)]
struct BitstampJson {
    data: MarketDataSourceData,
    channel: String,
}

#[derive(Clone)]
pub struct Bitstamp {
    info: MarketDataSourceInfo,
}

impl Bitstamp {
    pub fn new(
        address: &'static str,
        currency: &str,
        sender: Sender<OrderBookSnap>,
        name: &'static str,
    ) -> Self {
        println!("Create bitstamp instance for {}", currency.to_string());
        Bitstamp {
            info: MarketDataSourceInfo {
                address,
                currency: currency.to_string(),
                sender,
                name,
            },
        }
    }

    pub fn is_successful(&self, msg: &str) -> bool {
        let json_msg: Value = match serde_json::from_str(&msg) {
            Ok(msg) => msg,
            Err(e) => {
                log::error!("Invalid response: {:?} ,{msg}", e);
                return false;
            }
        };

        json_msg["event"] == SUCCESSFULLY_CONNECTED
    }
}

#[async_trait]
impl MarketDataSource for Bitstamp {
    fn normalize(&self, msg: &str) -> Result<OrderBookSnap, String> {
        let json_msg: BitstampJson = match serde_json::from_str(msg) {
            Ok(msg) => msg,
            Err(e) => {
                return Err(e.to_string());
            }
        };

        let currency = json_msg.channel.trim_start_matches(CHANNEL_PREFIX);
        if currency != self.info.currency {
            return Err("Receive depth for incorrect currency".to_string());
        }
        let mut order_book_snap = OrderBookSnap::new(Exchange::Bitstamp);
        order_book_snap.order_book.bids = json_msg.data.bids;
        order_book_snap.order_book.asks = json_msg.data.asks;

        Ok(order_book_snap)
    }

    async fn run(&self) {
        let url = match url::Url::parse(&self.info.address) {
            Ok(u) => u,
            Err(e) => {
                log::error!("Failed to parse address for {}: {:?}", self.info.name, e);
                return;
            }
        };

        let (ws_stream, _response) = match connect_async(url).await {
            Ok((s, r)) => (s, r),
            Err(e) => {
                log::error!("Failed to connect to {}: {:?}", self.info.name, e);
                return;
            }
        };

        let (mut write, mut read) = ws_stream.split();

        let msg = json!({
            "event": "bts:subscribe",
            "data": {
                "channel": format!("order_book_{}", self.info.currency)
            }
        });

        log::info!("Bitstamp sub message: {}", msg.to_string());

        if let Err(e) = write.send(Message::Text(msg.to_string())).await {
            log::error!("Failed to subscribe for {}: {:?}", self.info.name, e);
            return;
        }

        let mut got_first_message = false;
        while let Some(msg) = read.next().await {
            let message = match msg {
                Ok(m) => m,
                Err(e) => {
                    log::error!("{} recv msg err: {:#?}", self.info.name, e);
                    continue;
                }
            };

            match message {
                Message::Text(msg) => {
                    if !got_first_message {
                        if self.is_successful(&msg) {
                            got_first_message = true;
                        } else {
                            log::error!("Fail to subscribe to {}", self.info.name);
                            return;
                        }
                    } else {
                        //let data = msg.unwrap().into_text().unwrap();
                        match self.normalize(&msg) {
                            Ok(orderbook) => {
                                if let Err(msg) = self.info.sender.send(orderbook).await {
                                    log::error!("Failed to send orderbook snap: {msg}");
                                };
                            }
                            Err(e) => {
                                log::error!(
                                    "Failed to normalize msg for {}: {:?}",
                                    self.info.name,
                                    e
                                );
                            }
                        }
                    }
                }
                Message::Ping(m) => {
                    match write.send(Message::Pong(m)).await {
                        Ok(_) => {}
                        Err(e) => {
                            println!("Cannot send pong for {}: {}", self.info.name, e);
                        }
                    };
                }
                Message::Pong(_) | Message::Frame(_) => {}
                Message::Binary(_) => (),
                Message::Close(e) => {
                    log::error!("Disconnected {:?}", e);
                    return;
                }
            }
        }
    }
}
