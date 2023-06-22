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
use aggregator::{Aggregator, AggregatedOrderBook};
use aggregator_grpc_server::OrderBookAggregatorService;
use tonic::transport::Server;
use tokio::sync::Mutex;
use std::{net::ToSocketAddrs, sync::Arc};
use std::env;

const GRPC_SERVER_URL: &str = "[::1]:30253";

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
    let (ob_tx, ob_rx) : (Sender<OrderBookSnap>, Receiver<OrderBookSnap>) = mpsc::channel(CHANNEL_SIZE);
    let ob_tx2 = ob_tx.clone();

    let currency = &args[1];
    let bitstamp = Bitstamp::new("wss://ws.bitstamp.net", &currency, DEFAULT_DEPTH, ob_tx, "bitstamp");
    let bitstamp_stream = tokio::spawn( async move {
        bitstamp.run().await;
    });

    let binance = Binance::new("wss://stream.binance.com:9443/ws/", &currency, DEFAULT_DEPTH, ob_tx2, "binance"); //ethbtc@depth10@100ms
    let binance_stream = tokio::spawn( async move {
        binance.run().await;
    });

    let (ag_ob_tx, ag_ob_rx) : (Sender<AggregatedOrderBook>, Receiver<AggregatedOrderBook>) = mpsc::channel(CHANNEL_SIZE);
    let mut aggregator = Aggregator::new(ob_rx, ag_ob_tx, DEFAULT_DEPTH);
    let aggregator_stream = tokio::spawn( async move {
        aggregator.run().await;
    });
    
    let ag_ob_rx = Arc::new(Mutex::new(ag_ob_rx));
    let server: OrderBookAggregatorService = OrderBookAggregatorService::new(ag_ob_rx.clone(), DEFAULT_DEPTH);
    let order_aggregator_server = Server::builder()
        .add_service(orderbook::orderbook_aggregator_server::OrderbookAggregatorServer::new(server))
        .serve(GRPC_SERVER_URL.to_socket_addrs().unwrap().next().unwrap());

    /*bitstamp_stream.await?;
    binance_stream.await?;
    aggregator_stream.await?;
    orderAggServer.await?;*/

    tokio::select! {
        _ = bitstamp_stream => {},
        _ = binance_stream => {}
        _ = aggregator_stream => {},
        _ = order_aggregator_server => {}
    };
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