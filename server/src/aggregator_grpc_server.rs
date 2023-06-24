use crate::{orderbook::{Summary, orderbook_aggregator_server::{OrderbookAggregator, OrderbookAggregatorServer}, Empty}, aggregator::AggregatedOrderBook};
use tokio::{sync::mpsc, sync::mpsc::Receiver};
use tonic::{Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;
use tokio::sync::Mutex;
use std::sync::Arc;

pub struct OrderBookAggregatorService {
    aob_rx_rc: Arc<Mutex<Receiver<AggregatedOrderBook>>>,
    depth: usize,
}

impl OrderBookAggregatorService {
    pub fn new(aob_rx_rc: Arc<Mutex<Receiver<AggregatedOrderBook>>>, depth: usize) -> OrderBookAggregatorService {
        OrderBookAggregatorService { aob_rx_rc, depth }
    }
}

#[tonic::async_trait]
impl OrderbookAggregator for OrderBookAggregatorService {

    type BookSummaryStream = ReceiverStream<Result<Summary, Status>>;

    async fn book_summary(&self, _request: Request<Empty>) -> Result<Response<Self::BookSummaryStream>, Status> { 
        let (tx, rx) = mpsc::channel(4);
        let aob_rx_rc = self.aob_rx_rc.clone();
        //todo!() 
        let depth = self.depth;
        tokio::spawn( async move {
            let mut aob_rx = aob_rx_rc.lock().await;
                while let Some(msg) = aob_rx.recv().await {
                    //log::info!("OrderBook_Aggregator_Service got {:?}", msg);
                        let summary = Summary {
                            spread: msg.spread,
                            bids: msg.order_book.bids[0..depth].to_vec(),
                            asks: msg.order_book.asks[0..depth].to_vec(),
                        };
                        match tx.send(Ok(summary)).await {
                            Ok(_) => {},
                            Err(e) => { 
                                log::error!("Fail to send summary: {:?}", e.to_string()); 
                                break;
                            }
                        }
                    }
                });
                
        

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}