mod aggregator;
mod aggregator_grpc_server;
mod binance;
mod bitstamp;
mod market_data_source;
mod market_data_source_container;
mod multi_receiver_channels;

pub mod orderbook {
    tonic::include_proto!("orderbook");
}

use crate::market_data_source_container::MarketSources;
use aggregator::Aggregator;
use aggregator_grpc_server::OrderBookAggregatorService;
use binance::Binance;
use bitstamp::Bitstamp;
use fast_log::config::Config;
use fast_log::consts::LogSize;
use fast_log::plugin::file_split::RollingType;
use fast_log::plugin::packer::LogPacker;
use log::LevelFilter;
use market_data_source::{OrderBookSnap, DEFAULT_DEPTH};
use market_data_source_container::MarketDataSourceContainer;
use multi_receiver_channels::MultiReceiverChannel;
use orderbook::Summary;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::{sync::mpsc, sync::mpsc::Receiver, sync::mpsc::Sender};
use tonic::transport::Server;

const GRPC_SERVER_URL: &str = "[::1]:";
const CHANNEL_SIZE: usize = 10000;
const DEFAULT_CURRENCY: &str = "ethbtc";
const GRPC_SERVER_DEFAULT_PORT: usize = 30253;

#[tokio::main]
pub async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    fast_log::init(
        Config::new()
            .level(LevelFilter::Info)
            .chan_len(Some(100000))
            .file_split(
                "target/logs/server.log",
                LogSize::MB(50),
                RollingType::All,
                LogPacker {},
            ),
    )
    .expect("Fail to start logger");

    let args: Vec<String> = env::args().collect();

    let (ob_tx, ob_rx): (Sender<OrderBookSnap>, Receiver<OrderBookSnap>) =
        mpsc::channel(CHANNEL_SIZE);
    let ob_tx2 = ob_tx.clone();

    let currency = match args.len() {
        1 => DEFAULT_CURRENCY,
        _ => &args[1],
    };

    let grpc_port = match args.len() {
        1 | 2 => GRPC_SERVER_DEFAULT_PORT,
        _ => args[2]
            .parse::<usize>()
            .expect("Cannot get the grpc port from command line argument"),
    };

    let mut grpc_url = GRPC_SERVER_URL.to_owned();
    grpc_url.push_str(&grpc_port.to_string());

    let bitstamp = Bitstamp::new("wss://ws.bitstamp.net", &currency, ob_tx, "bitstamp");
    let binance = Binance::new(
        "wss://stream.binance.com:9443/stream?streams=",
        &currency,
        ob_tx2,
        "binance",
    ); //ethbtc@depth10@100ms
    let mut mds_container = MarketDataSourceContainer::new();
    mds_container.add(MarketSources::Bitstamp(bitstamp));
    mds_container.add(MarketSources::Binance(binance));
    mds_container.wait_resources();

    let mrc = Arc::new(Mutex::new(MultiReceiverChannel::<Summary>::new()));
    let mut aggregator = Aggregator::new(ob_rx, mrc.clone());
    let aggregator_stream = tokio::spawn(async move {
        aggregator.run().await;
    });

    let server: OrderBookAggregatorService = OrderBookAggregatorService::new(mrc.clone());

    Server::builder()
        .add_service(orderbook::orderbook_aggregator_server::OrderbookAggregatorServer::new(server))
        .serve(grpc_url.parse().expect("Invalid socket address"))
        .await?;

    aggregator_stream.await?;

    Ok(())
}
