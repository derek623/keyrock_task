use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use async_trait::async_trait;
use crate::marketdatasource::{MarketDataSourceInfo, MarketDataSource, OrderBookSnap};
use crate::orderbook::Level;
use serde_json::{json, Value};
use serde::Deserialize;
use tokio::sync::mpsc::Sender;
use crate::utility::de_f64_or_string_as_f64;

const SUCCESSFULLY_CONNECTED: &str = "bts:subscription_succeeded";

#[derive(Debug, Deserialize)]
struct BitstampLevel {
    #[serde(deserialize_with  = "de_f64_or_string_as_f64")]
    price: f64,
    #[serde(deserialize_with  = "de_f64_or_string_as_f64")]
    amount: f64,
}

#[derive(Debug, Deserialize)]
struct BitstampData {
    timestamp: String,
    microtimestamp: String,
    bids: Vec<BitstampLevel>,
    asks: Vec<BitstampLevel>,
}

#[derive(Debug, Deserialize)]
struct BitstampJson {
    data: BitstampData,
    channel: String,
    event: String,
}

pub struct Bitstamp {
    info: MarketDataSourceInfo,
}

impl Bitstamp {
    pub fn new(address: &str, currency: &str, depth: usize, sender: Sender<OrderBookSnap>, name: &str) -> impl MarketDataSource {
        Bitstamp { info: MarketDataSourceInfo {
                address: address.to_string(), 
                currency: currency.to_string(), 
                depth, 
                sender,
                name: name.to_string(),
            }
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
            Err(e) => { return Err(e.to_string()); }
        };

        let mut order_book_snap = OrderBookSnap::new(self.info.name.to_string(), self.info.depth, &self.info.currency);

        for index in 0..self.info.depth {
            order_book_snap.order_book.add_bid(Level{
                exchange: self.info.name.to_string(), 
                price: json_msg.data.bids[index].price, 
                amount: json_msg.data.bids[index].amount});
            order_book_snap.order_book.add_ask(Level{
                exchange: self.info.name.to_string(), 
                price: json_msg.data.asks[index].price, 
                amount: json_msg.data.asks[index].amount});
        }
        
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
        
        let (ws_stream, _response) = match connect_async(url).await{
            Ok((s,r)) => (s, r),
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
                    }
                    else {
                        //let data = msg.unwrap().into_text().unwrap();
                        match self.normalize(&msg) {
                            Ok(orderbook) => { 
                                if let Err(msg) = self.info.sender.send(orderbook).await {
                                    log::error!("Failed to send orderbook snap: {msg}");
                                }; },
                            Err(e) => { log::error!("Failed to normalize msg for {}: {:?}", self.info.name, e); },
                        }
                    }
                }
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_)=> {}
                Message::Binary(_) => (),
                Message::Close(e) => {
                    log::error!("Disconnected {:?}", e);
                    return;
                }
            }
        }
    }

}