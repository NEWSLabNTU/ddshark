use crate::message::RtpsPacketHeaders;
use bytes::Bytes;
use etherparse::{Ipv4Header, NetHeaders, PacketHeaders, TransportHeader, UdpHeader};
use libc::timeval;
use pcap::{PacketCodec, PacketHeader};
use rustdds::rtps::Message;
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    net::Ipv4Addr,
};
use tracing::error;

pub struct PacketDecoder {
    /// Map of (source, destination, ip-id) to fragments keyed by byte offset.
    fragments: HashMap<(Ipv4Addr, Ipv4Addr, u16), BTreeMap<usize, Vec<u8>>>,
    /// Map of (source, destination, ip-id) to the known total datagram length.
    /// 0 until the last fragment (more_fragments = false) reveals the size.
    assemblers: HashMap<(Ipv4Addr, Ipv4Addr, u16), usize>,
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
            net,
            transport,
            payload,
            ..
        } = headers;

        let Some(NetHeaders::Ipv4(ipv4, _)) = net else {
            return Dissection::NotSupported;
        };

        let is_fragment = ipv4.more_fragments || ipv4.fragment_offset.value() != 0;

        let (udp, defrag_payload) = if is_fragment {
            let payload = match self.process_fragments(&ipv4, payload.slice()) {
                Some(payload) => payload,
                None => {
                    return Dissection::Ipv4Fragment { ipv4 };
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
            (udp, Cow::Borrowed(payload.slice()))
        };

        MaybeAssembledUdpPacket {
            ipv4,
            udp,
            payload: defrag_payload,
        }
        .into()
    }

    /// Process packet fragments and return the payload if it is complete.
    /// Returns None if not all fragments have been received yet, or if the fragments
    /// seen so far do not form a contiguous, non-overlapping range.
    fn process_fragments(&mut self, ipv4: &Ipv4Header, payload: &[u8]) -> Option<Vec<u8>> {
        let key = (
            ipv4.source.into(),
            ipv4.destination.into(),
            ipv4.identification,
        );

        // The IP fragment offset field counts 8-octet units, not bytes.
        let byte_offset = (ipv4.fragment_offset.value() as usize) * 8;
        let fragment_len = payload.len();

        // Store the fragment keyed by its byte offset. Ignore duplicates /
        // retransmissions so they don't corrupt the contiguity accounting.
        let fragment_buffer = self.fragments.entry(key).or_default();
        fragment_buffer
            .entry(byte_offset)
            .or_insert_with(|| payload.to_vec());

        // The last fragment (more_fragments = false) reveals the total length.
        let total_length = self.assemblers.entry(key).or_insert(0);
        if !ipv4.more_fragments {
            *total_length = byte_offset + fragment_len;
        }
        let total_length = *total_length;
        if total_length == 0 {
            // Haven't seen the last fragment yet, so the size is unknown.
            return None;
        }

        // Complete only when fragments cover [0, total_length) with no gap or overlap.
        let fragment_buffer = &self.fragments[&key];
        let mut expected = 0usize;
        for (&offset, data) in fragment_buffer {
            if offset != expected {
                // Gap or overlap: not (yet) a clean contiguous datagram.
                return None;
            }
            expected += data.len();
        }
        if expected != total_length {
            return None;
        }

        // Reassemble in offset order.
        let fragment_buffer = self.fragments.remove(&key).unwrap();
        self.assemblers.remove(&key);
        let mut reassembled = Vec::with_capacity(total_length);
        for (_, fragment) in fragment_buffer {
            reassembled.extend(fragment);
        }
        Some(reassembled)
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
        let MaybeAssembledUdpPacket { ipv4, udp, payload } = packet;

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
        ipv4: Ipv4Header,
    },
    UdpPacket(MaybeAssembledUdpPacket<'a>),
}

struct MaybeAssembledUdpPacket<'a> {
    pub ipv4: Ipv4Header,
    pub udp: UdpHeader,
    pub payload: Cow<'a, [u8]>,
}

impl<'a> From<MaybeAssembledUdpPacket<'a>> for Dissection<'a> {
    fn from(v: MaybeAssembledUdpPacket<'a>) -> Self {
        Self::UdpPacket(v)
    }
}
