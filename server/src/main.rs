mod bitstamp;
mod binance;
mod marketdatasource;
mod aggregator;
mod utility;
mod aggregator_grpc_server;
mod multi_receiver_channels;

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
use fast_log::config::Config;
use fast_log::plugin::file_split::RollingType;
use fast_log::consts::LogSize;
use fast_log::plugin::packer::LogPacker;
use log::LevelFilter;
use multi_receiver_channels::MultiReceiverChannel;

const GRPC_SERVER_URL: &str = "[::1]:30253";

const CHANNEL_SIZE: usize = 10000;
const DEFAULT_DEPTH: usize = 10;

#[tokio::main]
pub async fn main() -> Result<()> {

    fast_log::init(Config::new().level(LevelFilter::Info).chan_len(Some(100000)).file_split(
        "target/logs/",
        LogSize::MB(50),
        RollingType::All,
        LogPacker {},
    )).expect("Fail to start logger");

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Missing arguments: Usage: server <currency> <depth>");
        return Ok(());
    };

    let (ob_tx, ob_rx) : (Sender<OrderBookSnap>, Receiver<OrderBookSnap>) = mpsc::channel(CHANNEL_SIZE);
    let ob_tx2 = ob_tx.clone();

    let currency = &args[1];
    let depth = match args.len() {
        2 => DEFAULT_DEPTH,
        _ => { args[2].parse::<usize>().expect("Cannot get the depth from command line argument") }
    };

    let bitstamp = Bitstamp::new("wss://ws.bitstamp.net", &currency, depth, ob_tx, "bitstamp");
    let bitstamp_stream = tokio::spawn( async move {
        bitstamp.run().await;
    });

    let binance = Binance::new("wss://stream.binance.com:9443/ws/", &currency, depth, ob_tx2, "binance"); //ethbtc@depth10@100ms
    let binance_stream = tokio::spawn( async move {
        binance.run().await;
    });

    let mrc = Arc::new(Mutex::new(MultiReceiverChannel::<AggregatedOrderBook>::new()));
    let mut aggregator = Aggregator::new(ob_rx, mrc.clone(), 2 * depth);
    let aggregator_stream = tokio::spawn( async move {
        aggregator.run().await;
    });
    
    let server: OrderBookAggregatorService = OrderBookAggregatorService::new(mrc.clone(), depth);
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

    Ok(())
}