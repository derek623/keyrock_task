use tokio::{sync::mpsc::Receiver};
use crate::order_book_snap::OrderBookSnap;
use std::collections::HashMap;
use crate::marketdatasource::{OrderBook, Level};
use std::cmp::Ordering;

/*pub struct AggregatedLevel {
    exchange: Exchanges,
    price: f64,
    amount: f64,
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
}*/

pub struct Aggregator {
    rx: Receiver<OrderBookSnap>,
    //aggregated_order_book: AggregatedOrderBook,
    currency_to_orderbook_map: HashMap<String, OrderBook>,
    max_depth: usize,

}

impl Aggregator {
    pub fn new (rx: Receiver<OrderBookSnap>, max_depth: usize) -> Aggregator {
        Aggregator { rx, currency_to_orderbook_map: HashMap::new(), max_depth }
    }

    pub fn merge(&mut self, order_book_snap: OrderBookSnap) {
        /*match order_book_snap.get_exchange() {
            Exchanges::BINANCE => { println!("Aggregator got Binance msg: {:#?}", order_book_snap); }
            Exchanges::BITSTAMP => { println!("Aggregator got Bitstamp msg: {:#?}", order_book_snap); }
        }*/

       /*  let mut need_merging = true;
        let mut order_book = self.currency_to_orderbook_map.entry(order_book_snap.order_book.get_currency().to_string()).
        or_insert_with(|| {
            let mut ob = order_book_snap.order_book;
            {
                let asks = ob.get_asks_mut();
                asks.reserve(self.max_depth - asks.len());
            }
            let bids = ob.get_bids_mut();
            bids.reserve(self.max_depth - bids.len());
            println!("Creating orderbook for {}: {:?}", ob.get_currency(), ob);
            need_merging = false;
            ob
        });

        if need_merging {
            unsafe {
            let exchange = order_book_snap.exchange;
            //merge bid
            let new_bids: Vec<Level> = order_book_snap.order_book.bids;
            }
        } */   
        use std::collections::hash_map::Entry;
        match self.currency_to_orderbook_map.entry(order_book_snap.order_book.get_currency().to_string()) {
            Entry::Occupied(mut o) => {
                let exchange = order_book_snap.exchange;
                let mut order_book = o.get_mut();
                //merge bid
                let mut new_bids: Vec<Level> = order_book_snap.order_book.bids;
                new_bids.reserve(self.max_depth - new_bids.len());
                //new bids now contain all the entry from order_book_snap. Next is to insert all entries from the existing merge order book 
                //which is not from this exchange to the new vector
                for n in &order_book.bids{
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
                order_book.bids = new_bids;
                println!("\norder_book.bids = {:?}", order_book.bids);
                //merge ask
                let mut new_asks: Vec<Level> = order_book_snap.order_book.asks;
                new_asks.reserve(self.max_depth - new_asks.len());
                //new bids now contain all the entry from order_book_snap. Next is to insert all entries from the existing merge order book 
                //which is not from this exchange to the new vector
                for n in &order_book.asks{
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
                order_book.asks = new_asks;
                println!("order_book.asks = {:?}", order_book.asks);
                
            },
            Entry::Vacant(v) => { 
                let mut ob = order_book_snap.order_book; 
                {
                    let asks = ob.get_asks_mut();
                    asks.reserve(self.max_depth - asks.len());
                }
                let bids = ob.get_bids_mut();
                bids.reserve(self.max_depth - bids.len());
                println!("Creating orderbook for {}: {:?}", ob.get_currency(), ob);
                v.insert(ob);
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
