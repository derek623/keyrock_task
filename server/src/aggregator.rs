use tokio::sync::mpsc::Receiver;
use crate::market_data_source::*;
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
    fn merge_bid(exchange_orderbook_array: &[OrderBook]) -> Result<ArrayVec<Level, DEFAULT_DEPTH>, String> {
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
        
        if value_vec.is_empty() {
            return Ok(result);
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
            result.push(first_item.level.to_orderbook_level(first_item.exchange.to_string()));
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
    fn merge_ask(exchange_orderbook_array: &[OrderBook]) -> Result<ArrayVec<Level, DEFAULT_DEPTH>, String> {
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
        
        if value_vec.is_empty() {
            return Ok(result);
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
            result.push(first_item.level.to_orderbook_level(first_item.exchange.to_string()));
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

    const INVALID_SIZE: usize = Exchange::VARIANT_COUNT + 1;

    #[test]
    fn test_merge_bid_invalid_exchange_size_1() {         
        //Number of exchange is more than defined in the exchange enum, but empty orderbook for each exchange        
        let exchange_orderbook_array: [OrderBook; INVALID_SIZE] = std::array::from_fn(|_|             
            OrderBook::new()
        );        
        assert_eq!(Aggregator::merge_bid(&exchange_orderbook_array).expect("Error").len(), 0);
    }
    #[test]
    fn test_merge_bid_invalid_exchange_size_2() {         
        //Number of exchange is more than defined in the exchange enum, add one level to each exchange
        let mut exchange_orderbook_array: [OrderBook; INVALID_SIZE] = std::array::from_fn(|_|             
            OrderBook::new()
        );
        for n in &mut exchange_orderbook_array {
            n.bids.push(MarketDatSourceLevel {amount: 1.0, price: 1.0});
        }
        assert_eq!(if let Err(e) = Aggregator::merge_bid(&exchange_orderbook_array) {
            e
        } else { "".to_string() },
         "Cannot get exchange enum from index".to_string());
    }
    #[test]
    fn test_merge_bid_one_order_book() {
        //Only one exchange has order book, others are empty
        let mut exchange_orderbook_array: [OrderBook; Exchange::VARIANT_COUNT] = std::array::from_fn(|_|             
            OrderBook::new()
        );
        let index = Exchange::Binance as usize;
        //exchange_orderbook_array[index].bids.push(MarketDatSourceLevel {amount: 1.0, price: 1.0});
        let mut expected_result = ArrayVec::<Level, DEFAULT_DEPTH>::new();

        let mut best_price = 10.0;
        let step = 0.1;
        while !expected_result.is_full() {
            exchange_orderbook_array[index].bids.push(MarketDatSourceLevel {amount: 1.0, price: best_price});
            expected_result.push(Level{amount: 1.0, price: best_price, exchange: Exchange::Binance.to_string()});
            assert_eq!(Aggregator::merge_bid(&exchange_orderbook_array).expect("Error"), expected_result);
            best_price -= step;
        }
    }

    #[test]
    fn test_merge_bid_multiple_order_book_1() {
        //Same price but different amount, the entry with the higher amount will go first
        let mut exchange_orderbook_array: [OrderBook; Exchange::VARIANT_COUNT] = std::array::from_fn(|_|             
            OrderBook::new()
        );
        let binance_index = Exchange::Binance as usize;
        let bitstamp_index = Exchange::Bitstamp as usize;
        //exchange_orderbook_array[index].bids.push(MarketDatSourceLevel {amount: 1.0, price: 1.0});
        let mut expected_result = ArrayVec::<Level, DEFAULT_DEPTH>::new();

        let mut binance_best_price = 10.0;
        let mut bitstamp_best_price = 10.0;
        let step = 0.1;
        for _ in 0..DEFAULT_DEPTH {
            exchange_orderbook_array[binance_index].bids.push(MarketDatSourceLevel {amount: 2.0, price: binance_best_price});
            exchange_orderbook_array[bitstamp_index].bids.push(MarketDatSourceLevel {amount: 1.0, price: bitstamp_best_price});            
            binance_best_price -= step;
            bitstamp_best_price -= step;
        }

        let mut index = 0;
        while !expected_result.is_full() {
            expected_result.push(exchange_orderbook_array[binance_index].bids[index].to_orderbook_level(Exchange::Binance.to_string()));
            expected_result.push(exchange_orderbook_array[bitstamp_index].bids[index].to_orderbook_level(Exchange::Bitstamp.to_string()));
            index += 1;
        }
        
        assert_eq!(Aggregator::merge_bid(&exchange_orderbook_array).expect("Error"), expected_result);            
    }

    #[test]
    fn test_merge_bid_multiple_order_book_2() {
        //All with different price, higher price go first
        let mut exchange_orderbook_array: [OrderBook; Exchange::VARIANT_COUNT] = std::array::from_fn(|_|             
            OrderBook::new()
        );
        let binance_index = Exchange::Binance as usize;
        let bitstamp_index = Exchange::Bitstamp as usize;
        //exchange_orderbook_array[index].bids.push(MarketDatSourceLevel {amount: 1.0, price: 1.0});
        let mut expected_result = ArrayVec::<Level, DEFAULT_DEPTH>::new();

        let mut binance_best_price = 21.0;
        let mut bitstamp_best_price = 20.0;
        let step = 2.0;
        for _ in 0..DEFAULT_DEPTH {
            exchange_orderbook_array[binance_index].bids.push(MarketDatSourceLevel {amount: 1.0, price: binance_best_price});
            exchange_orderbook_array[bitstamp_index].bids.push(MarketDatSourceLevel {amount: 1.0, price: bitstamp_best_price});            
            binance_best_price -= step;
            bitstamp_best_price -= step;
        }

        let mut index = 0;
        while !expected_result.is_full() {
            expected_result.push(exchange_orderbook_array[binance_index].bids[index].to_orderbook_level(Exchange::Binance.to_string()));
            expected_result.push(exchange_orderbook_array[bitstamp_index].bids[index].to_orderbook_level(Exchange::Bitstamp.to_string()));
            index += 1;
        }
        
        assert_eq!(Aggregator::merge_bid(&exchange_orderbook_array).expect("Error"), expected_result);            
    }    
}
