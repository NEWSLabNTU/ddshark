use super::{
    packet_decoder::PacketDecoder,
    packet_iter::MessageIter,
    packet_stream::{build_packet_stream, RtpsPacketStream},
};
use anyhow::{anyhow, Result};
use pcap::{Capture, Device};
use std::path::PathBuf;

#[derive(Debug)]
pub enum PacketSource {
    Default,
    File { path: PathBuf, sync_time: bool },
    Interface(String),
}

impl PacketSource {
    pub fn into_iter(self) -> Result<MessageIter> {
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

    pub fn into_stream(self) -> Result<RtpsPacketStream> {
        build_packet_stream(self)
    }
}
