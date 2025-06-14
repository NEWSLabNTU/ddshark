use crate::{
    message::UpdateEvent,
    metrics::Metrics,
    state::State,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::timeout;
use tracing::warn;

const BATCH_SIZE: usize = 64;
const BATCH_TIMEOUT: Duration = Duration::from_millis(10);

pub struct BatchProcessor {
    rx: flume::Receiver<UpdateEvent>,
    state: Arc<Mutex<State>>,
    metrics: Metrics,
    batch: Vec<UpdateEvent>,
}

impl BatchProcessor {
    pub fn new(
        rx: flume::Receiver<UpdateEvent>,
        state: Arc<Mutex<State>>,
        metrics: Metrics,
    ) -> Self {
        Self {
            rx,
            state,
            metrics,
            batch: Vec::with_capacity(BATCH_SIZE),
        }
    }

    /// Collect messages into a batch, with timeout
    pub async fn collect_batch(&mut self) -> bool {
        self.batch.clear();
        
        // Try to get first message (blocking)
        match self.rx.recv_async().await {
            Ok(msg) => {
                self.batch.push(msg);
                self.metrics.message_sent();
            }
            Err(_) => return false, // Channel closed
        }

        // Try to collect more messages non-blocking
        let deadline = tokio::time::Instant::now() + BATCH_TIMEOUT;
        
        while self.batch.len() < BATCH_SIZE {
            match timeout(deadline.saturating_duration_since(tokio::time::Instant::now()), 
                         self.rx.recv_async()).await {
                Ok(Ok(msg)) => {
                    self.batch.push(msg);
                    self.metrics.message_sent();
                }
                Ok(Err(_)) => break, // Channel closed
                Err(_) => break, // Timeout
            }
        }

        // Update metrics
        self.metrics.update_queue_depth(self.rx.len());
        
        true
    }

    /// Process the collected batch
    pub fn process_batch<F>(&mut self, mut processor: F) -> Result<(), anyhow::Error>
    where
        F: FnMut(&mut State, &UpdateEvent) -> Result<(), anyhow::Error>,
    {
        if self.batch.is_empty() {
            return Ok(());
        }

        // Acquire lock once for entire batch
        let mut state = self.state.lock().unwrap();
        
        for event in &self.batch {
            if let Err(e) = processor(&mut state, event) {
                warn!("Error processing event: {}", e);
            }
            self.metrics.message_processed();
        }

        Ok(())
    }
}