use crate::marketdatasource::{Exchanges, OrderBook};

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

    pub fn get_exchange(&self) -> &Exchanges {
        &self.exchange
    }
}