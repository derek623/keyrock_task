#[derive(Debug)]
pub struct Level {
    pub exchange: String,
    pub price: f32,
    pub amount: f32,
}

#[derive(Debug)]
pub struct OrderBookSnap<const DEPTH: usize> {
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
}

impl<const DEPTH: usize> OrderBookSnap<DEPTH> {
    pub fn new() -> OrderBookSnap<DEPTH>{
        let bids = Vec::with_capacity(DEPTH);
        let asks = Vec::with_capacity(DEPTH);
        OrderBookSnap{ bids, asks}
    }
    pub fn addBid(&mut self, level: Level)
    {
        self.bids.push(level);
    }
    pub fn addAsk(&mut self, level: Level)
    {
        self.asks.push(level);
    }
}