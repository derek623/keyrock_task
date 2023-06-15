use tokio::{sync::mpsc::Receiver};
use crate::order_book_snap::OrderBookSnap;
use crate::marketdatasource::Exchanges;

pub struct Aggregator {
    rx: Receiver<OrderBookSnap>,
    num_source: usize,
}

impl Aggregator {
    pub fn new (rx: Receiver<OrderBookSnap>, num_source: usize) -> Aggregator {
        Aggregator { rx, num_source }
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
