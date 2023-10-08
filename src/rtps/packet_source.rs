use super::packet_decoder::{PacketDecoder, PacketKind, RtpsPacket};
use anyhow::{anyhow, Result};
use pcap::{Active, Capture, Device, Offline, PacketIter};
use std::{path::PathBuf, thread, time::Instant};

#[derive(Debug)]
pub enum PacketSource {
    Default,
    File { path: PathBuf, sync_time: bool },
    Interface(String),
}

impl PacketSource {
    pub fn into_message_iter(self) -> Result<MessageIter> {
        let iter = match self {
            PacketSource::Default => {
                let cap = Device::lookup()?
                    .ok_or_else(|| anyhow!("no available network device"))?
                    .open()?;
                MessageIter::new_active(cap)
            }
            PacketSource::File { path, sync_time } => {
                let cap = Capture::from_file(path)?;
                MessageIter::new_offline(cap, sync_time)
            }
            PacketSource::Interface(interface) => {
                let cap = Device::list()?
                    .into_iter()
                    .find(|dev| dev.name == interface)
                    .ok_or_else(|| anyhow!("unable to find network device {interface}"))?
                    .open()?;
                MessageIter::from(cap.iter(PacketDecoder::new()))
            }
        };

        Ok(iter)
    }
}

pub enum MessageIter {
    Active(PacketIter<pcap::Active, PacketDecoder>),
    Offline(OfflineMessageIter),
}

impl MessageIter {
    pub fn new_active(capture: Capture<Active>) -> Self {
        MessageIter::from(capture.iter(PacketDecoder::new()))
    }

    pub fn new_offline(capture: Capture<Offline>, sync_time: bool) -> Self {
        OfflineMessageIter {
            packet_iter: capture.iter(PacketDecoder::new()),
            sync_time,
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
    sync_time: bool,
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

            if self.sync_time {
                let ts = packet.ts();
                let (since_instant, since_ts) =
                    *self.since.get_or_insert_with(|| (Instant::now(), ts));

                let diff = (ts - since_ts).to_std().unwrap();
                let until = since_instant + diff;

                if let Some(wait) = until.checked_duration_since(Instant::now()) {
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
