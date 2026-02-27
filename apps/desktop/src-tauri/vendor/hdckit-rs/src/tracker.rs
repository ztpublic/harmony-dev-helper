use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tokio::time::sleep;

use crate::connection::Connection;
use crate::error::HdcError;
use crate::parsers::read_targets;

#[derive(Debug)]
pub enum TargetEvent {
    Added(String),
    Removed(String),
    Error(HdcError),
}

#[derive(Debug)]
pub struct TargetTracker {
    receiver: mpsc::UnboundedReceiver<TargetEvent>,
    stop_sender: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl TargetTracker {
    pub(crate) fn new(mut connection: Connection) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let (stop_tx, mut stop_rx) = oneshot::channel();

        tokio::spawn(async move {
            let mut target_list: Vec<String> = Vec::new();

            loop {
                let result = poll_targets(&mut connection).await;

                match result {
                    Ok(new_list) => {
                        for target in &new_list {
                            if !target_list.contains(target) {
                                let _ = tx.send(TargetEvent::Added(target.clone()));
                            }
                        }
                        for target in &target_list {
                            if !new_list.contains(target) {
                                let _ = tx.send(TargetEvent::Removed(target.clone()));
                            }
                        }

                        target_list = new_list;
                    }
                    Err(err) => {
                        let _ = tx.send(TargetEvent::Error(err));
                        break;
                    }
                }

                tokio::select! {
                    _ = sleep(Duration::from_secs(1)) => {}
                    _ = &mut stop_rx => {
                        break;
                    }
                }
            }

            let _ = connection.end().await;
        });

        Self {
            receiver: rx,
            stop_sender: Arc::new(Mutex::new(Some(stop_tx))),
        }
    }

    pub async fn next_event(&mut self) -> Option<TargetEvent> {
        self.receiver.recv().await
    }

    pub fn end(&self) {
        if let Ok(mut sender) = self.stop_sender.lock() {
            if let Some(stop_tx) = sender.take() {
                let _ = stop_tx.send(());
            }
        }
    }
}

impl Drop for TargetTracker {
    fn drop(&mut self) {
        self.end();
    }
}

async fn poll_targets(connection: &mut Connection) -> Result<Vec<String>, HdcError> {
    connection.send(b"list targets").await?;
    let data = connection.read_value().await?;
    Ok(read_targets(&String::from_utf8_lossy(&data)))
}
