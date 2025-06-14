use crate::{metrics::MetricsCollector, opts::Opts};
use anyhow::Result;
use gethostname::gethostname;
use opentelemetry::KeyValue;
use opentelemetry_api::metrics::{Counter, Gauge, Histogram, Meter, MeterProvider};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    metrics::{
        reader::{DefaultAggregationSelector, DefaultTemporalitySelector},
        MeterProvider as SdkMeterProvider, PeriodicReader,
    },
    runtime, Resource,
};
use opentelemetry_semantic_conventions as semcov;
use std::time::Duration;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::error;

pub struct OtlpMetricsExporter {
    metrics: MetricsCollector,
    meter: Meter,

    // Counters
    packets_received: Counter<u64>,
    packets_parsed: Counter<u64>,
    parse_errors: Counter<u64>,
    rtps_messages_found: Counter<u64>,
    messages_sent: Counter<u64>,
    messages_dropped: Counter<u64>,
    send_timeouts: Counter<u64>,
    messages_processed: Counter<u64>,
    processing_errors: Counter<u64>,
    state_updates: Counter<u64>,
    lock_acquisitions: Counter<u64>,

    // Gauges
    queue_depth: Gauge<u64>,
    max_queue_depth: Gauge<u64>,
    packet_rate: Gauge<f64>,
    message_rate: Gauge<f64>,
    processing_rate: Gauge<f64>,
    drop_rate: Gauge<f64>,

    // Histograms
    processing_latency: Histogram<u64>,
    lock_wait_time: Histogram<u64>,
}

