use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use crate::MarketDataSource;
use async_trait::async_trait;
use crate::order_book_snap::{OrderBookSnap, Level};
use serde_json::{json, Value};

pub struct Bitstamp {
    address: String,
    currency: String,
}

impl Bitstamp {
    pub fn new(address: &str, currency: &str) -> impl MarketDataSource {
        Bitstamp { address: address.to_string(), currency: currency.to_string()}
    }
}

#[async_trait]
impl MarketDataSource for Bitstamp {
    //fn normalize(&self, msg: &str) -> OrderBookSnap<10> {
    fn normalize(&self, msg: &str) -> Result<OrderBookSnap<10>, ()> {
        let json_msg: Value = match serde_json::from_str(msg) {
            Ok(msg) => msg,
            Err(e) => { return Err(()); }
        };

        let exchange = "Bitstamp";

        let bids = &json_msg["data"]["bids"];
        let asks = &json_msg["data"]["asks"];

        if bids.is_null() || asks.is_null() {
            println!("failed to normlaize {}", msg);
            return Err(());
        }

        let mut orderbook: OrderBookSnap<{Bitstamp::MAX_DEPTH}> = OrderBookSnap::new();

        for index in 0..Bitstamp::MAX_DEPTH {
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
        let url = url::Url::parse(&self.address).unwrap();
        
    
        let (ws_stream, _response) = connect_async(url).await.expect("Failed to connect");
            
        let (mut write, read) = ws_stream.split();
      
        let msg = json!({
            "event": "bts:subscribe",
            "data": {
                "channel": format!("order_book_{}", self.currency)
            }
        });
        println!("Bitstamp sub message: {}", msg.to_string());
        write.send(Message::Text(msg.to_string())).await.unwrap();
        
    
        let read_future = read.for_each(|message| async {
            let data = message.unwrap().into_text().unwrap();
            match self.normalize(&data) {
               Ok(orderbook) => {},
               Err(_) => println!("Failed to normalize msg for bitstamp"),
            }
        });
    
        read_future.await;
    }

}