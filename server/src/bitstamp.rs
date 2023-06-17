use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use async_trait::async_trait;
use crate::order_book_snap::{OrderBookSnap, Level};
use crate::{MarketDataSource, order_book_snap, marketdatasource::Exchanges};
use serde_json::json;
use serde::Deserialize;
use tokio::sync::mpsc::Sender;
use crate::utility::de_f64_or_string_as_f64;

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
    address: String,
    currency: String,
    depth: usize,
    sender: Sender<OrderBookSnap>,
}

impl Bitstamp {
    pub fn new(address: &str, currency: &str, depth: usize, sender: Sender<OrderBookSnap>,) -> impl MarketDataSource {
        Bitstamp { address: address.to_string(), currency: currency.to_string(), depth, sender}
    }
}

#[async_trait]
impl MarketDataSource for Bitstamp {
    //fn normalize(&self, msg: &str) -> OrderBookSnap<10> {
    fn normalize(&self, msg: &str) -> Result<OrderBookSnap, ()> {

        let json_msg: BitstampJson = match serde_json::from_str(msg) {
            Ok(msg) => msg,
            Err(e) => { return Err(()); }
        };

        //println!("bitstamp JSON is: {:#?}\n", json_msg);

        //let exchange = "Bitstamp";
        let mut orderbook = OrderBookSnap::new(Exchanges::BITSTAMP, self.depth, &self.currency);

        for index in 0..self.depth {
            orderbook.add_bid(Level{
                exchange: Exchanges::BITSTAMP, 
                price: json_msg.data.bids[index].price, 
                amount: json_msg.data.bids[index].amount});
            orderbook.add_ask(Level{
                exchange: Exchanges::BITSTAMP, 
                price: json_msg.data.asks[index].price, 
                amount: json_msg.data.asks[index].amount});
        }
        
        //println!("Bitstamp snap: {:#?}", orderbook);
        
        Ok(orderbook)
    }
    async fn run(&self) {
        let url = url::Url::parse(&self.address).unwrap();
        
    
        let (ws_stream, _response) = connect_async(url).await.expect("Failed to connect");
            
        let (mut write, mut read) = ws_stream.split();
      
        let msg = json!({
            "event": "bts:subscribe",
            "data": {
                "channel": format!("order_book_{}", self.currency)
            }
        });
        println!("Bitstamp sub message: {}", msg.to_string());
        write.send(Message::Text(msg.to_string())).await.unwrap();

        while let Some(msg) = read.next().await {
            let data = msg.unwrap().into_text().unwrap();
             match self.normalize(&data) {
                Ok(orderbook) => { 
                    if let Err(msg) = self.sender.send(orderbook).await {
                        println!("Failed to send orderbook snap: {msg}");
                    }; },
                Err(_) => { println!("Failed to normalize msg for bitstamp") },
             }
        }
    }

}