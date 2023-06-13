mod bitstamp;
mod binance;
mod marketdatasource;
mod order_book_snap;

use tokio::io::Result;
use bitstamp::Bitstamp;
use binance::Binance;
use marketdatasource::MarketDataSource;

#[tokio::main]
pub async fn main() -> Result<()> {

    let mut bitstamp = Bitstamp::new("wss://ws.bitstamp.net", "ethbtc");
    let bitstamp_stream = tokio::spawn( async move {
        bitstamp.run().await;
    });

    let mut binance = Binance::new("wss://stream.binance.com:9443/ws/", "ethbtc"); //ethbtc@depth10@100ms
    let binance_stream = tokio::spawn( async move {
        binance.run().await;
    });

    bitstamp_stream.await?;
    binance_stream.await?;

    /*tokio::select! {
        _ = bitstamp_stream => {},
        _ = binance_stream => {}
    };*/
    //bitstamp_stream.await?;
    /*let bitstamp = Bitstamp::new("wss://ws.bitstamp.net");
    let binance = Binance::new("wss://stream.binance.com:9443/ws/ethbtc@depth20@100ms");

    let sources: Vec<Box<dyn MarketDataSource>> = Vec::new();
    sources.push(Box::new(bitstamp));
    sources.push(Box::new(binance));
    for source in sources {
        let stream = tokio::spawn( async move {
            source.run().await;
        });
        stream.await?;
    }*/


    Ok(())
}