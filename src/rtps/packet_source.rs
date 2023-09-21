use super::packet_decoder::PacketDecoder;
use crate::message::PacketHeaders;
use anyhow::{anyhow, Result};
use pcap::{Capture, Device, PacketIter};
use rustdds::serialization::Message;
use std::path::PathBuf;

#[derive(Debug)]
pub enum PacketSource {
    Default,
    File(PathBuf),
    Interface(String),
}

impl PacketSource {
    pub fn into_message_iter(self) -> Result<MessageIter> {
        let iter = match self {
            PacketSource::Default => {
                let cap = Device::lookup()?
                    .ok_or_else(|| anyhow!("no available network device"))?
                    .open()?;
                MessageIter::from(cap.iter(PacketDecoder::new()))
            }
            PacketSource::File(path) => {
                let cap = Capture::from_file(path)?;
                MessageIter::from(cap.iter(PacketDecoder::new()))
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
    Offline(PacketIter<pcap::Offline, PacketDecoder>),
}

impl Iterator for MessageIter {
    type Item = Result<(PacketHeaders, Message), pcap::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item = match self {
                MessageIter::Active(iter) => iter.next()?,
                MessageIter::Offline(iter) => iter.next()?,
            };
            if let Some(item) = item.transpose() {
                break Some(item);
            }
        }
    }
}

impl From<PacketIter<pcap::Offline, PacketDecoder>> for MessageIter {
    fn from(v: PacketIter<pcap::Offline, PacketDecoder>) -> Self {
        Self::Offline(v)
    }
}

impl From<PacketIter<pcap::Active, PacketDecoder>> for MessageIter {
    fn from(v: PacketIter<pcap::Active, PacketDecoder>) -> Self {
        Self::Active(v)
    }
}
