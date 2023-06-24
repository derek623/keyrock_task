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
    currency_to_agg_orderbook_map: HashMap<String, AggregatedOrderBook>,
    max_depth: usize,

}

impl Aggregator {
    pub fn new (rx: Receiver<OrderBookSnap> ,tx: Sender<AggregatedOrderBook> ,max_depth: usize) -> Aggregator {
        Aggregator { rx, tx, currency_to_agg_orderbook_map: HashMap::new(), max_depth } 
    }

    fn merge_side<F>(mut updated_queue: Vec<Level>, merged_queue: &Vec<Level>, exchange: &str, depth: usize, cmp: F) -> Result<Vec<Level>, String>
    where
    F: FnMut(&Level, &Level) -> Ordering {
        if depth < updated_queue.len() {
            return Err("Depth is smaller then updated queue length, fail to merge!".to_string());
        }

        let mut additional = depth - updated_queue.len();
        updated_queue.reserve(additional);
        //new bids now contain all the entry from order_book_snap. Next is to insert all entries from the existing merge order book 
        //which is not from this exchange to the new vector


        for n in merged_queue{
            if additional <= 0 {
                break;
            }
            if exchange != &n.exchange {
                updated_queue.push(n.to_owned());
                additional -= 1;
            }
        }
        updated_queue.sort_unstable_by(cmp);
        
        Ok(updated_queue)
    }

    pub fn merge_bid(updated_depth: Vec<Level>, old_depth: &Vec<Level>, exchange: &str, depth: usize,) -> Result<Vec<Level>, String> {
        Aggregator::merge_side(updated_depth, old_depth, exchange, depth, |l1, l2| {
            if l2.price > l1.price || (l2.price == l1.price && l2.amount > l1.amount){
                Ordering::Greater
            } else {
                Ordering::Less
            }})
    }

    pub fn merge_ask(updated_depth: Vec<Level>, old_depth: &Vec<Level>, exchange: &str, depth: usize,) -> Result<Vec<Level>, String> {
        Aggregator::merge_side(updated_depth, old_depth, exchange, depth, |l1, l2| {
            if l1.price < l2.price || (l2.price == l1.price && l1.amount > l2.amount){
                Ordering::Less
            } else {
                Ordering::Greater
            }
        })
    }   

    pub fn merge(&mut self, order_book_snap: OrderBookSnap) -> Result<AggregatedOrderBook, String> {

        use std::collections::hash_map::Entry;
        let agg_order_book = match self.currency_to_agg_orderbook_map.entry(order_book_snap.order_book.currency.to_string()) {
            Entry::Occupied(o) => {
                let exchange = order_book_snap.exchange;
                let agg_order_book = o.into_mut();
                //merge bid
                let result_bids = match Aggregator::merge_bid(order_book_snap.order_book.bids, 
                    &agg_order_book.order_book.bids, &exchange, self.max_depth) {
                        Ok(r) => r,
                        Err(e) => { return Err(e); }
                    };
                //merge ask
                let result_asks = match Aggregator::merge_ask(order_book_snap.order_book.asks, 
                    &agg_order_book.order_book.asks, &exchange, self.max_depth) {
                        Ok(r) => r,
                        Err(e) => { return Err(e); }
                    };

                /*if agg_order_book.order_book.bids.iter().zip(new_bids.iter()).filter(|&(a, b)| a == b ).count() == self.max_depth {
                    println!("No chance in bids!: {:?}, {:?}", agg_order_book.order_book.bids, new_bids);
                }*/

                agg_order_book.order_book.bids.clone_from(&result_bids);
                agg_order_book.order_book.asks.clone_from(&result_asks);

                agg_order_book
                
            },
            Entry::Vacant(v) => { 
                let mut order_book = order_book_snap.order_book; 
                {
                    let asks = &mut order_book.asks;
                    asks.reserve((self.max_depth)- asks.len()); //the orderbook within the aggregatedOrderBook needs to keep a size of max_depth
                }
                let bids = &mut order_book.bids;
                bids.reserve((self.max_depth) - bids.len());
                log::info!("Creating orderbook for {}: {:?}", order_book.currency, order_book);
                v.insert(AggregatedOrderBook::new(0_f64, order_book))
            },
        };

        log::info!("{:?}", agg_order_book);
        //now calculate the spread
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
                        Err(e) => { log::error!("Fail to send the following Aggreated order book to the grpc server: {:?}", e); },
                    }
                },
                Err(emsg) => { log::error!("Merging snapshot return error: {}", emsg); },
            }
        }
    }
}


