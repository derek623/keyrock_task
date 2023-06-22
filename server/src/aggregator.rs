use tokio::sync::mpsc::{Receiver, Sender};
use std::collections::HashMap;
use crate::marketdatasource::{OrderBook, OrderBookSnap};
use crate::orderbook::Level;
use std::cmp::Ordering;

#[derive(Default, Debug, Clone)]
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
    tx: Sender<AggregatedOrderBook>,
    //aggregated_order_book: AggregatedOrderBook,
    currency_to_agg_orderbook_map: HashMap<String, AggregatedOrderBook>,
    max_depth: usize,

}

impl Aggregator {
    pub fn new (rx: Receiver<OrderBookSnap> ,tx: Sender<AggregatedOrderBook> ,max_depth: usize) -> Aggregator {
        Aggregator { rx, tx, currency_to_agg_orderbook_map: HashMap::new(), max_depth } 
    }

    /*pub fn merge_side<F>(&self, new_side: &mut Vec<Level>, old_side: &Vec<Level>, exchange: &Exchanges, cmp: F) where
    F: FnMut(&Level, &Level) -> Ordering {
        //let mut new_bids: Vec<Level> = order_book_snap.order_book.bids;
        new_side.reserve(self.max_depth - new_side.len());
        //new bids now contain all the entry from order_book_snap. Next is to insert all entries from the existing merge order book 
        //which is not from this exchange to the new vector
        for n in old_side{
            if exchange != &n.exchange {
                new_side.push(n.to_owned());
            }
        }
        new_side.sort_unstable_by(cmp);
        //agg_order_book.order_book.bids = new_side;
    }*/

    pub fn merge(&mut self, order_book_snap: OrderBookSnap) -> Result<AggregatedOrderBook, String> {

        use std::collections::hash_map::Entry;
        let agg_order_book = match self.currency_to_agg_orderbook_map.entry(order_book_snap.order_book.currency.to_string()) {
            Entry::Occupied(o) => {
                let exchange = order_book_snap.exchange;
                let mut agg_order_book = o.into_mut();
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
                    if l2.price > l1.price || (l2.price == l1.price && l2.amount > l1.amount){
                        Ordering::Greater
                    } else {
                        Ordering::Less
                    }
                });

                /*if agg_order_book.order_book.bids.iter().zip(new_bids.iter()).filter(|&(a, b)| a == b ).count() == self.max_depth {
                    println!("No chance in bids!: {:?}, {:?}", agg_order_book.order_book.bids, new_bids);
                }*/

                agg_order_book.order_book.bids = new_bids;

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
                    if l1.price < l2.price  || (l2.price == l1.price && l2.amount > l1.amount){
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                } );
                agg_order_book.order_book.asks = new_asks;

                agg_order_book
                
            },
            Entry::Vacant(v) => { 
                let mut order_book = order_book_snap.order_book; 
                {
                    let asks = &mut order_book.asks;
                    asks.reserve((self.max_depth * 2)- asks.len()); //the orderbook within the aggregatedOrderBook needs to keep a size of 2 * max_depth
                }
                let bids = &mut order_book.bids;
                bids.reserve((self.max_depth * 2) - bids.len());
                println!("Creating orderbook for {}: {:?}", order_book.currency, order_book);
                v.insert(AggregatedOrderBook::new(0_f64, order_book))
            },
        };

        //now calculate the spread
        println!("agg_order_book is = {:?}", agg_order_book);
        if agg_order_book.order_book.bids.len() <= 0 || agg_order_book.order_book.asks.len() <= 0 {
            return Err("Fail to calculate spread as either bid or ask queue are empty".to_string());
        }
        agg_order_book.spread = agg_order_book.order_book.asks[0].price - agg_order_book.order_book.bids[0].price;
        
        Ok(agg_order_book.to_owned())
    }

    pub async fn run(&mut self) {
        while let Some(msg) = self.rx.recv().await {
            match self.merge(msg) {
                Ok(agg_order_book) => { 
                    match self.tx.send(agg_order_book).await {
                        Ok(_) => {},
                        Err(e) => { println!("Fail to send the following Aggreated order book to the grpc server: {:?}", e); },
                    }
                },
                Err(emsg) => { println!("Merging snapshot return error: {}", emsg); },
            }
            //println!("Aggregator got {:#?}", msg);
        }
    }
}
