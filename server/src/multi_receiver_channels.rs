use tokio::sync::mpsc::{self, Receiver, Sender};

#[derive(Debug)]
pub struct MultiReceiverChannel<T: Clone> {
    senders: Vec<Sender<T>>,
}

impl<T: Clone> MultiReceiverChannel<T> {
    pub fn new() -> MultiReceiverChannel<T> {
        MultiReceiverChannel {
            senders: Vec::<Sender<T>>::new(),
        }
    }

    pub async fn send(&mut self, t: T)
    where
    T: Clone 
    {
        if self.senders.is_empty() {
            return;
        }

        let mut i = 0 as usize;
        while i < self.senders.len() {
            let sender = &self.senders[i];
            match sender.send(t.clone()).await {
                Ok(_) => { i += 1; },
                Err(e) => { 
                    log::error!("MultiReceiverChannel failed to send {:?}", e.to_string()); 
                    self.senders.swap_remove(i);
                } 
            }
        };
    }

    pub fn create_receiver(&mut self, buffer: usize) -> Receiver<T> {
        let (tx, rx) = mpsc::channel(buffer);
        self.senders.push(tx);
        rx
    }
}
