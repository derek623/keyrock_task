use tokio::{sync::mpsc::Receiver};
use crate::order_book_snap::OrderBookSnap;
use std::collections::HashMap;
use crate::marketdatasource::{OrderBook, Level};
use std::cmp::Ordering;

#[derive(Default, Debug)]
pub struct AggregatedOrderBook {
    pub spread: f64,
    pub order_book: OrderBook,
}

impl AggregatedOrderBook {
    pub fn new(spread: f64, order_book: OrderBook) -> AggregatedOrderBook {
        AggregatedOrderBook {spread, order_book}
    }
}

pub struct Aggregator {
    rx: Receiver<OrderBookSnap>,
    //aggregated_order_book: AggregatedOrderBook,
    currency_to_agg_orderbook_map: HashMap<String, AggregatedOrderBook>,
    max_depth: usize,

}

impl Aggregator {
    pub fn new (rx: Receiver<OrderBookSnap>, max_depth: usize) -> Aggregator {
        Aggregator { rx, currency_to_agg_orderbook_map: HashMap::new(), max_depth }
    }

    pub fn merge(&mut self, order_book_snap: OrderBookSnap) {
        /*match order_book_snap.get_exchange() {
            Exchanges::BINANCE => { println!("Aggregator got Binance msg: {:#?}", order_book_snap); }
            Exchanges::BITSTAMP => { println!("Aggregator got Bitstamp msg: {:#?}", order_book_snap); }
        }*/

        use std::collections::hash_map::Entry;
        match self.currency_to_agg_orderbook_map.entry(order_book_snap.order_book.currency.to_string()) {
            Entry::Occupied(mut o) => {
                let exchange = order_book_snap.exchange;
                let mut agg_order_book = o.get_mut();
                //merge bid
                let mut new_bids: Vec<Level> = order_book_snap.order_book.bids;
                new_bids.reserve(self.max_depth - new_bids.len());
                //new bids now contain all the entry from order_book_snap. Next is to insert all entries from the existing merge order book 
                //which is not from this exchange to the new vector
                for n in &agg_order_book.order_book.bids{
                    if exchange != n.exchange {
                        new_bids.push(n.to_owned());
                    }
                }
                new_bids.sort_unstable_by(|l1, l2| {
                    if l2.price > l1.price {
                        Ordering::Greater
                    } else {
                        Ordering::Less
                    }
                });
                agg_order_book.order_book.bids = new_bids;
                println!("\norder_book.bids = {:?}", agg_order_book.order_book.bids);
                //merge ask
                let mut new_asks: Vec<Level> = order_book_snap.order_book.asks;
                new_asks.reserve(self.max_depth - new_asks.len());
                //new bids now contain all the entry from order_book_snap. Next is to insert all entries from the existing merge order book 
                //which is not from this exchange to the new vector
                for n in &agg_order_book.order_book.asks{
                    if exchange != n.exchange {
                        new_asks.push(n.to_owned());
                    }
                }
                new_asks.sort_unstable_by(|l1, l2| {
                    if l1.price < l2.price {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                } );
                agg_order_book.order_book.asks = new_asks;
                println!("order_book.asks = {:?}", agg_order_book.order_book.asks);
                
            },
            Entry::Vacant(v) => { 
                let mut order_book = order_book_snap.order_book; 
                {
                    let mut asks = &mut order_book.asks;
                    asks.reserve(self.max_depth - asks.len());
                }
                let mut bids = &mut order_book.bids;
                bids.reserve(self.max_depth - bids.len());
                println!("Creating orderbook for {}: {:?}", order_book.currency, order_book);
                v.insert(AggregatedOrderBook::new(0_f64, order_book));
            },
        }
    }

    pub async fn run(&mut self) {
        while let Some(msg) = self.rx.recv().await {
            self.merge(msg);
            //println!("Aggregator got {:#?}", msg);
        }
    }
}
