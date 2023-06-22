//mod console_renderer;
pub mod orderbook {
    tonic::include_proto!("orderbook");
}

use orderbook::{orderbook_aggregator_client, Empty};
use tokio_stream::StreamExt;

const GRPC_SERVER_URL: &str = "http://[::1]:30253";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client =
        orderbook_aggregator_client::OrderbookAggregatorClient::connect(GRPC_SERVER_URL).await?;

    let mut stream = client.book_summary(Empty {}).await?.into_inner();

    while let Some(summary) = stream.next().await {
        if let Ok(summary) = summary {
            println!("GRPC client got: {:?}", summary);
        }
    }
    Ok(())
}
