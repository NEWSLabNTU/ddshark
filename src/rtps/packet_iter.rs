use super::packet_decoder::{PacketDecoder, PacketKind, RtpsPacket};
use anyhow::Result;
use pcap::{Active, Capture, Offline, PacketIter};
use std::{thread, time::Instant};

pub enum MessageIter {
    Active(PacketIter<pcap::Active, PacketDecoder>),
    Offline(OfflineMessageIter),
}

impl MessageIter {
    pub fn new_active(capture: Capture<Active>) -> Self {
        MessageIter::from(capture.iter(PacketDecoder::new()))
    }

    pub fn new_offline(capture: Capture<Offline>) -> Self {
        OfflineMessageIter {
            packet_iter: capture.iter(PacketDecoder::new()),
            since: None,
        }
        .into()
    }
}

impl From<OfflineMessageIter> for MessageIter {
    fn from(v: OfflineMessageIter) -> Self {
        Self::Offline(v)
    }
}

impl Iterator for MessageIter {
    type Item = Result<RtpsPacket, pcap::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            MessageIter::Active(iter) => loop {
                let item = iter.next()?;
                match item {
                    Ok(PacketKind::Rtps(packet)) => break Some(Ok(packet)),
                    Ok(PacketKind::Other(_)) => continue,
                    Err(err) => break Some(Err(err)),
                }
            },
            MessageIter::Offline(iter) => iter.next(),
        }
    }
}

impl From<PacketIter<pcap::Active, PacketDecoder>> for MessageIter {
    fn from(v: PacketIter<pcap::Active, PacketDecoder>) -> Self {
        Self::Active(v)
    }
}

pub struct OfflineMessageIter {
    since: Option<(Instant, chrono::Duration)>,
    packet_iter: PacketIter<pcap::Offline, PacketDecoder>,
}

impl Iterator for OfflineMessageIter {
    type Item = Result<RtpsPacket, pcap::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item = self.packet_iter.next()?;
            let packet = match item {
                Ok(packet) => packet,
                Err(err) => break Some(Err(err)),
            };

            // Simulate the receipt rate
            {
                let now = Instant::now();
                let ts = packet.ts();
                let (since_instant, since_ts) = *self.since.get_or_insert((now, ts));

                let diff = (ts - since_ts).to_std().unwrap();
                let until = since_instant + diff;

                if let Some(wait) = until.checked_duration_since(now) {
                    thread::sleep(wait);
                }
            }

            match packet {
                PacketKind::Rtps(packet) => break Some(Ok(packet)),
                PacketKind::Other(_) => continue,
            }
        }
    }
}
