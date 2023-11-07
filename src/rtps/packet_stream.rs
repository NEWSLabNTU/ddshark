use std::{task::Poll, time::Instant};

use super::{
    packet_decoder::{PacketDecoder, PacketKind, RtpsPacket},
    PacketSource,
};
use anyhow::{anyhow, Result};
use futures::{
    stream::{self, BoxStream},
    Stream, StreamExt, TryStreamExt,
};
use itertools::Itertools;
use pcap::{Active, Capture, Device, Offline};

pub type RtpsPacketStream = BoxStream<'static, Result<RtpsPacket, pcap::Error>>;

pub fn build_packet_stream(src: PacketSource) -> Result<RtpsPacketStream> {
    let stream = match src {
        PacketSource::Default => {
            let cap = Device::lookup()?
                .ok_or_else(|| anyhow!("no available network device"))?
                .open()?;
            build_active_packet_stream(cap)?.boxed()
        }
        PacketSource::File { path, sync_time } => {
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
    // let iter = cap
    //     .iter(PacketDecoder::new())
    //     .map_ok(|pkt| {
    //         let PacketKind::Rtps(pkt) = pkt else {
    //             return None;
    //         };
    //         Some(pkt)
    //     })
    //     .flatten_ok();
    // let stream = futures::stream::iter(iter);

    let iter = cap.iter(PacketDecoder::new());
    let stream = stream::iter(iter).try_filter_map(|pkt| async move {
        let PacketKind::Rtps(pkt) = pkt else {
            return Ok(None);
        };
        Ok(Some(pkt))
    });

    Ok(stream)
}
