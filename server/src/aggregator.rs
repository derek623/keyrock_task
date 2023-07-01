use tokio::sync::mpsc::Receiver;
use crate::{market_data_source::*, orderbook};
use crate::orderbook::Summary;
use crate::{orderbook::Level, multi_receiver_channels::MultiReceiverChannel};
use std::cmp::Ordering;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::DEFAULT_DEPTH;
use arrayvec::ArrayVec;

struct MergeEntry<'a> {
    level: &'a MarketDatSourceLevel,
    exchange: Exchange
}
pub struct Aggregator {
    rx: Receiver<OrderBookSnap>,
    mpc: Arc<Mutex<MultiReceiverChannel<Summary>>>,
    exchange_orderbook_array: [OrderBook; Exchange::VARIANT_COUNT]
}

impl Aggregator {
    pub fn new (rx: Receiver<OrderBookSnap> ,mpc: Arc<Mutex<MultiReceiverChannel<Summary>>>) -> Aggregator {
        Aggregator { rx, mpc, exchange_orderbook_array: Default::default() } 
    }

    //This merge algorithm assumes that each exchange's bids are in the correct order and has the same depth
    fn merge_bid(exchange_orderbook_array: &[OrderBook; Exchange::VARIANT_COUNT]) -> Result<ArrayVec<Level, DEFAULT_DEPTH>, String> {
        //declare a vec with size = number of exchanges
        let mut value_vec = Vec::<MergeEntry>::with_capacity(Exchange::VARIANT_COUNT);
        // A vector that keeps track of the index of the next element in each exchange        
        let mut exchange_index_vec: [usize; Exchange::VARIANT_COUNT] = [0; Exchange::VARIANT_COUNT];
        //The result arrayVec
        let mut result = ArrayVec::<Level, DEFAULT_DEPTH>::new();
        
        //Initially put the first entry of each exchange into the array
        for (index, book) in exchange_orderbook_array.iter().enumerate() {
            if !book.bids.is_empty() {
                match num::FromPrimitive::from_usize(index) {
                    Some(exchange) => {
                        value_vec.push(MergeEntry{level: &book.bids[0], exchange});
                        //Increment the index as the first element has been pushed into the value_vec
                        exchange_index_vec[index] += 1;
                    },
                    None => { return Err("Cannot get exchange enum from index".to_string()); }
                }
            }
        }        
        
        //Sort, take the first entry, and then pust the next one into the vector. Repeat until result is full
        while !result.is_full() {
            //sort the vector
            value_vec.sort_unstable_by(|l1, l2| {
                if l2.level.price > l1.level.price || (l2.level.price == l1.level.price && l2.level.amount > l1.level.amount){
                    Ordering::Greater
                } else {
                    Ordering::Less
                }});
            //get the first item and push to result
            let first_item = value_vec.swap_remove(0);        
            result.push(Level {price: first_item.level.price, amount: first_item.level.amount, exchange: first_item.exchange.to_string()});
            //push the next entry from the respective exchange into value_vec.
            let first_item_exchange_index = first_item.exchange as usize;
            if exchange_index_vec[first_item_exchange_index] >= exchange_orderbook_array[first_item_exchange_index].bids.len() {
                break;
            }

            let next_item = MergeEntry { level: &exchange_orderbook_array[first_item_exchange_index].bids[exchange_index_vec[first_item_exchange_index]],
                exchange: first_item.exchange };
            value_vec.push(next_item);
            //update the index of the respective exchange
            exchange_index_vec[first_item_exchange_index] += 1;
        }
        
        Ok(result)
    }

