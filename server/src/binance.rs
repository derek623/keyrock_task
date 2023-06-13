use tokio_tungstenite::connect_async;
use futures_util::StreamExt;
use crate::MarketDataSource;
use async_trait::async_trait;
use crate::order_book_snap::{OrderBookSnap, Level};
use serde_json::Value;

pub struct Binance {
    address: String,
    currency: String
}

impl Binance {
    pub fn new(address: &str, currency: &str) -> impl MarketDataSource {
        Binance { address: address.to_string(), currency: currency.to_string() }
    }
}

#[async_trait]
impl MarketDataSource for Binance {
    fn normalize(&self, msg: &str) -> Result<OrderBookSnap<10>, ()>{
        
        let json_msg: Value = serde_json::from_str(msg).unwrap();

        let exchange = "Binance";
        let bids = &json_msg["bids"];
        let asks = &json_msg["asks"];

        let mut orderbook: OrderBookSnap<{Binance::MAX_DEPTH}> = OrderBookSnap::new();

        for index in 0..Binance::MAX_DEPTH {
            orderbook.add_bid(Level{
                exchange: exchange.to_string(), 
                price: bids[index][0].as_str().unwrap().parse::<f32>().unwrap(), 
                amount: bids[index][1].as_str().unwrap().parse::<f32>().unwrap()});
            orderbook.add_ask(Level{
                exchange: exchange.to_string(), 
                price: asks[index][0].as_str().unwrap().parse::<f32>().unwrap(), 
                amount: asks[index][1].as_str().unwrap().parse::<f32>().unwrap()});
        }
        
        println!("Binance snap: {:#?}", orderbook);
        
        Ok(orderbook)
    }

    async fn run(&self) {

        let final_address = format!("{}{}@depth{}@100ms", self.address, self.currency, Binance::MAX_DEPTH.to_string());
        let url = url::Url::parse(&final_address).unwrap();
        
    
        let (ws_stream, _response) = connect_async(url).await.expect("Failed to connect");
    
        let (mut write, read) = ws_stream.split();
      
        let read_future = read.for_each(|message| async {
            
             let data = message.unwrap().into_text().unwrap();
             match self.normalize(&data) {
                Ok(orderbook) => {},
                Err(_) => { println!("Failed to normalize msg for binance") },
             }
            
        });
    
        read_future.await;
    }

}