impl OtlpMetricsExporter {
    pub fn new(metrics: MetricsCollector, opts: &Opts) -> Result<Self> {
        let endpoint = opts
            .otlp_endpoint
            .as_deref()
            .unwrap_or("http://localhost:4317");

        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(endpoint)
            .with_timeout(Duration::from_secs(5));

        let reader = PeriodicReader::builder(exporter, runtime::Tokio)
            .with_interval(Duration::from_secs(10))
            .with_timeout(Duration::from_secs(5))
            .build();

        let provider = SdkMeterProvider::builder()
            .with_reader(reader)
            .with_resource(Resource::new(vec![
                KeyValue::new(semcov::resource::SERVICE_NAME, "ddshark"),
                KeyValue::new(semcov::resource::SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
                KeyValue::new(
                    semcov::resource::HOST_NAME,
                    gethostname().to_string_lossy().to_string(),
                ),
            ]))
            .build();

        let meter = provider.meter("ddshark");

        // Create metrics instruments
        let packets_received = meter
            .u64_counter("ddshark_packets_received_total")
            .with_description("Total number of packets received")
            .init();

        let packets_parsed = meter
            .u64_counter("ddshark_packets_parsed_total")
            .with_description("Total number of packets successfully parsed")
            .init();

        let parse_errors = meter
            .u64_counter("ddshark_parse_errors_total")
            .with_description("Total number of packet parse errors")
            .init();

        let rtps_messages_found = meter
            .u64_counter("ddshark_rtps_messages_total")
            .with_description("Total number of RTPS messages found")
            .init();

        let messages_sent = meter
            .u64_counter("ddshark_messages_sent_total")
            .with_description("Total number of messages sent to queue")
            .init();

        let messages_dropped = meter
            .u64_counter("ddshark_messages_dropped_total")
            .with_description("Total number of messages dropped due to congestion")
            .init();

        let send_timeouts = meter
            .u64_counter("ddshark_send_timeouts_total")
            .with_description("Total number of send timeouts")
            .init();

        let messages_processed = meter
            .u64_counter("ddshark_messages_processed_total")
            .with_description("Total number of messages processed by updater")
            .init();

        let processing_errors = meter
            .u64_counter("ddshark_processing_errors_total")
            .with_description("Total number of message processing errors")
            .init();

        let state_updates = meter
            .u64_counter("ddshark_state_updates_total")
            .with_description("Total number of state updates")
            .init();

        let lock_acquisitions = meter
            .u64_counter("ddshark_lock_acquisitions_total")
            .with_description("Total number of lock acquisitions")
            .init();

        let queue_depth = meter
            .u64_gauge("ddshark_queue_depth")
            .with_description("Current queue depth")
            .init();

        let max_queue_depth = meter
            .u64_gauge("ddshark_max_queue_depth")
            .with_description("Maximum queue depth observed")
            .init();

        let packet_rate = meter
            .f64_gauge("ddshark_packet_rate")
            .with_description("Packet processing rate (packets/sec)")
            .init();

        let message_rate = meter
            .f64_gauge("ddshark_message_rate")
            .with_description("Message processing rate (messages/sec)")
            .init();

        let processing_rate = meter
            .f64_gauge("ddshark_processing_rate")
            .with_description("Message processing rate (messages/sec)")
            .init();

        let drop_rate = meter
            .f64_gauge("ddshark_drop_rate")
            .with_description("Message drop rate percentage")
            .init();

        let processing_latency = meter
            .u64_histogram("ddshark_processing_latency_microseconds")
            .with_description("Message processing latency in microseconds")
            .init();

        let lock_wait_time = meter
            .u64_histogram("ddshark_lock_wait_time_microseconds")
            .with_description("Lock wait time in microseconds")
            .init();

        Ok(Self {
            metrics,
            meter,
            packets_received,
            packets_parsed,
            parse_errors,
            rtps_messages_found,
            messages_sent,
            messages_dropped,
            send_timeouts,
            messages_processed,
            processing_errors,
            state_updates,
            lock_acquisitions,
            queue_depth,
            max_queue_depth,
            packet_rate,
            message_rate,
            processing_rate,
            drop_rate,
            processing_latency,
            lock_wait_time,
        })
    }

    pub async fn run(self, cancel_token: CancellationToken) -> Result<()> {
        let mut export_interval = interval(Duration::from_secs(10));

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    // Export final metrics before exit
                    self.export_metrics();
                    break;
                }
                _ = export_interval.tick() => {
                    self.export_metrics();
                }
            }
        }

        Ok(())
    }

    fn export_metrics(&self) {
        let snapshot = self.metrics.snapshot();

        // Update counters (these need to track the absolute values since OTLP handles deltas)
        self.packets_received.add(snapshot.packets_received, &[]);
        self.packets_parsed.add(snapshot.packets_parsed, &[]);
        self.parse_errors.add(snapshot.parse_errors, &[]);
        self.rtps_messages_found
            .add(snapshot.rtps_messages_found, &[]);
        self.messages_sent.add(snapshot.messages_sent, &[]);
        self.messages_dropped.add(snapshot.messages_dropped, &[]);
        self.send_timeouts.add(snapshot.send_timeouts, &[]);
        self.messages_processed
            .add(snapshot.messages_processed, &[]);
        self.processing_errors.add(snapshot.processing_errors, &[]);
        self.state_updates.add(snapshot.state_updates, &[]);
        self.lock_acquisitions.add(snapshot.lock_acquisitions, &[]);

        // Update gauges
        self.queue_depth.record(snapshot.queue_depth as u64, &[]);
        self.max_queue_depth
            .record(snapshot.max_queue_depth as u64, &[]);
        self.packet_rate.record(snapshot.packet_rate, &[]);
        self.message_rate.record(snapshot.message_rate, &[]);
        self.processing_rate.record(snapshot.processing_rate, &[]);
        self.drop_rate.record(snapshot.drop_rate, &[]);

        // Record histograms
        self.processing_latency
            .record(snapshot.processing_latency_p50, &[]);
        self.processing_latency
            .record(snapshot.processing_latency_p99, &[]);
        self.lock_wait_time.record(snapshot.lock_wait_p50, &[]);
        self.lock_wait_time.record(snapshot.lock_wait_p99, &[]);
    }
}

pub fn spawn_otlp_metrics_exporter(
    metrics: MetricsCollector,
    opts: &Opts,
    cancel_token: CancellationToken,
) -> Result<tokio::task::JoinHandle<Result<()>>> {
    let exporter = OtlpMetricsExporter::new(metrics, opts)?;

    let handle = tokio::task::spawn(async move { exporter.run(cancel_token).await });

    Ok(handle)
}
