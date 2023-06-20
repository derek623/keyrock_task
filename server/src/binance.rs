use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::StreamExt;
use crate::marketdatasource::{Exchanges, MarketDataSourceInfo, MarketDataSource, OrderBookSnap, Level};
use async_trait::async_trait;
use serde::{Deserialize};
use tokio::sync::mpsc::Sender;
use crate::utility::de_f64_or_string_as_f64;

#[derive(Debug, Deserialize)]
struct BinanceLevel {
    #[serde(deserialize_with  = "de_f64_or_string_as_f64")]
    price: f64,
    #[serde(deserialize_with  = "de_f64_or_string_as_f64")]
    amount: f64,
}

#[derive(Debug, Deserialize)]
struct BinanceJson {
    bids: Vec<BinanceLevel>,
    asks: Vec<BinanceLevel>,
}
pub struct Binance {
    info: MarketDataSourceInfo,
}

impl Binance {
    pub fn new(address: &str, currency: &str, depth: usize, sender: Sender<OrderBookSnap>, name: &str) -> impl MarketDataSource {
        Binance { info: MarketDataSourceInfo { 
            address: address.to_string(), 
            currency: currency.to_string(), 
            depth, 
            sender,
            name: name.to_string() }
        }
    }
}

#[async_trait]
impl MarketDataSource for Binance {
    fn normalize(&self, msg: &str) -> Result<OrderBookSnap, ()>{
        
        let json_msg: BinanceJson = match serde_json::from_str(msg) {
            Ok(msg) => msg,
            Err(e) => { return Err(()); }
        };
        //println!("binance JSON is: {:#?}\n", json_msg);
        //let exchange = "Binance";

        let mut order_book_snap = OrderBookSnap::new(Exchanges::BINANCE, self.info.depth, &self.info.currency);

        for index in 0..self.info.depth {
            order_book_snap.order_book.add_bid(Level{
                exchange: Exchanges::BINANCE, 
                price: json_msg.bids[index].price, 
                amount: json_msg.bids[index].amount});
            order_book_snap.order_book.add_ask(Level{
                exchange: Exchanges::BINANCE,
                price: json_msg.asks[index].price, 
                amount: json_msg.asks[index].amount});
        }
        
        //println!("Binance snap: {:#?}", orderbook);
        
        Ok(order_book_snap)
    }

    async fn run(&self) {

        let final_address = format!("{}{}@depth{}@100ms", self.info.address, self.info.currency, self.info.depth.to_string());

        let url = match url::Url::parse(&final_address) {
            Ok(u) => u,
            Err(e) => {
                println!("Failed to parse address for {}: {:?}", self.info.name, e);
                return;
            }
        };
        
        let (ws_stream, _response) = match connect_async(url).await{
            Ok((s,r)) => (s, r),
            Err(e) => {
                println!("Failed to connect to {}: {:?}", self.info.name, e);    
                return;
            }
        };
    
        let (mut write, mut read) = ws_stream.split();
      
        while let Some(msg) = read.next().await {
             let message = match msg {
                Ok(m) => m,
                Err(e) => {
                    println!("{} recv msg err: {:#?}", self.info.name, e);
                    continue;
                }
            };
             match message{
                Message::Text(msg) => {
                    match self.normalize(&msg) {
                    Ok(orderbook) => { 
                        if let Err(msg) = self.info.sender.send(orderbook).await {
                            println!("Failed to send orderbook snap: {msg}");
                        }; },
                    Err(_) => { println!("Failed to normalize msg for {}", self.info.name) },
                    }
                },
                
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_)=> {},
                Message::Binary(_) => (),
                Message::Close(e) => {
                    println!("Disconnected {:?}", e);
                    return;
                }
            }
        }
    }

}