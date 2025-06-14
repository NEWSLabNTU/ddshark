use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use crossbeam_queue::ArrayQueue;
use crate::metrics::Metrics;

/// A high-performance ring buffer with overflow handling strategies
pub struct RingBuffer<T> {
    queue: Arc<ArrayQueue<T>>,
    metrics: Metrics,
    overflow_strategy: OverflowStrategy,
}

#[derive(Clone, Copy)]
pub enum OverflowStrategy {
    /// Drop new messages when full
    DropNewest,
    /// Drop oldest messages to make room
    DropOldest,
    /// Block until space is available (with timeout)
    Block(Duration),
}

impl<T> RingBuffer<T> {
    pub fn new(capacity: usize, metrics: Metrics, strategy: OverflowStrategy) -> Self {
        Self {
            queue: Arc::new(ArrayQueue::new(capacity)),
            metrics,
            overflow_strategy: strategy,
        }
    }

    pub fn send(&self, value: T) -> Result<(), T> {
        match self.overflow_strategy {
            OverflowStrategy::DropNewest => {
                match self.queue.push(value) {
                    Ok(()) => {
                        self.metrics.message_sent();
                        self.update_metrics();
                        Ok(())
                    }
                    Err(value) => {
                        self.metrics.message_dropped();
                        Err(value)
                    }
                }
            }
            OverflowStrategy::DropOldest => {
                loop {
                    match self.queue.push(value) {
                        Ok(()) => {
                            self.metrics.message_sent();
                            self.update_metrics();
                            return Ok(());
                        }
                        Err(v) => {
                            // Try to remove oldest and retry
                            if self.queue.pop().is_some() {
                                self.metrics.message_dropped();
                                value = v;
                                continue;
                            }
                            return Err(v);
                        }
                    }
                }
            }
            OverflowStrategy::Block(timeout) => {
                let start = std::time::Instant::now();
                loop {
                    match self.queue.push(value) {
                        Ok(()) => {
                            self.metrics.message_sent();
                            self.update_metrics();
                            return Ok(());
                        }
                        Err(v) => {
                            if start.elapsed() > timeout {
                                self.metrics.message_dropped();
                                return Err(v);
                            }
                            std::thread::yield_now();
                            value = v;
                        }
                    }
                }
            }
        }
    }

    pub fn recv(&self) -> Option<T> {
        let result = self.queue.pop();
        if result.is_some() {
            self.update_metrics();
        }
        result
    }

    pub fn try_recv_batch(&self, batch: &mut Vec<T>, max_size: usize) -> usize {
        let mut count = 0;
        while count < max_size {
            match self.queue.pop() {
                Some(item) => {
                    batch.push(item);
                    count += 1;
                }
                None => break,
            }
        }
        if count > 0 {
            self.update_metrics();
        }
        count
    }

    fn update_metrics(&self) {
        self.metrics.update_queue_depth(self.queue.len());
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.queue.capacity()
    }
}