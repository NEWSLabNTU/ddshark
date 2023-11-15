use crate::message::RtpsPacketHeaders;
use anyhow::bail;
use bytes::Bytes;
use etherparse::{
    Ethernet2Header, IpHeader, Ipv4Header, PacketHeaders, TransportHeader, UdpHeader, VlanHeader,
};
use libc::timeval;
use pcap::{PacketCodec, PacketHeader};
use rustdds::serialization::Message;
use std::{
    borrow::Cow,
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

    fn dissect_packet<'a>(&mut self, packet: &'a pcap::Packet) -> Dissection<'a> {
        let Ok(headers) = PacketHeaders::from_ethernet_slice(packet) else {
            return Dissection::NotSupported;
        };
        let PacketHeaders {
            link,
            vlan,
            ip,
            transport,
            payload,
        } = headers;

        let Some(IpHeader::Version4(ipv4, _)) = ip else {
            return Dissection::NotSupported;
        };

        let is_fragment = ipv4.more_fragments || ipv4.fragments_offset != 0;

        let (udp, defrag_payload) = if is_fragment {
            let payload = match self.process_fragments(&ipv4, payload) {
                Some(payload) => payload,
                None => {
                    return Dissection::Ipv4Fragment { link, vlan, ipv4 };
                }
            };
            let Ok((udp, payload)) = UdpHeader::from_slice(&payload) else {
                return Dissection::NotSupported;
            };
            (udp, Cow::Owned(payload.to_vec()))
        } else {
            let Some(TransportHeader::Udp(udp)) = transport else {
                return Dissection::NotSupported;
            };
            (udp, Cow::Borrowed(payload))
        };

        MaybeAssembledUdpPacket {
            link,
            vlan,
            ipv4,
            udp,
            payload: defrag_payload,
        }
        .into()
    }

    /// Process packet fragments and return the payload if it is complete.
    /// Returns None if not all fragments have been received
    fn process_fragments(&mut self, ipv4: &Ipv4Header, payload: &[u8]) -> Option<Vec<u8>> {
        let src = ipv4.source.into();
        let dst = ipv4.destination.into();
        let ident = ipv4.identification;

        // Store the fragment into the buffer
        let fragment_buffer = self.fragments.entry((src, dst, ident)).or_default();
        fragment_buffer.insert(ipv4.fragments_offset, payload.to_vec());

        // Update the assembler
        let (received_length, total_length) =
            self.assemblers.entry((src, dst, ident)).or_insert((0, 0));
        let fragment_len = payload.len();
        *received_length += fragment_len;

        // Update total_length if this is the last fragment
        if !ipv4.more_fragments {
            let new_total_length = ipv4.fragments_offset as usize + fragment_len;
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
            return Some(reassembled);
        }

        None
    }
}

impl PacketCodec for PacketDecoder {
    type Item = PacketKind;

    fn decode(&mut self, pcap_packet: pcap::Packet) -> Self::Item {
        macro_rules! bail {
            () => {{
                let PacketHeader { ts, caplen, len } = *pcap_packet.header;
                let ts = timeval_to_duration(ts);
                return PacketKind::Other(OtherPacket { ts, caplen, len });
            }};
        }

        let dissection = self.dissect_packet(&pcap_packet);
        let packet = match dissection {
            Dissection::NotSupported => bail!(),
            Dissection::Ipv4Fragment { .. } => bail!(),
            Dissection::UdpPacket(packet) => packet,
        };
        let MaybeAssembledUdpPacket {
            link,
            vlan,
            ipv4,
            udp,
            payload,
        } = packet;

        if !payload.starts_with(b"RTPS") {
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
            headers: RtpsPacketHeaders {
                pcap_header: *pcap_packet.header,
                link,
                vlan,
                ipv4,
                udp,
                ts: timeval_to_duration(pcap_packet.header.ts),
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
    pub headers: RtpsPacketHeaders,
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

enum Dissection<'a> {
    NotSupported,
    #[allow(unused)]
    Ipv4Fragment {
        link: Option<Ethernet2Header>,
        vlan: Option<VlanHeader>,
        ipv4: Ipv4Header,
    },
    UdpPacket(MaybeAssembledUdpPacket<'a>),
}

struct MaybeAssembledUdpPacket<'a> {
    pub link: Option<Ethernet2Header>,
    pub vlan: Option<VlanHeader>,
    pub ipv4: Ipv4Header,
    pub udp: UdpHeader,
    pub payload: Cow<'a, [u8]>,
}

impl<'a> From<MaybeAssembledUdpPacket<'a>> for Dissection<'a> {
    fn from(v: MaybeAssembledUdpPacket<'a>) -> Self {
        Self::UdpPacket(v)
    }
}
