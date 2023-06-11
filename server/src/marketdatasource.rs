use async_trait::async_trait;
use crate::order_book_snap::OrderBookSnap;

#[async_trait]
pub trait MarketDataSource {
    async fn run(&self);
    //fn normalize(&self, msg: &str) -> OrderBookSnap<10>;
    fn normalize(&self, msg: &str) -> Result<OrderBookSnap<10>, ()>;
    const MAX_DEPTH: usize = 10;
}