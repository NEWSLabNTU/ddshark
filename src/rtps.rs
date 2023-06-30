use crate::message::{DataEvent, DataFragEvent, PacketHeaders, RtpsEvent, RtpsMessage};
use anyhow::{anyhow, Result};
use bytes::Bytes;
use etherparse::{Ethernet2Header, SingleVlanHeader};
use pcap::{Capture, Device, PacketCodec, PacketIter};
use rustdds::{
    dds::traits::serde_adapters::no_key::DeserializerAdapter,
    discovery::data_types::topic_data::DiscoveredWriterData,
    messages::submessages::submessages::{Data, DataFrag, EntitySubmessage, InterpreterSubmessage},
    serialization::{
        pl_cdr_deserializer::PlCdrDeserializerAdapter, Message, SubMessage, SubmessageBody,
    },
    structure::{guid::EntityId, sequence_number::FragmentNumber},
    GUID,
};
use smoltcp::{
    phy::ChecksumCapabilities,
    wire::{Ipv4Packet, Ipv4Repr},
};
use std::{
    collections::{BTreeMap, HashMap},
    net::Ipv4Addr,
    path::PathBuf,
};
use tracing::{error, warn};

pub fn rtps_watcher(source: PacketSource, tx: flume::Sender<RtpsMessage>) -> Result<()> {
    let iter = match source {
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

    'msg_loop: for msg in iter {
        let (headers, msg) = msg?;

        let events = msg
            .submessages
            .iter()
            .filter_map(|submsg| submsg_to_event(&msg, submsg));

        for event in events {
            use flume::TrySendError as E;

            match tx.try_send(RtpsMessage {
                headers: headers.clone(),
                event,
            }) {
                Ok(()) => {}
                Err(E::Disconnected(_)) => break 'msg_loop,
                Err(E::Full(_)) => {
                    warn!("channel is full");
                    continue;
                }
            }
        }
    }

    Ok(())
}

fn submsg_to_event(msg: &Message, submsg: &SubMessage) -> Option<RtpsEvent> {
    let guid_prefix = msg.header.guid_prefix;

    match &submsg.body {
        SubmessageBody::Entity(emsg) => match emsg {
            EntitySubmessage::AckNack(_, _) => None,
            EntitySubmessage::Data(data, _) => {
                let Data {
                    reader_id,
                    writer_id,
                    writer_sn,
                    ref inline_qos,
                    ref serialized_payload,
                } = *data;
                let payload_size = match serialized_payload {
                    Some(payload) => payload.value.len(),
                    None => 0,
                };

                let discovery_data: Option<DiscoveredWriterData> = match writer_id {
                    EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER => {
                        PlCdrDeserializerAdapter::from_bytes(
                            serialized_payload.as_ref()?.value.as_ref(),
                            serialized_payload.as_ref()?.representation_identifier,
                        )
                        .map_err(|e| {
                            error!("unable to deserialize discovery data: {:?}", e);
                        })
                        .ok()
                    }
                    _ => None,
                };

                Some(
                    DataEvent {
                        writer_id: GUID::new(guid_prefix, writer_id),
                        reader_id: GUID::new(guid_prefix, reader_id),
                        writer_sn,
                        payload_size,
                        discovery_data,
                    }
                    .into(),
                )
            }
            EntitySubmessage::DataFrag(data, _) => {
                let DataFrag {
                    reader_id,
                    writer_id,
                    writer_sn,
                    fragment_starting_num: FragmentNumber(fragment_starting_num),
                    fragments_in_submessage,
                    data_size,
                    fragment_size,
                    ref inline_qos,
                    ref serialized_payload,
                } = *data;
                let payload_size = serialized_payload.len();

                Some(
                    DataFragEvent {
                        writer_id: GUID::new(guid_prefix, writer_id),
                        reader_id: GUID::new(guid_prefix, reader_id),
                        writer_sn,
                        fragment_starting_num,
                        fragments_in_submessage,
                        data_size,
                        fragment_size,
                        payload_size,
                    }
                    .into(),
                )
            }
            EntitySubmessage::Gap(_, _) => None,
            EntitySubmessage::Heartbeat(_, _) => None,
            EntitySubmessage::HeartbeatFrag(_, _) => None,
            EntitySubmessage::NackFrag(_, _) => None,
        },
        SubmessageBody::Interpreter(imsg) => match *imsg {
            InterpreterSubmessage::InfoSource(_, _) => None,
            InterpreterSubmessage::InfoDestination(_, _) => None,
            InterpreterSubmessage::InfoReply(_, _) => None,
            InterpreterSubmessage::InfoTimestamp(_, _) => None,
        },
    }
}

struct PacketDecoder {
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

        match etherparse::EtherType::from_u16(eth_header.ether_type) {
            Some(etherparse::EtherType::VlanTaggedFrame) => {
                let (vlan_header, remaining_data) = SingleVlanHeader::from_slice(data)
                    .map_err(|_| "Failed to parse VLAN header")?;
                Ok((eth_header, Some(vlan_header), remaining_data))
            }
            Some(etherparse::EtherType::Ipv4) => Ok((eth_header, None, data)),
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
    pub fn dissect_ip_header<'a>(
        packet_data: &'a [u8],
    ) -> Result<(Ipv4Repr, &'a [u8]), &'static str> {
        let checksum_caps = ChecksumCapabilities::default();

        let ip_packet = Ipv4Packet::new_checked(packet_data)
            .map_err(|_| "Failed to parse IPv4 packet header")?;
        let ip_repr = Ipv4Repr::parse(&ip_packet, &checksum_caps)
            .map_err(|_| "Failed to parse IPv4 packet header")?;

        let payload = &packet_data[ip_packet.header_len() as usize..];

        Ok((ip_repr, payload))
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
            let fragment_buffer = self
                .fragments
                .entry((src, dst, ident))
                .or_insert_with(BTreeMap::new);
            fragment_buffer.insert(ip_packet.frag_offset() as u16, fragment_data.to_vec());

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
    type Item = Option<(PacketHeaders, Message)>;

    fn decode(&mut self, packet: pcap::Packet) -> Self::Item {
        let (eth_header, vlan_header, packet_data) = Self::dissect_eth_header(&packet).ok()?;
        let (ip_repr, data) = match Self::is_fragment(packet_data).ok()? {
            false => {
                let (ip_repr, data) = Self::dissect_ip_header(packet_data).ok()?;
                (ip_repr, data.to_vec())
            }
            true => {
                let (ip_repr, data) = self.process_fragments(packet_data).ok()?;
                (ip_repr, data?)
            }
        };

        let position = data.windows(4).position(|window| window == b"RTPS")?;

        let payload = data[position..].to_vec();
        if payload.get(0..4) != Some(b"RTPS") {
            return None;
        }

        let bytes = Bytes::copy_from_slice(&payload);
        let message: Message = match Message::read_from_buffer(&bytes) {
            Ok(msg) => msg,
            Err(err) => {
                error!("error: {err:?}");
                return None;
            }
        };

        Some((
            PacketHeaders {
                pcap_header: packet.header.clone(),
                eth_header: eth_header,
                vlan_header: vlan_header,
                ipv4_header: ip_repr,
            },
            message,
        ))
    }
}

#[derive(Debug)]
pub enum PacketSource {
    Default,
    File(PathBuf),
    Interface(String),
}

enum MessageIter {
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
