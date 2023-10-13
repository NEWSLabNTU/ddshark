use crate::message::PacketHeaders;
use anyhow::Result;
use bytes::Bytes;
use etherparse::{EtherType, Ethernet2Header, SingleVlanHeader};
use libc::timeval;
use pcap::{PacketCodec, PacketHeader};
use rustdds::serialization::Message;
use smoltcp::{
    phy::ChecksumCapabilities,
    wire::{Ipv4Packet, Ipv4Repr, UdpPacket, UdpRepr},
};
use std::{
    collections::{BTreeMap, HashMap},
    net::Ipv4Addr,
};
use tracing::error;

pub struct PacketDecoder {
    /// Map of (source, destination, id) to (fragment offset, payload)
    fragments: HashMap<(Ipv4Addr, Ipv4Addr, u16), BTreeMap<u16, Vec<u8>>>,
    /// Map of (source, destination, id) to (total received length, total length)
    assemblers: HashMap<(Ipv4Addr, Ipv4Addr, u16), (usize, usize)>,
}

impl PacketDecoder {
    pub fn new() -> Self {
        PacketDecoder {
            fragments: HashMap::new(),
            assemblers: HashMap::new(),
        }
    }

    /// Process a packet and dissect its ethernet header, and optionaly VLAN header.
    pub fn dissect_eth_header<'a>(
        packet: &'a pcap::Packet,
    ) -> Result<(Ethernet2Header, Option<SingleVlanHeader>, &'a [u8]), &'static str> {
        let (eth_header, data) =
            Ethernet2Header::from_slice(packet).map_err(|_| "Failed to parse Ethernet header")?;

        match EtherType::from_u16(eth_header.ether_type) {
            Some(EtherType::VlanTaggedFrame) => {
                let (vlan_header, remaining_data) = SingleVlanHeader::from_slice(data)
                    .map_err(|_| "Failed to parse VLAN header")?;
                Ok((eth_header, Some(vlan_header), remaining_data))
            }
            Some(EtherType::Ipv4) => Ok((eth_header, None, data)),
            _ => Err("Unsupported EtherType"),
        }
    }

    /// Check if the packet is a fragment and return true if it is
    pub fn is_fragment(packet_data: &[u8]) -> Result<bool, &'static str> {
        let ip_packet = Ipv4Packet::new_checked(packet_data)
            .map_err(|_| "Failed to parse IPv4 packet header")?;

        if ip_packet.more_frags() || ip_packet.frag_offset() != 0 {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Process a packet and dissect its IP header.
    /// Returns the IP header and the payload
    pub fn dissect_ip_header(packet_data: &[u8]) -> Result<(Ipv4Repr, &[u8]), &'static str> {
        let checksum_caps = ChecksumCapabilities::default();

        let ip_packet = Ipv4Packet::new_checked(packet_data)
            .map_err(|_| "Failed to parse IPv4 packet header")?;
        let ip_repr = Ipv4Repr::parse(&ip_packet, &checksum_caps)
            .map_err(|_| "Failed to parse IPv4 packet header")?;

        let payload = &packet_data[ip_packet.header_len() as usize..];

        Ok((ip_repr, payload))
    }

    /// Process a packet and dissect its UDP header.
    /// Returns the UDP header and the payload
    pub fn dissect_udp_header(
        ip_repr: Ipv4Repr,
        packet_data: &[u8],
    ) -> Result<(UdpRepr, &[u8]), &'static str> {
        let checksum_caps = ChecksumCapabilities::default();
        let udp_packet = UdpPacket::new_checked(packet_data)
            .map_err(|_| "Failed to parse IPv4 packet header")?;
        let udp_repr = UdpRepr::parse(
            &udp_packet,
            &ip_repr.src_addr.into(),
            &ip_repr.dst_addr.into(),
            &checksum_caps,
        )
        .map_err(|_| "Failed to parse IPv4 packet header")?;

        let payload = &packet_data[udp_repr.header_len()..];

        Ok((udp_repr, payload))
    }

    /// Process packet fragments and return the payload if it is complete.
    /// Returns None if not all fragments have been received
    pub fn process_fragments(
        &mut self,
        packet_data: &[u8],
    ) -> Result<(Ipv4Repr, Option<Vec<u8>>), &'static str> {
        let checksum_caps = ChecksumCapabilities::default();

        let ip_packet = Ipv4Packet::new_checked(packet_data)
            .map_err(|_| "Failed to parse IPv4 packet header")?;
        let ip_repr = Ipv4Repr::parse(&ip_packet, &checksum_caps)
            .map_err(|_| "Failed to parse IPv4 packet header")?;

        if ip_packet.more_frags() || ip_packet.frag_offset() != 0 {
            let src = ip_repr.src_addr.into();
            let dst = ip_repr.dst_addr.into();
            let ident = ip_packet.ident();

            // Store the fragment into the buffer
            let fragment_data = &packet_data[ip_packet.header_len() as usize..];
            let fragment_buffer = self.fragments.entry((src, dst, ident)).or_default();
            fragment_buffer.insert(ip_packet.frag_offset(), fragment_data.to_vec());

            // Update the assembler
            let (received_length, total_length) =
                self.assemblers.entry((src, dst, ident)).or_insert((0, 0));
            let fragment_len = fragment_data.len();
            *received_length += fragment_len;

            // Update total_length if this is the last fragment
            if !ip_packet.more_frags() {
                let new_total_length = ip_packet.frag_offset() as usize + fragment_len;
                if new_total_length > *total_length {
                    *total_length = new_total_length;
                }
            }

            // If all fragments have been received, reassemble and return the packet
            if *received_length == *total_length {
                let reassembled_fragments = self.fragments.remove(&(src, dst, ident)).unwrap();
                let mut reassembled = Vec::new();
                for (_, fragment) in reassembled_fragments {
                    reassembled.extend(fragment);
                }
                self.assemblers.remove(&(src, dst, ident));
                return Ok((ip_repr, Some(reassembled)));
            }
        }

        Ok((ip_repr, None))
    }
}