    //This merge algorithm assumes that each exchange's asks are in the correct order and has the same depth
    fn merge_ask(exchange_orderbook_array: &[OrderBook; Exchange::VARIANT_COUNT]) -> Result<ArrayVec<Level, DEFAULT_DEPTH>, String> {
        //declare a vec with size = number of exchanges
        let mut value_vec = Vec::<MergeEntry>::with_capacity(Exchange::VARIANT_COUNT);
        // A vector that keeps track of the index of the next element in each exchange        
        let mut exchange_index_vec: [usize; Exchange::VARIANT_COUNT] = [0; Exchange::VARIANT_COUNT];
        //The result arrayVec
        let mut result = ArrayVec::<Level, DEFAULT_DEPTH>::new();

        //Initially put the first entry of each exchange into the array
        for (index, book) in exchange_orderbook_array.iter().enumerate() {
            if !book.asks.is_empty() {
                match num::FromPrimitive::from_usize(index) {
                    Some(exchange) => {
                        value_vec.push(MergeEntry{level: &book.asks[0], exchange});
                        //Increment the index as the first element has been pushed into the value_vec
                        exchange_index_vec[index] += 1;
                    },
                    None => { return Err("Cannot get exchange enum from index".to_string()); }
                }
            }
        }        
        
        //Sort, take the first entry, and then pust the next one into the vector. Repeat until result is full
        while !result.is_full() {
            //sort the vector
            value_vec.sort_unstable_by(|l1, l2| {
                if l1.level.price < l2.level.price || (l2.level.price == l1.level.price && l1.level.amount > l2.level.amount){
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            });
            //get the first item and push to result
            let first_item = value_vec.swap_remove(0);        
            result.push(Level {price: first_item.level.price, amount: first_item.level.amount, exchange: first_item.exchange.to_string()});
            //push the next entry from the respective exchange into value_vec.
            let first_item_exchange_index = first_item.exchange as usize;
            if exchange_index_vec[first_item_exchange_index] >= exchange_orderbook_array[first_item_exchange_index].asks.len() {
                break;
            }

            let next_item = MergeEntry { level: &exchange_orderbook_array[first_item_exchange_index].asks[exchange_index_vec[first_item_exchange_index]],
                exchange: first_item.exchange };
            value_vec.push(next_item);
            //update the index of the respective exchange
            exchange_index_vec[first_item_exchange_index] += 1;
        }

        Ok(result)
    }

    fn merge(&mut self, order_book_snap: OrderBookSnap) -> Result<(Vec<Level>, Vec<Level>), String> {
        //First update the exchange image in the exchange_orderbook_array        
        let order_book = &mut self.exchange_orderbook_array[order_book_snap.exchange as usize];        
        order_book.bids = order_book_snap.order_book.bids;        
        order_book.asks = order_book_snap.order_book.asks;        
                
        //let mut result = OrderBook::new();
        let bids = match Aggregator::merge_bid(&self.exchange_orderbook_array) {
            Ok(bids) => bids.to_vec(),
            Err(s) => { return Err(s); } 
        };
        
        let asks = match Aggregator::merge_ask(&self.exchange_orderbook_array) {
            Ok(asks) => asks.to_vec(),
            Err(s) => { return Err(s); }
        };
        Ok((bids, asks))
    }

    pub fn merge_and_gen_summary(&mut self, order_book_snap: OrderBookSnap) -> Result<Summary, String> {        
        let bid_ask_depth = match self.merge(order_book_snap) {
            Ok(depth) => depth,
            Err(e) => { return Err(e); },
        };
        let spread = bid_ask_depth.1[0].price - bid_ask_depth.0[0].price;        
        let summary = Summary{spread, bids: bid_ask_depth.0, asks: bid_ask_depth.1};
        log::info!("{:?}", summary);
        Ok(summary)
    }

    pub async fn run(&mut self) {
        while let Some(msg) = self.rx.recv().await {
            match self.merge_and_gen_summary(msg) {
                Ok(summary) => { 
                    let mut mpc = self.mpc.lock().await;
                    mpc.send(summary).await;
                },
                Err(e) => { log::error!("Merging snapshot return error: {}", e); },
            }
        }
    }
}


mod test {
    use super::*;

    #[test]
    fn test_merge_bid() {         
        //fn merge_bid(exchange_orderbook_array: &[OrderBook; Exchange::VARIANT_COUNT]) -> Result<ArrayVec<Level, DEFAULT_DEPTH>, String>
        //error case
        /*let ob_1 = OrderBook::new();
        ob_1.add_bid(Level {price: 1.0, amount: 1.0, exchange: "a".to_string()});
        assert_eq!(if let Err(e) = Aggregator::merge_bid(vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}], &vec![], "", 0) {
            e
        } else { "".to_string() },
         "Depth is smaller then updated queue length, fail to merge!".to_string());

        //normal case
        assert_eq!(Aggregator::merge_bid(vec![], &vec![], "", 10).unwrap(), vec![]);*/
        /*assert_eq!(Aggregator::merge_bid(vec![Level{exchange: "a".to_string(), price: 1_f64, amount: 1_f64}], &vec![Level{exchange: "a".to_string(), 
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
                Level{exchange: "a".to_string(), price: 0.8_f64, amount: 1_f64}]);*/
    }
/* 
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
    }*/
}
