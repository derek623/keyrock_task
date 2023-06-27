//mod console_renderer;
#[macro_use] extern crate prettytable;
use prettytable::Table;
use std::io::{Write, StdoutLock};
use orderbook::Summary;
use std::env;

pub mod orderbook {
    tonic::include_proto!("orderbook");
}

use orderbook::{orderbook_aggregator_client, Empty};
use tokio_stream::StreamExt;

const GRPC_SERVER_URL: &str = "http://[::1]:";
const GRPC_SERVER_DEFAULT_PORT: usize = 30253;

fn clear(lock: &mut StdoutLock) {
    //let _ = write!(lock, "{esc}c", esc = 27 as char);
    let _ = write!(lock, "{esc}[2J{esc}[1;1H", esc = 27 as char);
}

fn print_summary(lock: &mut StdoutLock, summary: &Summary) {
    let spread_table = table!(["Spread", summary.spread]);
    let _ = writeln!(lock, "{}", spread_table.to_string());

    let mut bid_ask_table = Table::new();
    
    for n in summary.asks.iter().rev() {
        bid_ask_table.add_row(row!["", "", "", n.price, n.amount, n.exchange]);
    }
    for n in summary.bids.iter() {
        bid_ask_table.add_row(row![n.exchange, n.amount, n.price, "", "", ""]);
    }
    
    let _ = writeln!(lock, "{}", bid_ask_table.to_string());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let grpc_port = match args.len() {
        1 => { GRPC_SERVER_DEFAULT_PORT },
        _ => { args[1].parse::<usize>().expect("Cannot get the grpc port from command line argument") }
    };
    
    let mut grpc_url = GRPC_SERVER_URL.to_owned();
    grpc_url.push_str(&grpc_port.to_string());

    let mut client =
        orderbook_aggregator_client::OrderbookAggregatorClient::connect(grpc_url).await?;

    let mut stream = client.book_summary(Empty {}).await?.into_inner();

    while let Some(summary) = stream.next().await {
        if let Ok(summary) = summary {
            //println!("GRPC client got: {:?}", summary);
            //print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
            let stdout = std::io::stdout();
            let mut lock = stdout.lock();
            clear(&mut lock);
            print_summary(&mut lock, &summary);
        }
    }
    Ok(())
}
