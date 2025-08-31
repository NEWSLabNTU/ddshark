use crate::metrics::MetricsCollector;
use anyhow::Result;
use std::{fs::OpenOptions, io::Write, path::Path, time::Duration};
use tokio::{task, time::interval};
use tokio_util::sync::CancellationToken;

pub struct MetricsLogger {
    metrics: MetricsCollector,
    log_file_path: std::path::PathBuf,
    log_interval: Duration,
}

impl MetricsLogger {
    pub fn new<P: AsRef<Path>>(
        metrics: MetricsCollector,
        log_file_path: P,
        log_interval: Duration,
    ) -> Result<Self> {
        let log_file_path = log_file_path.as_ref().to_path_buf();

        // Create/open the CSV file and write header if it's new
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)?;

        // Check if file is empty (new) to write header
        if file.metadata()?.len() == 0 {
            writeln!(
                file,
                "timestamp,uptime_seconds,packets_received,packets_parsed,parse_errors,rtps_messages_found,\
                 messages_sent,messages_dropped,send_timeouts,messages_processed,processing_errors,\
                 state_updates,lock_acquisitions,queue_depth,max_queue_depth,packet_rate,message_rate,\
                 processing_rate,drop_rate,batch_count,avg_batch_size,processing_latency_p50,\
                 processing_latency_p99,lock_wait_p50,lock_wait_p99"
            )?;
        }

        Ok(Self {
            metrics,
            log_file_path,
            log_interval,
        })
    }

    pub async fn run(self, cancel_token: CancellationToken) -> Result<()> {
        let mut interval_timer = interval(self.log_interval);

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    // Log final metrics before exit
                    if let Err(e) = self.log_metrics() {
                        eprintln!("Failed to log final metrics: {}", e);
                    }
                    break;
                }
                _ = interval_timer.tick() => {
                    if let Err(e) = self.log_metrics() {
                        eprintln!("Failed to log metrics: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    fn log_metrics(&self) -> Result<()> {
        let snapshot = self.metrics.snapshot();
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path)?;

        writeln!(
            file,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.2},{:.2},{:.2},{:.2},{},{:.2},{},{},{},{}",
            timestamp,
            snapshot.uptime.as_secs_f64(),
            snapshot.packets_received,
            snapshot.packets_parsed,
            snapshot.parse_errors,
            snapshot.rtps_messages_found,
            snapshot.messages_sent,
            snapshot.messages_dropped,
            snapshot.send_timeouts,
            snapshot.messages_processed,
            snapshot.processing_errors,
            snapshot.state_updates,
            snapshot.lock_acquisitions,
            snapshot.queue_depth,
            snapshot.max_queue_depth,
            snapshot.packet_rate,
            snapshot.message_rate,
            snapshot.processing_rate,
            snapshot.drop_rate,
            snapshot.batch_count,
            snapshot.avg_batch_size,
            snapshot.processing_latency_p50,
            snapshot.processing_latency_p99,
            snapshot.lock_wait_p50,
            snapshot.lock_wait_p99,
        )?;

        Ok(())
    }
}

pub fn spawn_metrics_logger(
    metrics: MetricsCollector,
    log_file_path: std::path::PathBuf,
    cancel_token: CancellationToken,
) -> Result<task::JoinHandle<Result<()>>> {
    let logger = MetricsLogger::new(metrics, log_file_path, Duration::from_secs(1))?;

    let handle = task::spawn(async move { logger.run(cancel_token).await });

    Ok(handle)
}
