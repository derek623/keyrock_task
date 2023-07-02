use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use crate::market_data_source::*;
use async_trait::async_trait;
use serde::{Deserialize};
use tokio::sync::mpsc::Sender;

#[derive(Debug, Deserialize)]
struct BinanceJson {
    stream: String,
    data: MarketDataSourceData,
}
pub struct Binance {
    info: MarketDataSourceInfo,
    metadata: String,
}

impl Binance {
    pub fn new(address: &str, currency: &str, sender: Sender<OrderBookSnap>, name: &str) -> impl MarketDataSource {
        Binance { info: MarketDataSourceInfo { 
            address: address.to_string(), 
            currency: currency.to_string(), 
            sender,
            name: name.to_string(),
             },
             metadata: format!("@depth{}@100ms", DEFAULT_DEPTH.to_string())
        }
    }
}

#[async_trait]
impl MarketDataSource for Binance {
    fn normalize(&self, msg: &str) -> Result<OrderBookSnap, String>{
        
        let json_msg: BinanceJson = match serde_json::from_str(msg) {
            Ok(msg) => msg,
            Err(e) => { return Err(e.to_string()); }
        };

        let currency = json_msg.stream.trim_end_matches(&self.metadata);
        if currency != self.info.currency {
            return Err("Receive depth for incorrect currency".to_string());
        }
        let mut order_book_snap = OrderBookSnap::new(Exchange::Binance);
        order_book_snap.order_book.bids = json_msg.data.bids;
        order_book_snap.order_book.asks = json_msg.data.asks;
        
        Ok(order_book_snap)
    }

    async fn run(&self) {        
        let final_address = format!("{}{}{}", self.info.address, self.info.currency, self.metadata);

        let url = match url::Url::parse(&final_address) {
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
      
        while let Some(msg) = read.next().await {
             let message = match msg {
                Ok(m) => m,
                Err(e) => {
                    log::error!("{} recv msg err: {:#?}", self.info.name, e);
                    continue;
                }
            };
             match message{
                Message::Text(msg) => {
                    match self.normalize(&msg) {
                    Ok(orderbook) => { 
                        if let Err(msg) = self.info.sender.send(orderbook).await {
                            log::error!("Failed to send orderbook snap: {msg}");
                        }; },
                    Err(e) => { log::error!("Failed to normalize msg for {}: {}", self.info.name, e) },
                    }
                },
                
                Message::Ping(m) => {                 
                    match write.send(Message::Pong(m)).await {
                        Ok(_) => {}
                        Err(e) => { println!("Cannot send pong for {}: {}", self.info.name, e); } 
                    };
                }
                Message::Pong(_) | Message::Frame(_)=> {}
                Message::Binary(_) => (),
                Message::Close(e) => {
                    log::error!("Disconnected {:?}", e);
                    return;
                }
            }
        }
    }

}