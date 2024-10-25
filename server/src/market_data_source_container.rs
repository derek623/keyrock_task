use crate::market_data_source::MarketDataSource;
use crate::OrderBookSnap;
use crate::{Binance, Bitstamp};
use enum_dispatch::enum_dispatch;

pub struct MarketDataSourceContainer {
    market_data_sources: Vec<MarketSources>,
}

//enum dispatch is a macro for generating the code necessary for
//static dispatch using enum. It generates the code for all the variants
//https://docs.rs/enum_dispatch/latest/enum_dispatch/
#[enum_dispatch(MarketDataSource)]
#[derive(Clone)]
pub enum MarketSources {
    Binance(Binance),
    Bitstamp(Bitstamp),
}

impl MarketDataSourceContainer {
    pub fn new() -> MarketDataSourceContainer {
        MarketDataSourceContainer {
            market_data_sources: Vec::new(),
        }
    }

    //Add a market data source to the contaier
    pub fn add(&mut self, market_data_source: MarketSources) {
        self.market_data_sources.push(market_data_source);
    }

    //Start each market data source
    pub fn wait_resources(&self) {
        for n in self.market_data_sources.clone() {
            tokio::spawn(async move {
                n.run().await;
            });
        }
    }
}

//example macro, but not that useful for now
#[macro_export]
macro_rules! start_exchange {
    ($source:ident) => {
        match $source {
            MarketSources::Binance(b) => Self::start(b).await,
            MarketSources::Bitstamp(b) => Self::start(b).await,
        }
    };
}
