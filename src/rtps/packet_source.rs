use super::{
    packet_decoder::PacketDecoder,
    packet_iter::MessageIter,
    packet_stream::{build_packet_stream, RtpsPacketStream},
};
use crate::capabilities;
use anyhow::{anyhow, Result};
use pcap::{Capture, Device};
use std::path::PathBuf;

#[derive(Debug)]
pub enum PacketSource {
    Default,
    File { path: PathBuf },
    Interface(String),
}

impl PacketSource {
    pub fn into_iter(self) -> Result<MessageIter> {
        let iter = match self {
            PacketSource::Default => {
                let device =
                    Device::lookup()?.ok_or_else(|| anyhow!("no available network device"))?;
                match device.open() {
                    Ok(cap) => MessageIter::new_active(cap),
                    Err(e) => {
                        // Check if it's a permission error
                        if e.to_string().contains("permission")
                            || e.to_string().contains("Operation not permitted")
                        {
                            return Err(anyhow!(
                                "{}\n\n{}",
                                e,
                                capabilities::get_capability_error_message()
                            ));
                        }
                        return Err(e.into());
                    }
                }
            }
            PacketSource::File { path } => {
                let cap = Capture::from_file(path)?;
                MessageIter::new_offline(cap)
            }
            PacketSource::Interface(interface) => {
                let device = Device::list()?
                    .into_iter()
                    .find(|dev| dev.name == interface)
                    .ok_or_else(|| anyhow!("unable to find network device {interface}"))?;
                match device.open() {
                    Ok(cap) => MessageIter::from(cap.iter(PacketDecoder::new())),
                    Err(e) => {
                        // Check if it's a permission error
                        if e.to_string().contains("permission")
                            || e.to_string().contains("Operation not permitted")
                        {
                            return Err(anyhow!(
                                "{}\n\n{}",
                                e,
                                capabilities::get_capability_error_message()
                            ));
                        }
                        return Err(e.into());
                    }
                }
            }
        };

        Ok(iter)
    }

    pub fn into_stream(self) -> Result<RtpsPacketStream> {
        build_packet_stream(self)
    }
}