impl PacketCodec for PacketDecoder {
    type Item = PacketKind;

    fn decode(&mut self, packet: pcap::Packet) -> Self::Item {
        let args = (move |packet: &pcap::Packet| {
            let (eth_header, vlan_header, data) = Self::dissect_eth_header(packet).ok()?;
            let (ip_repr, data) = if Self::is_fragment(data).ok()? {
                let (ip_repr, data) = self.process_fragments(data).ok()?;
                (ip_repr, data?)
            } else {
                let (ip_repr, data) = Self::dissect_ip_header(data).ok()?;
                (ip_repr, data.to_vec())
            };
            // let (udp_repr, data) = Self::dissect_udp_header(ip_repr, &data).ok()?;

            let position = data.windows(4).position(|window| window == b"RTPS")?;
            Some((
                eth_header,
                vlan_header,
                ip_repr,
                // udp_repr,
                data.to_vec(),
                position,
            ))
        })(&packet);

        macro_rules! bail {
            () => {
                let PacketHeader { ts, caplen, len } = *packet.header;
                let ts = timeval_to_duration(ts);
                return PacketKind::Other(OtherPacket { ts, caplen, len });
            };
        }

        let Some((
            eth_header,
            vlan_header,
            ip_repr, // udp_repr,
            data,
            position,
        )) = args
        else {
            bail!();
        };

        let payload = data[position..].to_vec();
        if payload.get(0..4) != Some(b"RTPS") {
            bail!();
        }

        let bytes = Bytes::copy_from_slice(&payload);
        let message: Message = match Message::read_from_buffer(&bytes) {
            Ok(msg) => msg,
            Err(err) => {
                error!("error: {err:?}");
                bail!();
            }
        };

        RtpsPacket {
            headers: PacketHeaders {
                pcap_header: *packet.header,
                eth_header,
                vlan_header,
                ipv4_header: ip_repr,
                // udp_header: udp_repr,
                ts: timeval_to_duration(packet.header.ts),
            },
            message,
        }
        .into()
    }
}

pub enum PacketKind {
    Rtps(RtpsPacket),
    Other(OtherPacket),
}

impl PacketKind {
    pub fn ts(&self) -> chrono::Duration {
        match self {
            PacketKind::Rtps(packet) => packet.headers.ts,
            PacketKind::Other(packet) => packet.ts,
        }
    }
}

impl From<RtpsPacket> for PacketKind {
    fn from(v: RtpsPacket) -> Self {
        Self::Rtps(v)
    }
}

impl From<OtherPacket> for PacketKind {
    fn from(v: OtherPacket) -> Self {
        Self::Other(v)
    }
}

pub struct RtpsPacket {
    pub headers: PacketHeaders,
    pub message: Message,
}

pub struct OtherPacket {
    pub ts: chrono::Duration,
    pub caplen: u32,
    pub len: u32,
}

fn timeval_to_duration(ts: timeval) -> chrono::Duration {
    let timeval { tv_sec, tv_usec } = ts;
    chrono::Duration::microseconds(tv_sec * 1_000_000 + tv_usec)
}
