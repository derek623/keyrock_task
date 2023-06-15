use async_trait::async_trait;
use crate::order_book_snap::OrderBookSnap;

#[derive(Debug)]
pub enum Exchanges {
    BINANCE,
    BITSTAMP,
}

#[async_trait]
pub trait MarketDataSource {
    async fn run(&self);
    fn normalize(&self, msg: &str) -> Result<OrderBookSnap, ()>;
}