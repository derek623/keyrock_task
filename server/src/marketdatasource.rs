use async_trait::async_trait;
use crate::order_book_snap::OrderBookSnap;
use tokio::sync::mpsc::Sender;

#[derive(Debug)]
pub enum Exchanges {
    BINANCE,
    BITSTAMP,
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