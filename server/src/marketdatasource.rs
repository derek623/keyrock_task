use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Default)]
pub enum Exchanges {
    BINANCE,
    BITSTAMP,
    #[default] UNKNOWN,
}

#[derive(Debug, Clone, Default)]
pub struct Level {
    pub exchange: Exchanges,
    pub price: f64,
    pub amount: f64,
}

#[derive(Debug, Default)]
pub struct OrderBook {
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
    pub currency: String,
}

impl OrderBook {
    pub fn new(bids: Vec<Level>, asks: Vec<Level>, currency: &str) -> OrderBook{
        OrderBook {bids, asks, currency: currency.to_string()}
    }

    pub fn add_bid(&mut self, level: Level)
    {
        self.bids.push(level);
    }
    pub fn add_ask(&mut self, level: Level)
    {
        self.asks.push(level);
    }

}

#[derive(Debug)]
pub struct OrderBookSnap {
    pub exchange: Exchanges,
    pub order_book: OrderBook,
}

impl OrderBookSnap {
    pub fn new(exchange: Exchanges, depth: usize, currency: &str) -> OrderBookSnap{
        OrderBookSnap{ exchange, 
            order_book: OrderBook::new (
                Vec::with_capacity(depth), 
                Vec::with_capacity(depth), 
                currency, 
            )
        }
    }
}

pub struct MarketDataSourceInfo {
    pub address: String,
    pub currency: String,
    pub depth: usize,
    pub sender: Sender<OrderBookSnap>,
    pub name: String,
}

#[async_trait]
pub trait MarketDataSource {
    async fn run(&self);
    fn normalize(&self, msg: &str) -> Result<OrderBookSnap, ()>;
}