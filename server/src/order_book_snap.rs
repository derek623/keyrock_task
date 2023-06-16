use crate::marketdatasource::{MarketDataSource, Exchanges};

#[derive(Debug)]
pub struct Level {
    pub exchange: Exchanges,
    pub price: f32,
    pub amount: f32,
}

#[derive(Debug)]
pub struct OrderBookSnap {
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
    exchange: Exchanges,
    currency: String,
}

impl OrderBookSnap {
    pub fn new(exchange: Exchanges, depth: usize, currency: &str) -> OrderBookSnap{
        let bids = Vec::with_capacity(depth);
        let asks = Vec::with_capacity(depth);
        OrderBookSnap{ bids, asks, exchange, currency: currency.to_string() }
    }
    pub fn add_bid(&mut self, level: Level)
    {
        self.bids.push(level);
    }
    pub fn add_ask(&mut self, level: Level)
    {
        self.asks.push(level);
    }

    pub fn get_exchange(&self) -> &Exchanges {
        &self.exchange
    }

    pub fn get_currency(&self) -> &str {
        &self.currency
    }
}