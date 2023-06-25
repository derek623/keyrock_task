use crate::{orderbook::{Summary, orderbook_aggregator_server::{OrderbookAggregator}, Empty}, aggregator::AggregatedOrderBook};
use tokio::sync::mpsc;
use tonic::{Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;
use tokio::sync::Mutex;
use std::sync::Arc;
use crate::multi_receiver_channels::MultiReceiverChannel;

const GRPC_BUFFER_SIZE: usize = 1000;

pub struct OrderBookAggregatorService {
    mpc: Arc<Mutex<MultiReceiverChannel<AggregatedOrderBook>>>,
    depth: usize,
}

impl OrderBookAggregatorService {
    pub fn new(mpc: Arc<Mutex<MultiReceiverChannel<AggregatedOrderBook>>>, depth: usize) -> OrderBookAggregatorService {
        OrderBookAggregatorService { mpc, depth }
    }
}

#[tonic::async_trait]
impl OrderbookAggregator for OrderBookAggregatorService {

    type BookSummaryStream = ReceiverStream<Result<Summary, Status>>;

    async fn book_summary(&self, _request: Request<Empty>) -> Result<Response<Self::BookSummaryStream>, Status> { 
        let (tx, rx) = mpsc::channel(100);
        let mpc = self.mpc.clone();
        //todo!() 
        let mut mpc = mpc.lock().await;
        let mut mpc_rx = mpc.create_receiver(GRPC_BUFFER_SIZE);
        let depth = self.depth;
        tokio::spawn( async move {
            while let Some(msg) = mpc_rx.recv().await {
                    let summary = Summary {
                        spread: msg.spread,
                        bids: msg.order_book.bids[0..depth].to_vec(),
                        asks: msg.order_book.asks[0..depth].to_vec(),
                    };
                    
                    match tx.send(Ok(summary)).await {
                        Ok(_) => {},
                        Err(e) => { 
                            log::error!("Fail to send summary: {:?}", e.to_string()); 
                            drop(mpc_rx);
                            break;
                        }
                    }
                }
            });
                
        

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}