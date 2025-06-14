use parking_lot::RwLock;
use std::{
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct MetricsCollector {
    inner: Arc<MetricsInner>,
}

struct MetricsInner {
    // RTPS Watcher metrics
    packets_received: AtomicU64,
    packets_parsed: AtomicU64,
    parse_errors: AtomicU64,
    rtps_messages_found: AtomicU64,

    // Channel metrics
    messages_sent: AtomicU64,
    messages_dropped: AtomicU64,
    send_timeouts: AtomicU64,
    queue_depth: AtomicUsize,
    max_queue_depth: AtomicUsize,

    // Updater metrics
    messages_processed: AtomicU64,
    batch_count: AtomicU64,
    total_batch_size: AtomicU64,
    processing_errors: AtomicU64,

    // State metrics
    state_updates: AtomicU64,
    lock_acquisitions: AtomicU64,
    lock_wait_time_us: AtomicU64,

    // Timing metrics
    start_time: Instant,
    last_reset: RwLock<Instant>,

    // Latency tracking (using RwLock for percentile calculations)
    latencies: RwLock<LatencyTracker>,
}

#[derive(Default)]
struct LatencyTracker {
    processing_latencies_us: Vec<u64>,
    lock_wait_latencies_us: Vec<u64>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MetricsInner {
                packets_received: AtomicU64::new(0),
                packets_parsed: AtomicU64::new(0),
                parse_errors: AtomicU64::new(0),
                rtps_messages_found: AtomicU64::new(0),

                messages_sent: AtomicU64::new(0),
                messages_dropped: AtomicU64::new(0),
                send_timeouts: AtomicU64::new(0),
                queue_depth: AtomicUsize::new(0),
                max_queue_depth: AtomicUsize::new(0),

                messages_processed: AtomicU64::new(0),
                batch_count: AtomicU64::new(0),
                total_batch_size: AtomicU64::new(0),
                processing_errors: AtomicU64::new(0),

                state_updates: AtomicU64::new(0),
                lock_acquisitions: AtomicU64::new(0),
                lock_wait_time_us: AtomicU64::new(0),

                start_time: Instant::now(),
                last_reset: RwLock::new(Instant::now()),
                latencies: RwLock::new(LatencyTracker::default()),
            }),
        }
    }

    // RTPS Watcher metrics
    pub fn packet_received(&self) {
        self.inner.packets_received.fetch_add(1, Ordering::Relaxed);
    }

    pub fn packet_parsed(&self) {
        self.inner.packets_parsed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn parse_error(&self) {
        self.inner.parse_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn rtps_message_found(&self) {
        self.inner
            .rtps_messages_found
            .fetch_add(1, Ordering::Relaxed);
    }

    // Channel metrics
    pub fn message_sent(&self) {
        self.inner.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    pub fn message_dropped(&self) {
        self.inner.messages_dropped.fetch_add(1, Ordering::Relaxed);
    }

    pub fn send_timeout(&self) {
        self.inner.send_timeouts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn update_queue_depth(&self, depth: usize) {
        self.inner.queue_depth.store(depth, Ordering::Relaxed);

        // Update max if needed
        let mut max = self.inner.max_queue_depth.load(Ordering::Relaxed);
        while depth > max {
            match self.inner.max_queue_depth.compare_exchange_weak(
                max,
                depth,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => max = current,
            }
        }
    }

    // Updater metrics
    pub fn message_processed(&self) {
        self.inner
            .messages_processed
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn batch_processed(&self, batch_size: usize) {
        self.inner.batch_count.fetch_add(1, Ordering::Relaxed);
        self.inner
            .total_batch_size
            .fetch_add(batch_size as u64, Ordering::Relaxed);
    }

    pub fn processing_error(&self) {
        self.inner.processing_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_processing_latency(&self, duration: Duration) {
        let us = duration.as_micros() as u64;
        let mut latencies = self.inner.latencies.write();
        latencies.processing_latencies_us.push(us);

        // Keep only last 10000 samples to avoid unbounded growth
        if latencies.processing_latencies_us.len() > 10000 {
            latencies.processing_latencies_us.drain(0..5000);
        }
    }

    // State metrics
    pub fn state_update(&self) {
        self.inner.state_updates.fetch_add(1, Ordering::Relaxed);
    }

    pub fn lock_acquired(&self, wait_time: Duration) {
        self.inner.lock_acquisitions.fetch_add(1, Ordering::Relaxed);
        let us = wait_time.as_micros() as u64;
        self.inner
            .lock_wait_time_us
            .fetch_add(us, Ordering::Relaxed);

        let mut latencies = self.inner.latencies.write();
        latencies.lock_wait_latencies_us.push(us);

        // Keep only last 10000 samples
        if latencies.lock_wait_latencies_us.len() > 10000 {
            latencies.lock_wait_latencies_us.drain(0..5000);
        }
    }

    // Get comprehensive snapshot
    pub fn snapshot(&self) -> MetricsSnapshot {
        let packets_received = self.inner.packets_received.load(Ordering::Relaxed);
        let packets_parsed = self.inner.packets_parsed.load(Ordering::Relaxed);
        let messages_sent = self.inner.messages_sent.load(Ordering::Relaxed);
        let messages_dropped = self.inner.messages_dropped.load(Ordering::Relaxed);
        let messages_processed = self.inner.messages_processed.load(Ordering::Relaxed);

        let elapsed = self.inner.start_time.elapsed().as_secs_f64();

        // Calculate rates
        let packet_rate = if elapsed > 0.0 {
            packets_received as f64 / elapsed
        } else {
            0.0
        };

        let message_rate = if elapsed > 0.0 {
            messages_sent as f64 / elapsed
        } else {
            0.0
        };

        let processing_rate = if elapsed > 0.0 {
            messages_processed as f64 / elapsed
        } else {
            0.0
        };

        // Calculate drop rate
        let total_attempts = messages_sent + messages_dropped;
        let drop_rate = if total_attempts > 0 {
            (messages_dropped as f64 / total_attempts as f64) * 100.0
        } else {
            0.0
        };

        // Calculate average batch size
        let batch_count = self.inner.batch_count.load(Ordering::Relaxed);
        let total_batch_size = self.inner.total_batch_size.load(Ordering::Relaxed);
        let avg_batch_size = if batch_count > 0 {
            total_batch_size as f64 / batch_count as f64
        } else {
            0.0
        };

        // Get latency percentiles
        let latencies = self.inner.latencies.read();
        let processing_latency_p50 = percentile(&latencies.processing_latencies_us, 50.0);
        let processing_latency_p99 = percentile(&latencies.processing_latencies_us, 99.0);
        let lock_wait_p50 = percentile(&latencies.lock_wait_latencies_us, 50.0);
        let lock_wait_p99 = percentile(&latencies.lock_wait_latencies_us, 99.0);

        MetricsSnapshot {
            // Counts
            packets_received,
            packets_parsed,
            parse_errors: self.inner.parse_errors.load(Ordering::Relaxed),
            rtps_messages_found: self.inner.rtps_messages_found.load(Ordering::Relaxed),
            messages_sent,
            messages_dropped,
            send_timeouts: self.inner.send_timeouts.load(Ordering::Relaxed),
            messages_processed,
            processing_errors: self.inner.processing_errors.load(Ordering::Relaxed),
            state_updates: self.inner.state_updates.load(Ordering::Relaxed),
            lock_acquisitions: self.inner.lock_acquisitions.load(Ordering::Relaxed),

            // Queue metrics
            queue_depth: self.inner.queue_depth.load(Ordering::Relaxed),
            max_queue_depth: self.inner.max_queue_depth.load(Ordering::Relaxed),

            // Rates
            packet_rate,
            message_rate,
            processing_rate,
            drop_rate,

            // Batch metrics
            batch_count,
            avg_batch_size,

            // Latency metrics (in microseconds)
            processing_latency_p50,
            processing_latency_p99,
            lock_wait_p50,
            lock_wait_p99,

            // Timing
            uptime: self.inner.start_time.elapsed(),
        }
    }

    // Reset metrics (useful for interval-based reporting)
    pub fn reset_interval_metrics(&self) {
        *self.inner.last_reset.write() = Instant::now();
        // We don't reset counters, but we could track interval-specific metrics here
    }
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    // Counts
    pub packets_received: u64,
    pub packets_parsed: u64,
    pub parse_errors: u64,
    pub rtps_messages_found: u64,
    pub messages_sent: u64,
    pub messages_dropped: u64,
    pub send_timeouts: u64,
    pub messages_processed: u64,
    pub processing_errors: u64,
    pub state_updates: u64,
    pub lock_acquisitions: u64,

    // Queue metrics
    pub queue_depth: usize,
    pub max_queue_depth: usize,

    // Rates (per second)
    pub packet_rate: f64,
    pub message_rate: f64,
    pub processing_rate: f64,
    pub drop_rate: f64, // percentage

    // Batch metrics
    pub batch_count: u64,
    pub avg_batch_size: f64,

    // Latency percentiles (microseconds)
    pub processing_latency_p50: u64,
    pub processing_latency_p99: u64,
    pub lock_wait_p50: u64,
    pub lock_wait_p99: u64,

    // Timing
    pub uptime: Duration,
}

impl MetricsSnapshot {
    pub fn format_summary(&self) -> String {
        format!(
            "Packets: {}/s | Messages: {}/s ({}% drop) | Queue: {}/{} | Process: {}/s | P99: {}μs",
            self.packet_rate as u64,
            self.message_rate as u64,
            self.drop_rate as u64,
            self.queue_depth,
            self.max_queue_depth,
            self.processing_rate as u64,
            self.processing_latency_p99,
        )
    }
}

// Simple percentile calculation
fn percentile(values: &[u64], p: f64) -> u64 {
    if values.is_empty() {
        return 0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_unstable();

    let index = ((p / 100.0) * (sorted.len() - 1) as f64) as usize;
    sorted[index]
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}
