mod bitstamp;
mod binance;
mod marketdatasource;
mod aggregator;
mod utility;
mod aggregator_grpc_server;

pub mod orderbook {
    tonic::include_proto!("orderbook");
}

use tokio::{io::Result, sync::mpsc, sync::mpsc::Receiver, sync::mpsc::Sender};
use bitstamp::Bitstamp;
use binance::Binance;
use marketdatasource::{MarketDataSource, OrderBookSnap};
use aggregator::Aggregator;
use std::env;

const CHANNEL_SIZE: usize = 10000;
const DEFAULT_DEPTH: usize = 10;

#[tokio::main]
pub async fn main() -> Result<()> {

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Missing arguments: Usage: server <currency> <depth>");
        return Ok(());
    };

    //let (tx: Sender<order_book_snap::OrderBookSnap>, mut rx: Receiver<order_book_snap::OrderBookSnap>) = mpsc::channel(CHANNEL_SIZE);
    let (tx, rx) : (Sender<OrderBookSnap>, Receiver<OrderBookSnap>) = mpsc::channel(CHANNEL_SIZE);
    let tx2 = tx.clone();

    let currency = &args[1];
    let bitstamp = Bitstamp::new("wss://ws.bitstamp.net", &currency, DEFAULT_DEPTH, tx, "bitstamp");
    let bitstamp_stream = tokio::spawn( async move {
        bitstamp.run().await;
    });

    let binance = Binance::new("wss://stream.binance.com:9443/ws/", &currency, DEFAULT_DEPTH, tx2, "binance"); //ethbtc@depth10@100ms
    let binance_stream = tokio::spawn( async move {
        binance.run().await;
    });

    let mut aggregator = Aggregator::new(rx, 2 * DEFAULT_DEPTH); //The aggretor only needs to keep 2 * depth of bid/ask not matter how many sources we have
    let aggregator_stream = tokio::spawn( async move {
        aggregator.run().await;
    });

    bitstamp_stream.await?;
    binance_stream.await?;
    aggregator_stream.await?;

    /*tokio::select! {
        _ = bitstamp_stream => {},
        _ = binance_stream => {}
        _ = aggregator_stream => {},
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