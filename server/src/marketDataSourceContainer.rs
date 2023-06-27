use crate::marketdatasource::MarketDataSource;

pub struct MarketDataSourceContainer {
    market_data_sources: Vec<Box<dyn MarketDataSource + Sync + Send>>,
}

impl MarketDataSourceContainer{
    pub fn new() -> MarketDataSourceContainer{
        MarketDataSourceContainer { market_data_sources: Vec::new() }
    }

    pub fn add (&mut self, market_data_source: Box<dyn MarketDataSource + Sync + Send>) {
        self.market_data_sources.push(market_data_source);
    }

    pub async fn wait_resources(self){
        let mut v = Vec::new();
        for n in self.market_data_sources {
            v.push(tokio::spawn( async move {
                n.run().await;
            }))
        };
    }
}