mod test {
    use super::*;

    #[test]
    fn test_merge_bid() {         
        //pub fn merge_bid(updated_depth: Vec<Level>, old_depth: &Vec<Level>, exchange: &str, depth: usize) -> Vec<Level>
        //error case
        assert_eq!(if let Err(e) = Aggregator::merge_bid(vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}], &vec![], "", 0) {
            e
        } else { "".to_string() },
         "Depth is smaller then updated queue length, fail to merge!".to_string());

        //normal case
        assert_eq!(Aggregator::merge_bid(vec![], &vec![], "", 10).unwrap(), vec![]);
        assert_eq!(Aggregator::merge_bid(vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}], &vec![Level{exchange: "a".to_string(), 
            price: 1_f64, amount: 1_f64}], "a", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}]);
        
        //only 1 exchange is involved
        assert_eq!(Aggregator::merge_bid(
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}],
            &vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}], "a", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}]);
        assert_eq!(Aggregator::merge_bid(
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 2_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}],
            &vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}], "a", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 2_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}]);
        assert_eq!(Aggregator::merge_bid(
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 2_f64, amount: 1_f64}],
            &vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 2_f64, amount: 1_f64}], "a", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 2_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}]);
        assert_eq!(Aggregator::merge_bid(
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 2_f64, amount: 1_f64}],
            &vec![Level{exchange: "a".to_string(), price: 3_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 5_f64, amount: 1_f64}], "a", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 2_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}]);
        assert_eq!(Aggregator::merge_bid(
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}],
            &vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}], "", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}]);

        //multiple exchanges are involved
        assert_eq!(Aggregator::merge_bid(
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1.2}],
            &vec![Level{exchange: "b".to_string(), price: 1_f64, amount: 1_f64}], "a", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1.2}, Level{exchange: "b".to_string(), price: 1_f64, amount: 1_f64}]);
        assert_eq!(Aggregator::merge_bid(
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1.2}],
            &vec![Level{exchange: "b".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "c".to_string(), price: 1_f64, amount: 1.1_f64}], "a", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1.2}, Level{exchange: "c".to_string(), price: 1_f64, amount: 1.1_f64}, 
            Level{exchange: "b".to_string(), price: 1_f64, amount: 1_f64}]);
        assert_eq!(Aggregator::merge_bid(
            vec![Level{exchange: "a".to_string(), price: 1.7_f64, amount: 1.2}, Level{exchange: "a".to_string(), price: 1.6_f64, amount: 1.2}],
            &vec![Level{exchange: "a".to_string(), price: 1.5_f64, amount: 1.2},
                Level{exchange: "a".to_string(), price: 1.4_f64, amount: 1.2},
                Level{exchange: "b".to_string(), price: 1.3_f64, amount: 1_f64}, 
                Level{exchange: "b".to_string(), price: 1.2_f64, amount: 1_f64}, 
                Level{exchange: "c".to_string(), price: 1.1_f64, amount: 1.1_f64},
                Level{exchange: "c".to_string(), price: 1_f64, amount: 1.1_f64}], "a", 6).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1.7_f64, amount: 1.2},
                Level{exchange: "a".to_string(), price: 1.6_f64, amount: 1.2},
                Level{exchange: "b".to_string(), price: 1.3_f64, amount: 1_f64}, 
                Level{exchange: "b".to_string(), price: 1.2_f64, amount: 1_f64}, 
                Level{exchange: "c".to_string(), price: 1.1_f64, amount: 1.1_f64},
                Level{exchange: "c".to_string(), price: 1_f64, amount: 1.1_f64}]);
        assert_eq!(Aggregator::merge_bid(
            vec![Level{exchange: "a".to_string(), price: 0.9_f64, amount: 1.2}, Level{exchange: "a".to_string(), price: 0.8_f64, amount: 1.2}],
            &vec![Level{exchange: "a".to_string(), price: 1.5_f64, amount: 1.2},
                Level{exchange: "a".to_string(), price: 1.4_f64, amount: 1.2},
                Level{exchange: "b".to_string(), price: 1.3_f64, amount: 1_f64}, 
                Level{exchange: "b".to_string(), price: 1.2_f64, amount: 1_f64}, 
                Level{exchange: "c".to_string(), price: 1.1_f64, amount: 1.1_f64},
                Level{exchange: "c".to_string(), price: 1_f64, amount: 1.1_f64}], "a", 6).unwrap(), 
            vec![Level{exchange: "b".to_string(), price: 1.3_f64, amount: 1_f64}, 
                Level{exchange: "b".to_string(), price: 1.2_f64, amount: 1_f64}, 
                Level{exchange: "c".to_string(), price: 1.1_f64, amount: 1.1_f64},
                Level{exchange: "c".to_string(), price: 1_f64, amount: 1.1_f64},
                Level{exchange: "a".to_string(), price: 0.9_f64, amount: 1.2},
                Level{exchange: "a".to_string(), price: 0.8_f64, amount: 1.2}]);
        assert_eq!(Aggregator::merge_bid(
            vec![Level{exchange: "a".to_string(), price: 0.9_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 0.8_f64, amount: 1_f64}],
            &vec![Level{exchange: "a".to_string(), price: 1.5_f64, amount: 1_f64},
                Level{exchange: "b".to_string(), price: 1.4_f64, amount: 1_f64},
                Level{exchange: "b".to_string(), price: 1.3_f64, amount: 1_f64}, 
                Level{exchange: "a".to_string(), price: 1.2_f64, amount: 1_f64}, 
                Level{exchange: "c".to_string(), price: 1.1_f64, amount: 1.1_f64},
                Level{exchange: "c".to_string(), price: 1_f64, amount: 1.1_f64}], "a", 6).unwrap(), 
            vec![Level{exchange: "b".to_string(), price: 1.4_f64, amount: 1_f64}, 
                Level{exchange: "b".to_string(), price: 1.3_f64, amount: 1_f64}, 
                Level{exchange: "c".to_string(), price: 1.1_f64, amount: 1.1_f64},
                Level{exchange: "c".to_string(), price: 1_f64, amount: 1.1_f64},
                Level{exchange: "a".to_string(), price: 0.9_f64, amount: 1_f64},
                Level{exchange: "a".to_string(), price: 0.8_f64, amount: 1_f64}]);
    }

    #[test]
    fn test_merge_ask() {         
        //error case
        assert_eq!(if let Err(e) = Aggregator::merge_ask(vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}], &vec![], "", 0) {
            e
        } else { "".to_string() },
         "Depth is smaller then updated queue length, fail to merge!".to_string());

        //normal case
        assert_eq!(Aggregator::merge_ask(vec![], &vec![], "", 10).unwrap(), vec![]);
        assert_eq!(Aggregator::merge_ask(vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}], &vec![Level{exchange: "a".to_string(), 
            price: 1_f64, amount: 1_f64}], "a", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}]);

        //only 1 exchange is involved
        assert_eq!(Aggregator::merge_ask(
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}],
            &vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}], "a", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}]);
        assert_eq!(Aggregator::merge_ask(
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 2_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}],
            &vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}], "a", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 2_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}]);
        assert_eq!(Aggregator::merge_ask(
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 2_f64, amount: 1_f64}],
            &vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 2_f64, amount: 1_f64}], "a", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 2_f64, amount: 1_f64}]);
        assert_eq!(Aggregator::merge_ask(
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 2_f64, amount: 1_f64}],
            &vec![Level{exchange: "a".to_string(), price: 3_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 5_f64, amount: 1_f64}], "a", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 2_f64, amount: 1_f64}]);
        assert_eq!(Aggregator::merge_ask(
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}],
            &vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}], "", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}]);

        //multiple exchanges are involved
        assert_eq!(Aggregator::merge_ask(
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1.2}],
            &vec![Level{exchange: "b".to_string(), price: 1_f64, amount: 1_f64}], "a", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1.2}, Level{exchange: "b".to_string(), price: 1_f64, amount: 1_f64}]);
        assert_eq!(Aggregator::merge_ask(
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1.2}],
            &vec![Level{exchange: "b".to_string(), price: 1_f64, amount: 1_f64}, Level{exchange: "c".to_string(), price: 1_f64, amount: 1.1_f64}], "a", 10).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1.2}, Level{exchange: "c".to_string(), price: 1_f64, amount: 1.1_f64}, 
            Level{exchange: "b".to_string(), price: 1_f64, amount: 1_f64}]);
        assert_eq!(Aggregator::merge_ask(
            vec![Level{exchange: "a".to_string(), price: 1.1_f64, amount: 1.2}, Level{exchange: "a".to_string(), price: 1.2_f64, amount: 1.2}],
            &vec![Level{exchange: "a".to_string(), price: 1.3_f64, amount: 1.2},
                Level{exchange: "a".to_string(), price: 1.4_f64, amount: 1.2},
                Level{exchange: "b".to_string(), price: 1.5_f64, amount: 1_f64}, 
                Level{exchange: "b".to_string(), price: 1.6_f64, amount: 1_f64}, 
                Level{exchange: "c".to_string(), price: 1.7_f64, amount: 1.1_f64},
                Level{exchange: "c".to_string(), price: 1.8_f64, amount: 1.1_f64}], "a", 6).unwrap(), 
            vec![Level{exchange: "a".to_string(), price: 1.1_f64, amount: 1.2},
                Level{exchange: "a".to_string(), price: 1.2_f64, amount: 1.2},
                Level{exchange: "b".to_string(), price: 1.5_f64, amount: 1_f64}, 
                Level{exchange: "b".to_string(), price: 1.6_f64, amount: 1_f64}, 
                Level{exchange: "c".to_string(), price: 1.7_f64, amount: 1.1_f64},
                Level{exchange: "c".to_string(), price: 1.8_f64, amount: 1.1_f64}]);
        assert_eq!(Aggregator::merge_ask(
            vec![Level{exchange: "a".to_string(), price: 2.0_f64, amount: 1.2}, Level{exchange: "a".to_string(), price: 2.1_f64, amount: 1.2}],
            &vec![Level{exchange: "a".to_string(), price: 0.9_f64, amount: 1.2},
                Level{exchange: "a".to_string(), price: 1.0_f64, amount: 1.2},
                Level{exchange: "b".to_string(), price: 1.1_f64, amount: 1_f64}, 
                Level{exchange: "b".to_string(), price: 1.2_f64, amount: 1_f64}, 
                Level{exchange: "c".to_string(), price: 1.3_f64, amount: 1.1_f64},
                Level{exchange: "c".to_string(), price: 1.5_f64, amount: 1.1_f64}], "a", 6).unwrap(), 
            vec![Level{exchange: "b".to_string(), price: 1.1_f64, amount: 1_f64}, 
                Level{exchange: "b".to_string(), price: 1.2_f64, amount: 1_f64}, 
                Level{exchange: "c".to_string(), price: 1.3_f64, amount: 1.1_f64},
                Level{exchange: "c".to_string(), price: 1.5_f64, amount: 1.1_f64},
                Level{exchange: "a".to_string(), price: 2.0_f64, amount: 1.2},
                Level{exchange: "a".to_string(), price: 2.1_f64, amount: 1.2}]);
        assert_eq!(Aggregator::merge_ask(
            vec![Level{exchange: "a".to_string(), price: 2.0_f64, amount: 1_f64}, Level{exchange: "a".to_string(), price: 2.1_f64, amount: 1_f64}],
            &vec![Level{exchange: "a".to_string(), price: 0.9_f64, amount: 1_f64},
                Level{exchange: "b".to_string(), price: 1.0_f64, amount: 1_f64},
                Level{exchange: "b".to_string(), price: 1.1_f64, amount: 1_f64}, 
                Level{exchange: "a".to_string(), price: 1.2_f64, amount: 1_f64}, 
                Level{exchange: "c".to_string(), price: 1.3_f64, amount: 1.1_f64},
                Level{exchange: "c".to_string(), price: 1.4_f64, amount: 1.1_f64}], "a", 6).unwrap(), 
            vec![Level{exchange: "b".to_string(), price: 1.0_f64, amount: 1_f64}, 
                Level{exchange: "b".to_string(), price: 1.1_f64, amount: 1_f64}, 
                Level{exchange: "c".to_string(), price: 1.3_f64, amount: 1.1_f64},
                Level{exchange: "c".to_string(), price: 1.4_f64, amount: 1.1_f64},
                Level{exchange: "a".to_string(), price: 2.0_f64, amount: 1_f64},
                Level{exchange: "a".to_string(), price: 2.1_f64, amount: 1_f64}]);      
    }
}