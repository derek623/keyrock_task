use crate::{orderbook::{Summary, orderbook_aggregator_server::{OrderbookAggregator}, Empty}};
use tokio::sync::{mpsc, Mutex};
use tonic::{Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;
use std::sync::Arc;
use crate::multi_receiver_channels::MultiReceiverChannel;

const GRPC_BUFFER_SIZE: usize = 1000;

pub struct OrderBookAggregatorService {
    mpc: Arc<Mutex<MultiReceiverChannel<Summary>>>,
}

impl OrderBookAggregatorService {
    pub fn new(mpc: Arc<Mutex<MultiReceiverChannel<Summary>>>) -> OrderBookAggregatorService {
        OrderBookAggregatorService { mpc }
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
        
        tokio::spawn( async move {
            while let Some(msg) = mpc_rx.recv().await {                    
                    match tx.send(Ok(msg)).await {
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