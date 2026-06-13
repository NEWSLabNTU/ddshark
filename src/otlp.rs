use crate::{message::RtpsSubmsgEventKind, opts::Opts};

use gethostname::gethostname;
use mac_address::mac_address_by_name;
use opentelemetry::{trace::TracerProvider as _, KeyValue};
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    trace::{Sampler, SdkTracer, SdkTracerProvider},
    Resource,
};
use std::time::{Duration, SystemTime};

pub struct TraceHandle {
    provider: SdkTracerProvider,
    tracer: SdkTracer,
    mac_address: [u8; 6],
}

impl TraceHandle {
    pub fn new(opts: &Opts) -> Self {
        let mac_address = match mac_address_by_name(opts.interface.as_deref().unwrap_or("eno2")) {
            Ok(Some(ma)) => ma.bytes(),
            Ok(None) => [0; 6],
            Err(_) => [0; 6],
        };

        // `endpoint` should always be set from the opts.
        let endpoint = opts
            .otlp_endpoint
            .as_deref()
            .unwrap_or("http://localhost:4317");

        let exporter = SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .with_timeout(Duration::from_secs(2))
            .build()
            .expect("failed to build OTLP span exporter");

        let resource = Resource::builder()
            .with_service_name("dds.traffic")
            .with_attribute(KeyValue::new(
                "host.name",
                gethostname().to_string_lossy().to_string(),
            ))
            .build();

        let provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_sampler(Sampler::AlwaysOn)
            .with_resource(resource)
            .build();

        let tracer = provider.tracer("ddshark");

        TraceHandle {
            provider,
            tracer,
            mac_address,
        }
    }

    pub fn send_trace(&self, _message: &RtpsSubmsgEventKind, _topic_name: String) {
        todo!();
    }

    // pub fn send_trace(&self, message: &RtpsEvent, topic_name: String) {
    //     let (headers, event) = (message.headers.clone(), message.context.clone());
    //     let capture_time = headers.pcap_header.ts;
    //     // let ma: [u8; 6] = headers.eth_header.destination;

    //     let (submsg_type, writer_id, sn, fragment_starting_num, payload_size) = match event {
    //         RtpsEvent::Data(event) => (
    //             "DATA",
    //             event.writer_guid,
    //             event.writer_sn,
    //             0u32,
    //             event.payload_size,
    //         ),
    //         RtpsEvent::DataFrag(event) => (
    //             "DATA_FRAG",
    //             event.writer_guid,
    //             event.writer_sn,
    //             event.fragment_starting_num,
    //             event.payload_size,
    //         ),
    //         RtpsEvent::Gap(_) => todo!(),
    //         RtpsEvent::Heartbeat(_) => todo!(),
    //         RtpsEvent::AckNack(_) => todo!(),
    //         RtpsEvent::NackFrag(_) => todo!(),
    //         RtpsEvent::HeartbeatFrag(_) => todo!(),
    //     };
    //     let traffic_type = match writer_id.entity_id.entity_kind {
    //         // TODO: add complete cases
    //         EntityKind::WRITER_NO_KEY_USER_DEFINED => "USER_DEFINED",
    //         _ => "BUILT_IN",
    //     };

    //     // Create attributes to be attached to the span.
    //     let attrs = vec![
    //         semcov::trace::EVENT_NAME.string("eno2"),
    //         KeyValue::new("traffic_type", traffic_type.to_string()),
    //         KeyValue::new("topic_name", topic_name),
    //         KeyValue::new("writer_id", convert_to_colon_sep_hex(writer_id.to_bytes())),
    //         KeyValue::new("sn", sn.0),
    //         KeyValue::new("fragment_starting_num", fragment_starting_num as i64),
    //         KeyValue::new("payload_size", payload_size as i64),
    //         KeyValue::new(
    //             "pcp",
    //             headers
    //                 .vlan_header
    //                 .unwrap_or(SingleVlanHeader::default())
    //                 .priority_code_point as i64,
    //         ),
    //     ];

    //     // Create a span with the given attributes. The start time is set to captured time.
    //     // The end time is set to captured time + payload size * 8 / 2.5Gbps.
    //     let mut span = self.tracer.build(SpanBuilder {
    //         name: submsg_type.into(),
    //         span_kind: Some(SpanKind::Internal),
    //         start_time: Some(convert_to_system_time(capture_time)),
    //         attributes: Some(attrs.into_iter().collect()),
    //         ..Default::default()
    //     });
    //     span.end_with_timestamp(
    //         convert_to_system_time(capture_time)
    //             + Duration::from_secs_f64(payload_size as f64 * 8. / (2.5 * 1e9)),
    //     );
    // }
}

impl Drop for TraceHandle {
    fn drop(&mut self) {
        let _ = self.provider.shutdown();
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
