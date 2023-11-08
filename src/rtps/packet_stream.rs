use super::{
    packet_decoder::{PacketDecoder, PacketKind, RtpsPacket},
    PacketSource,
};
use anyhow::{anyhow, Result};
use futures::{
    stream::{self, BoxStream},
    FutureExt, Stream, StreamExt, TryFutureExt, TryStreamExt,
};
use pcap::{Active, Capture, Device, Offline};
use std::time::Instant;

pub type RtpsPacketStream = BoxStream<'static, Result<RtpsPacket, pcap::Error>>;

pub fn build_packet_stream(src: PacketSource) -> Result<RtpsPacketStream> {
    let stream = match src {
        PacketSource::Default => {
            let cap = Device::lookup()?
                .ok_or_else(|| anyhow!("no available network device"))?
                .open()?;
            build_active_packet_stream(cap)?.boxed()
        }
        PacketSource::File { path } => {
            let cap = Capture::from_file(path)?;
            build_offline_packet_stream(cap)?.boxed()
        }
        PacketSource::Interface(interface) => {
            let cap = Device::list()?
                .into_iter()
                .find(|dev| dev.name == interface)
                .ok_or_else(|| anyhow!("unable to find network device {interface}"))?
                .open()?;
            build_active_packet_stream(cap)?.boxed()
        }
    };

    Ok(stream)
}

fn build_active_packet_stream(
    cap: Capture<Active>,
) -> Result<impl Stream<Item = Result<RtpsPacket, pcap::Error>> + Send + 'static> {
    let stream = cap
        .setnonblock()?
        .stream(PacketDecoder::new())?
        .try_filter_map(|pkt| async move {
            let PacketKind::Rtps(pkt) = pkt else {
                return Ok(None);
            };

            Ok(Some(pkt))
        });
    Ok(stream)
}

fn build_offline_packet_stream(
    cap: Capture<Offline>,
) -> Result<impl Stream<Item = Result<RtpsPacket, pcap::Error>> + Send + 'static> {
    let iter = cap.iter(PacketDecoder::new());
    let mut stream = stream::iter(iter);

    let stream = async move {
        let Some(first_packet) = stream.try_next().await? else {
            return Ok(None);
        };

        let since_instant = Instant::now();
        let since_ts = first_packet.ts();

        let rest = stream.and_then(move |packet| async move {
            // Simulate the receipt rate
            let now = Instant::now();
            let ts = packet.ts();

            let diff = (ts - since_ts).to_std().unwrap();
            let until = since_instant + diff;

            if let Some(wait) = until.checked_duration_since(now) {
                tokio::time::sleep(wait).await;
            }

            Ok(packet)
        });

        let stream = stream::iter([Ok(first_packet)]).chain(rest);

        Result::<_, pcap::Error>::Ok(Some(stream))
    }
    .map_ok(|stream| stream::iter(stream).flatten())
    .into_stream()
    .try_flatten()
    .try_filter_map(|packet| async move {
        // Get the RTPS packet
        let PacketKind::Rtps(packet) = packet else {
            return Ok(None);
        };

        Ok(Some(packet))
    });

    Ok(stream)
}
