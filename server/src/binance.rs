use tokio_tungstenite::connect_async;
use futures_util::StreamExt;
use crate::{MarketDataSource, order_book_snap, marketdatasource::Exchanges};
use async_trait::async_trait;
use crate::order_book_snap::{OrderBookSnap, Level};
use serde::{Deserialize};
use tokio::sync::mpsc::Sender;

#[derive(Debug, Deserialize)]
struct BinanceLevel {
    price: String,
    amount: String,
}

#[derive(Debug, Deserialize)]
struct BinanceJson {
    bids: Vec<BinanceLevel>,
    asks: Vec<BinanceLevel>,
}
pub struct Binance {
    address: String,
    currency: String,
    depth: usize,
    sender: Sender<OrderBookSnap>,
}

impl Binance {
    pub fn new(address: &str, currency: &str, depth: usize, sender: Sender<order_book_snap::OrderBookSnap>) -> impl MarketDataSource {
        Binance { address: address.to_string(), currency: currency.to_string(), depth, sender }
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

        let mut orderbook = OrderBookSnap::new(Exchanges::BINANCE, self.depth);

        for index in 0..self.depth {
            let price = match json_msg.bids[index].price.parse::<f32>() {
                Ok(p) => p,
                Err(_) => { return Err(()); }
            };
            let amount = match json_msg.bids[index].amount.parse::<f32>() {
                Ok(p) => p,
                Err(_) => { return Err(()); }
            };
            orderbook.add_bid(Level{
                exchange: Exchanges::BINANCE, 
                price, 
                amount});
            let price = match json_msg.asks[index].price.parse::<f32>() {
                Ok(p) => p,
                Err(_) => { return Err(()); }
            };
            let amount = match json_msg.asks[index].amount.parse::<f32>() {
                Ok(p) => p,
                Err(_) => { return Err(()); }
            };
            orderbook.add_ask(Level{
                exchange: Exchanges::BINANCE,
                price, 
                amount});
        }
        
        //println!("Binance snap: {:#?}", orderbook);
        
        Ok(orderbook)
    }

    async fn run(&self) {

        let final_address = format!("{}{}@depth{}@100ms", self.address, self.currency, self.depth.to_string());
        let url = url::Url::parse(&final_address).unwrap();
        
    
        let (ws_stream, _response) = connect_async(url).await.expect("Failed to connect");
    
        let (mut write, read) = ws_stream.split();
      
        let read_future = read.for_each(|message| async {
            
             let data = message.unwrap().into_text().unwrap();
             match self.normalize(&data) {
                Ok(orderbook) => { self.sender.send(orderbook).await; },
                Err(_) => { println!("Failed to normalize msg for binance") },
             }
            
        });
    
        read_future.await;
    }

}