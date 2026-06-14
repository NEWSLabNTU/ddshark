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
    time::{Duration, Instant},
};
use tracing::{error, warn};

/// Drop a partial IP reassembly that has not completed within this window.
/// (RFC 791 suggests an IP reassembly timeout in the 15–120 s range.)
const REASSEMBLY_TTL: Duration = Duration::from_secs(30);
/// Hard cap on concurrent partial reassemblies, to bound memory against a
/// hostile or churning IP-id space. Oldest are evicted past this.
const MAX_REASSEMBLIES: usize = 4096;

type FragmentKey = (Ipv4Addr, Ipv4Addr, u16);

/// A partially-received IP datagram being reassembled.
struct Reassembly {
    /// Fragments keyed by byte offset.
    parts: BTreeMap<usize, Vec<u8>>,
    /// Total datagram length; 0 until the last fragment reveals it.
    total_length: usize,
    /// When the first fragment of this datagram was seen (for TTL eviction).
    first_seen: Instant,
}

pub struct PacketDecoder {
    /// Map of (source, destination, ip-id) to its in-progress reassembly.
    reassemblies: HashMap<FragmentKey, Reassembly>,
}

impl PacketDecoder {
    pub fn new() -> Self {
        PacketDecoder {
            reassemblies: HashMap::new(),
        }
    }

    /// Drop reassemblies older than the TTL, then enforce the count cap by
    /// evicting the oldest. Keeps partial-fragment memory bounded (issue 009).
    fn evict_stale(&mut self, now: Instant) {
        let before = self.reassemblies.len();
        self.reassemblies
            .retain(|_, r| now.duration_since(r.first_seen) <= REASSEMBLY_TTL);

        while self.reassemblies.len() > MAX_REASSEMBLIES {
            if let Some(oldest) = self
                .reassemblies
                .iter()
                .min_by_key(|(_, r)| r.first_seen)
                .map(|(k, _)| *k)
            {
                self.reassemblies.remove(&oldest);
            } else {
                break;
            }
        }

        let dropped = before.saturating_sub(self.reassemblies.len());
        if dropped > 0 {
            warn!("evicted {dropped} stale/overflowing IP reassembly buffers");
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
        let now = Instant::now();
        let key = (
            ipv4.source.into(),
            ipv4.destination.into(),
            ipv4.identification,
        );

        // The IP fragment offset field counts 8-octet units, not bytes.
        let byte_offset = (ipv4.fragment_offset.value() as usize) * 8;
        let fragment_len = payload.len();

        {
            let entry = self.reassemblies.entry(key).or_insert_with(|| Reassembly {
                parts: BTreeMap::new(),
                total_length: 0,
                first_seen: now,
            });
            // Store the fragment keyed by its byte offset. Ignore duplicates /
            // retransmissions so they don't corrupt the contiguity accounting.
            entry
                .parts
                .entry(byte_offset)
                .or_insert_with(|| payload.to_vec());
            // The last fragment (more_fragments = false) reveals the total length.
            if !ipv4.more_fragments {
                entry.total_length = byte_offset + fragment_len;
            }
        }

        // Complete only when fragments cover [0, total_length) with no gap or overlap.
        let entry = &self.reassemblies[&key];
        let complete = entry.total_length != 0 && {
            let mut expected = 0usize;
            let contiguous = entry.parts.iter().all(|(&offset, data)| {
                let ok = offset == expected;
                expected += data.len();
                ok
            });
            contiguous && expected == entry.total_length
        };

        if !complete {
            self.evict_stale(now);
            return None;
        }

        // Reassemble in offset order.
        let entry = self.reassemblies.remove(&key).unwrap();
        let mut reassembled = Vec::with_capacity(entry.total_length);
        for (_, fragment) in entry.parts {
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
