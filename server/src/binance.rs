use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use crate::market_data_source::{MarketDataSourceInfo, MarketDataSource, OrderBookSnap};
use crate::orderbook::Level;
use async_trait::async_trait;
use serde::{Deserialize};
use tokio::sync::mpsc::Sender;
use crate::utility::de_f64_or_string_as_f64;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct BinanceLevel {
    #[serde(deserialize_with  = "de_f64_or_string_as_f64")]
    price: f64,
    #[serde(deserialize_with  = "de_f64_or_string_as_f64")]
    amount: f64,
}

#[derive(Debug, Deserialize)]
struct BinanceData {
    bids: Vec<BinanceLevel>,
    asks: Vec<BinanceLevel>,
}

#[derive(Debug, Deserialize)]
struct BinanceJson {
    stream: String,
    data: BinanceData,
}
pub struct Binance {
    info: MarketDataSourceInfo,
    metadata: String,
}

impl Binance {
    pub fn new(address: &str, currency: &str, depth: usize, sender: Sender<OrderBookSnap>, name: &str) -> impl MarketDataSource {
        Binance { info: MarketDataSourceInfo { 
            address: address.to_string(), 
            currency: currency.to_string(), 
            depth, 
            sender,
            name: name.to_string(),
             },
             metadata: format!("@depth{}@100ms", depth.to_string())
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
        let mut order_book_snap = OrderBookSnap::new(self.info.name.to_string(), self.info.depth, currency);

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
        let currency_vec: Vec<&str> = self.info.currency.split(',').collect();
        //If not currency is specified, do not start the binance connection as it needs at least one currency in the url
        if currency_vec.len() <= 0 {
            return;
        }        
        let final_address = format!("{}{}{}", self.info.address, currency_vec[0], self.metadata);

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
        //Subscribe to additional currency
        for n in 1..currency_vec.len() {
            let msg = json!({
                "method": "SUBSCRIBE",
                "params": [
                    format!("{}@depth10@100ms", currency_vec[n])
                ],
                "id": 1
            });

            log::info!("Binance sub message: {}", msg.to_string());
            
            if let Err(e) = write.send(Message::Text(msg.to_string())).await {
                log::error!("Failed to subscribe for {}: {:?}", self.info.name, e);
                return;
            }
        }
      
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
                
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_)=> {},
                Message::Binary(_) => (),
                Message::Close(e) => {
                    log::error!("Disconnected {:?}", e);
                    return;
                }
            }
        }
    }

}