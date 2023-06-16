use tokio::{sync::mpsc::Receiver};
use crate::order_book_snap::OrderBookSnap;
use crate::marketdatasource::Exchanges;

pub struct AggregatedLevel {
    exchange: Exchanges,
    price: f32,
    amount: f32,
}
pub struct AggregatedOrderBook {
    max_depth: usize,
    bids: Vec<AggregatedLevel>,
    asks: Vec<AggregatedLevel>,
}

impl AggregatedOrderBook {
    pub fn new (max_depth: usize) ->AggregatedOrderBook {
        AggregatedOrderBook {
            max_depth,
            bids: Vec::with_capacity(max_depth),
            asks: Vec::with_capacity(max_depth)
        }
    }

    pub fn get_bids(&self) -> &Vec<AggregatedLevel> {
        &self.bids
    }

    pub fn get_asks(&self) -> &Vec<AggregatedLevel> {
        &self.asks
    }

    pub fn merge(order_book_snap: OrderBookSnap) {
        
    }
}

pub struct Aggregator {
    rx: Receiver<OrderBookSnap>,
    aggregated_order_book: AggregatedOrderBook,
}

impl Aggregator {
    pub fn new (rx: Receiver<OrderBookSnap>, max_depth: usize) -> Aggregator {
        Aggregator { rx, aggregated_order_book: AggregatedOrderBook::new(max_depth) }
    }

    pub async fn run(&mut self) {
        while let Some(msg) = self.rx.recv().await {
            match msg.get_exchange() {
                Exchanges::BINANCE => { println!("Aggregator got Binance msg: {:#?}", msg); }
                Exchanges::BITSTAMP => { println!("Aggregator got Bitstamp msg: {:#?}", msg); }
            }
            //println!("Aggregator got {:#?}", msg);
        }
    }
}
