use etherparse::SingleVlanHeader;
use gethostname::gethostname;
use mac_address::mac_address_by_name;

use opentelemetry_api::{
    global::shutdown_tracer_provider,
    trace::{Span, SpanBuilder, SpanKind, Tracer},
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime, trace as sdktrace, trace::Sampler, Resource};
use opentelemetry_semantic_conventions as semcov;
use std::time::{Duration, SystemTime};

use crate::{
    message::{RtpsEvent, RtpsMessage},
    opts::Opts,
};
use rustdds::structure::guid::EntityKind;

pub struct TraceHandle {
    tracer: sdktrace::Tracer,
    mac_address: [u8; 6],
}

impl TraceHandle {
    pub fn new(opts: &Opts) -> Self {
        let mac_address = match mac_address_by_name(&opts.interface.as_deref().unwrap_or("eno2")) {
            Ok(Some(ma)) => ma.bytes(),
            Ok(None) => [0; 6],
            Err(_) => [0; 6],
        };

        // `endpoint` should always be set from the opts.
        let endpoint = opts
            .otlp_endpoint
            .as_deref()
            .unwrap_or("http://localhost:4317");

        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(endpoint)
            .with_timeout(Duration::from_secs(2));

        let trace_config = sdktrace::config()
            .with_sampler(Sampler::AlwaysOn)
            .with_max_events_per_span(64)
            .with_max_attributes_per_span(16)
            .with_resource(Resource::new(vec![
                KeyValue::new(semcov::resource::SERVICE_NAME, "dds.traffic"),
                KeyValue::new(
                    semcov::resource::HOST_NAME,
                    gethostname().to_string_lossy().to_string(),
                ),
            ]));

        let batch_config = sdktrace::BatchConfig::default()
            .with_max_concurrent_exports(4)
            .with_max_export_batch_size(512)
            .with_max_queue_size(500000);

        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(exporter)
            .with_trace_config(trace_config)
            .with_batch_config(batch_config)
            .install_batch(runtime::Tokio)
            .unwrap();

        TraceHandle {
            mac_address,
            tracer,
        }
    }

    pub fn send_trace(&self, message: &RtpsMessage, topic_name: String) -> () {
        let (headers, event) = (message.headers.clone(), message.event.clone());
        let capture_time = headers.pcap_header.ts;
        // let ma: [u8; 6] = headers.eth_header.destination;

        let (submsg_type, writer_id, sn, fragment_starting_num, payload_size) = match event {
            RtpsEvent::Data(event) => (
                "DATA",
                event.writer_id,
                event.writer_sn,
                0 as u32,
                event.payload_size,
            ),
            RtpsEvent::DataFrag(event) => (
                "DATA_FRAG",
                event.writer_id,
                event.writer_sn,
                event.fragment_starting_num,
                event.payload_size,
            ),
        };
        let traffic_type = match writer_id.entity_id.entity_kind {
            // TODO: add complete cases
            EntityKind::WRITER_NO_KEY_USER_DEFINED => "USER_DEFINED",
            _ => "BUILT_IN",
        };

        // Create attributes to be attached to the span.
        let attrs = vec![
            semcov::trace::EVENT_NAME.string("eno2"),
            KeyValue::new("traffic_type", traffic_type.to_string()),
            KeyValue::new("topic_name", topic_name.clone()),
            KeyValue::new("writer_id", convert_to_colon_sep_hex(writer_id.to_bytes())),
            KeyValue::new("sn", sn.0),
            KeyValue::new("fragment_starting_num", fragment_starting_num as i64),
            KeyValue::new("payload_size", payload_size as i64),
            KeyValue::new(
                "pcp",
                headers
                    .vlan_header
                    .unwrap_or(SingleVlanHeader::default())
                    .priority_code_point as i64,
            ),
        ];

        // Create a span with the given attributes. The start time is set to captured time.
        // The end time is set to captured time + payload size * 8 / 2.5Gbps.
        let mut span = self.tracer.build(SpanBuilder {
            name: submsg_type.into(),
            span_kind: Some(SpanKind::Internal),
            start_time: Some(convert_to_system_time(capture_time)),
            attributes: Some(attrs.into_iter().collect()),
            ..Default::default()
        });
        span.end_with_timestamp(
            convert_to_system_time(capture_time)
                + Duration::from_secs_f64(payload_size as f64 * 8. / (2.5 * 1e9)),
        );

        ()
    }
}

impl Drop for TraceHandle {
    fn drop(&mut self) {
        shutdown_tracer_provider();
    }
}

pub fn convert_to_system_time(capture_time: libc::timeval) -> SystemTime {
    SystemTime::UNIX_EPOCH
        + Duration::new(
            capture_time.tv_sec as u64,
            (capture_time.tv_usec * 1000) as u32,
        )
}

pub fn convert_to_colon_sep_hex<const N: usize>(obj: [u8; N]) -> String {
    obj.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(":")
}
