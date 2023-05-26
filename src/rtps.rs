use crate::message::{DataEvent, DataFragEvent, RtpsEvent};
use anyhow::{anyhow, Result};
use bytes::Bytes;
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
use std::path::PathBuf;
use tracing::{error, warn};

pub fn rtps_watcher(source: PacketSource, tx: flume::Sender<RtpsEvent>) -> Result<()> {
    let iter = match source {
        PacketSource::Default => {
            let cap = Device::lookup()?
                .ok_or_else(|| anyhow!("no available network device"))?
                .open()?;
            MessageIter::from(cap.iter(PacketDecoder))
        }
        PacketSource::File(path) => {
            let cap = Capture::from_file(path)?;
            MessageIter::from(cap.iter(PacketDecoder))
        }
        PacketSource::Interface(interface) => {
            let cap = Device::list()?
                .into_iter()
                .find(|dev| dev.name == interface)
                .ok_or_else(|| anyhow!("unable to find network device {interface}"))?
                .open()?;
            MessageIter::from(cap.iter(PacketDecoder))
        }
    };

    'msg_loop: for msg in iter {
        let msg = msg?;

        let events = msg
            .submessages
            .iter()
            .filter_map(|submsg| submsg_to_event(&msg, submsg));

        for event in events {
            use flume::TrySendError as E;

            match tx.try_send(event) {
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

struct PacketDecoder;

impl PacketCodec for PacketDecoder {
    type Item = Option<Message>;

    fn decode(&mut self, packet: pcap::Packet) -> Self::Item {
        let position = packet
            .data
            .windows(4)
            .position(|window| window == b"RTPS")?;

        let payload = &packet.data[position..];
        if payload.get(0..4) != Some(b"RTPS") {
            return None;
        }

        let bytes = Bytes::copy_from_slice(payload);
        let message: Message = match Message::read_from_buffer(&bytes) {
            Ok(msg) => msg,
            Err(err) => {
                error!("error: {err:?}");
                return None;
            }
        };

        Some(message)
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
    type Item = Result<Message, pcap::Error>;

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
