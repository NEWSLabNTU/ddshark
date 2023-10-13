use super::{packet_decoder::RtpsPacket, PacketSource};
use crate::{
    message::{
        AckNackEvent, DataEvent, DataFragEvent, GapEvent, HeartbeatEvent, HeartbeatFragEvent,
        NackFragEvent, PacketHeaders, ParticipantInfo, RtpsSubmsgEvent, UpdateEvent,
    },
    utils::EntityIdExt,
};
use anyhow::Result;
use rustdds::{
    dds::{traits::serde_adapters::no_key::DeserializerAdapter, DiscoveredTopicData},
    discovery::data_types::{
        spdp_participant_data::SpdpDiscoveredParticipantData,
        topic_data::{DiscoveredReaderData, DiscoveredWriterData},
    },
    messages::{
        header::Header,
        protocol_version::ProtocolVersion,
        submessages::{
            submessage_elements::serialized_payload::SerializedPayload,
            submessages::{
                AckNack, Data, DataFrag, EntitySubmessage, Gap, Heartbeat, HeartbeatFrag,
                InfoDestination, InfoSource, InfoTimestamp, InterpreterSubmessage, NackFrag,
            },
        },
        vendor_id::VendorId,
    },
    serialization::{
        pl_cdr_deserializer::{PlCdrDeserialize, PlCdrDeserializerAdapter},
        SubMessage, SubmessageBody,
    },
    structure::{
        guid::{EntityId, GuidPrefix},
        locator::Locator,
        sequence_number::FragmentNumber,
    },
    SequenceNumber, Timestamp, GUID,
};
use serde::Deserialize;
use smoltcp::wire::{Ipv4Repr, UdpRepr};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    net::SocketAddrV4,
};
use tracing::{debug, error, warn};

struct Interpreter {
    src_version: ProtocolVersion,
    src_vendor_id: VendorId,
    src_guid_prefix: GuidPrefix,
    dst_guid_prefix: Option<GuidPrefix>,
    timestamp: Option<Timestamp>,
    unicast_locator_list: Option<Vec<Locator>>,
    multicast_locator_list: Option<Vec<Locator>>,
}

pub fn rtps_watcher(source: PacketSource, tx: flume::Sender<UpdateEvent>) -> Result<()> {
    let iter = source.into_message_iter()?;

    'msg_loop: for msg in iter {
        let RtpsPacket { headers, message } = msg?;

        let mut interpreter = {
            let Header {
                protocol_version,
                vendor_id,
                guid_prefix,
                ..
            } = message.header;
            let PacketHeaders {
                pcap_header,
                eth_header,
                vlan_header,
                ipv4_header: Ipv4Repr { src_addr, .. },
                // udp_header: UdpRepr { src_port, .. },
                ts,
            } = headers;
            let src_port = 0; // TODO: find correct UDP port
            assert_ne!(guid_prefix, GuidPrefix::UNKNOWN);

            // TODO: extract the UDP port
            let unicast_locator = Locator::UdpV4(SocketAddrV4::new(src_addr.0.into(), src_port));

            Interpreter {
                src_version: protocol_version,
                src_vendor_id: vendor_id,
                src_guid_prefix: guid_prefix,
                dst_guid_prefix: None,
                timestamp: None,
                unicast_locator_list: Some(vec![unicast_locator]),
                multicast_locator_list: None,
            }
        };

        let event: UpdateEvent = ParticipantInfo {
            guid_prefix: interpreter.src_guid_prefix,
            unicast_locator_list: interpreter.unicast_locator_list.as_ref().unwrap().clone(),
            multicast_locator_list: None,
        }
        .into();

        let mut events = vec![event];

        let event_iter = message
            .submessages
            .iter()
            .flat_map(|submsg| handle_submsg(&mut interpreter, submsg));
        events.extend(event_iter);

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

fn handle_submsg(interpreter: &mut Interpreter, submsg: &SubMessage) -> Vec<UpdateEvent> {
    match &submsg.body {
        SubmessageBody::Entity(emsg) => match emsg {
            EntitySubmessage::AckNack(data, _) => {
                let event = handle_submsg_acknack(interpreter, data);
                vec![event.into()]
            }
            EntitySubmessage::Data(data, _) => {
                let event = handle_submsg_data(interpreter, data);
                vec![event.into()]
            }
            EntitySubmessage::DataFrag(data, _) => {
                let event = handle_submsg_datafrag(interpreter, data);
                vec![event.into()]
            }
            EntitySubmessage::Gap(data, _) => {
                let event = handle_submsg_gap(interpreter, data);
                vec![event.into()]
            }
            EntitySubmessage::Heartbeat(data, _) => {
                let event = handle_submsg_heartbeat(interpreter, data);
                vec![event.into()]
            }
            EntitySubmessage::HeartbeatFrag(data, _) => {
                let event = handle_submsg_heartbeatfrag(interpreter, data);
                vec![event.into()]
            }
            EntitySubmessage::NackFrag(data, _) => {
                let event = handle_submsg_nackfrag(interpreter, data);
                vec![event.into()]
            }
        },
        SubmessageBody::Interpreter(imsg) => match imsg {
            InterpreterSubmessage::InfoSource(info, _) => {
                let InfoSource {
                    protocol_version,
                    vendor_id,
                    guid_prefix,
                } = *info;
                assert_ne!(guid_prefix, GuidPrefix::UNKNOWN);

                *interpreter = Interpreter {
                    src_version: protocol_version,
                    src_vendor_id: vendor_id,
                    src_guid_prefix: guid_prefix,
                    dst_guid_prefix: interpreter.dst_guid_prefix,
                    timestamp: None,
                    unicast_locator_list: None,
                    multicast_locator_list: None,
                };

                vec![]
            }
            InterpreterSubmessage::InfoDestination(info, _) => {
                let InfoDestination { guid_prefix } = *info;
                if guid_prefix != GuidPrefix::UNKNOWN {
                    interpreter.dst_guid_prefix = Some(guid_prefix);
                }
                vec![]
            }
            InterpreterSubmessage::InfoReply(info, _) => {
                interpreter.unicast_locator_list = Some(info.unicast_locator_list.clone());
                interpreter.multicast_locator_list = info.multicast_locator_list.clone();

                let event: UpdateEvent = ParticipantInfo {
                    guid_prefix: interpreter.src_guid_prefix,
                    unicast_locator_list: info.unicast_locator_list.clone(),
                    multicast_locator_list: info.multicast_locator_list.clone(),
                }
                .into();

                vec![event]
            }
            InterpreterSubmessage::InfoTimestamp(info, _) => {
                let InfoTimestamp { timestamp } = *info;

                if let Some(timestamp) = timestamp {
                    interpreter.timestamp = Some(timestamp);
                };

                vec![]
            }
        },
    }
}

fn handle_submsg_data(interpreter: &Interpreter, data: &Data) -> RtpsSubmsgEvent {
    let Data {
        writer_id,
        writer_sn,
        inline_qos: _,
        ref serialized_payload,
        ..
    } = *data;
    let writer_guid = GUID::new(interpreter.src_guid_prefix, writer_id);

    let payload_size = match serialized_payload {
        Some(payload) => payload.value.len(),
        None => 0,
    };

    let payload = (|| {
        macro_rules! bail {
            () => {
                debug!(
                    "payload deserialization is not implemented for {}",
                    writer_id.display()
                );
                return None;
            };
        }
        let serialized_payload = serialized_payload.as_ref();

        let payload = match writer_id {
            EntityId::SEDP_BUILTIN_TOPIC_WRITER => {
                let data: DiscoveredTopicData = deserialize_payload(writer_id, serialized_payload)?;
                data.into()
            }
            EntityId::SEDP_BUILTIN_TOPIC_READER => {
                let data: DiscoveredTopicData = deserialize_payload(writer_id, serialized_payload)?;
                data.into()
            }
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER => {
                let data: DiscoveredWriterData =
                    deserialize_payload(writer_id, serialized_payload)?;
                data.into()
            }
            EntityId::SEDP_BUILTIN_PUBLICATIONS_READER => {
                let data: DiscoveredWriterData =
                    deserialize_payload(writer_id, serialized_payload)?;
                data.into()
            }
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER => {
                let data: DiscoveredReaderData =
                    deserialize_payload(writer_id, serialized_payload)?;
                data.into()
            }
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER => {
                let data: DiscoveredReaderData =
                    deserialize_payload(writer_id, serialized_payload)?;
                data.into()
            }
            EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER => {
                let data: SpdpDiscoveredParticipantData =
                    deserialize_payload(writer_id, serialized_payload)?;
                data.into()
            }
            EntityId::SPDP_BUILTIN_PARTICIPANT_READER => {
                let data: SpdpDiscoveredParticipantData =
                    deserialize_payload(writer_id, serialized_payload)?;
                data.into()
            }
            EntityId::P2P_BUILTIN_PARTICIPANT_MESSAGE_WRITER => {
                bail!();
            }
            EntityId::P2P_BUILTIN_PARTICIPANT_MESSAGE_READER => {
                bail!();
            }
            _ => return None,
        };

        Some(payload)
    })();

    DataEvent {
        writer_guid,
        writer_sn,
        payload_size,
        payload,
    }
    .into()
}

fn handle_submsg_datafrag(interpreter: &Interpreter, data: &DataFrag) -> RtpsSubmsgEvent {
    let DataFrag {
        writer_id,
        writer_sn,
        fragment_starting_num: FragmentNumber(fragment_starting_num),
        fragments_in_submessage,
        data_size,
        fragment_size,
        ref serialized_payload,
        ..
    } = *data;
    let writer_guid = GUID::new(interpreter.src_guid_prefix, writer_id);
    let payload_size = serialized_payload.len();

    fn calculate_hash<T: Hash>(t: &T) -> u64 {
        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        s.finish()
    }

    let payload_hash = calculate_hash(serialized_payload);

    // println!(
    //     "datafrag {}\t\
    //      start={fragment_starting_num}\t\
    //      n_msgs={fragments_in_submessage}\t\
    //      data_size={data_size}\t\
    //      frag_size={fragment_size}\t\
    //      payload_size={payload_size}",
    //     writer_id.display()
    // );

    DataFragEvent {
        writer_guid,
        writer_sn,
        fragment_starting_num,
        fragments_in_submessage,
        data_size,
        fragment_size,
        payload_size,
        payload_hash,
    }
    .into()
}

fn handle_submsg_gap(interpreter: &Interpreter, data: &Gap) -> RtpsSubmsgEvent {
    let Gap {
        reader_id,
        writer_id,
        gap_start,
        ref gap_list,
    } = *data;
    let writer_guid = GUID::new(interpreter.src_guid_prefix, writer_id);
    let reader_guid = GUID::new(interpreter.dst_guid_prefix.unwrap(), reader_id); // TODO: warn if dst_guid_prefix is not set

    // println!("gap {}", writer_id.display());

    GapEvent {
        writer_guid,
        reader_guid,
        gap_start,
        gap_list: gap_list.clone(),
    }
    .into()
}

fn handle_submsg_nackfrag(interpreter: &Interpreter, data: &NackFrag) -> RtpsSubmsgEvent {
    let NackFrag {
        reader_id,
        writer_id,
        writer_sn,
        // ref fragment_number_state,
        count,
        ..
    } = *data;
    let writer_guid = GUID::new(interpreter.dst_guid_prefix.unwrap(), writer_id); // TODO: warn if dst_guid_prefix is not set
    let reader_guid = GUID::new(interpreter.src_guid_prefix, reader_id);

    // println!("nack {}\t{fragment_number_state:?}", writer_id.display());

    // let nums: Vec<_> = fragment_number_state
    //     .iter()
    //     .map(|FragmentNumber(n)| n)
    //     .collect();
    // println!("nack_frag {} {:?}", writer_id.display(), nums);

    NackFragEvent {
        writer_guid,
        reader_guid,
        writer_sn,
        count,
    }
    .into()
}

fn handle_submsg_heartbeat(interpreter: &Interpreter, data: &Heartbeat) -> RtpsSubmsgEvent {
    let Heartbeat {
        writer_id,
        first_sn,
        last_sn,
        count,
        ..
    } = *data;
    let writer_guid = GUID::new(interpreter.src_guid_prefix, writer_id);

    // println!("heartbeat {}\t{first_sn}\t{last_sn}", writer_id.display());

    HeartbeatEvent {
        writer_guid,
        first_sn,
        last_sn,
        count,
    }
    .into()
}

fn handle_submsg_heartbeatfrag(interpreter: &Interpreter, data: &HeartbeatFrag) -> RtpsSubmsgEvent {
    let HeartbeatFrag {
        writer_id,
        writer_sn,
        last_fragment_num,
        count,
        ..
    } = *data;
    let writer_guid = GUID::new(interpreter.src_guid_prefix, writer_id);

    // println!(
    //     "heartbeat_frag {}\t{last_fragment_num}",
    //     writer_id.display()
    // );

    HeartbeatFragEvent {
        writer_guid,
        writer_sn,
        last_fragment_num,
        count,
    }
    .into()
}

fn handle_submsg_acknack(interpreter: &Interpreter, data: &AckNack) -> RtpsSubmsgEvent {
    let AckNack {
        writer_id,
        reader_id,
        ref reader_sn_state,
        count,
        ..
    } = *data;

    let writer_guid = GUID::new(interpreter.dst_guid_prefix.unwrap(), writer_id); // TODO: warn if dst_guid_prefix is not set
    let reader_guid = GUID::new(interpreter.src_guid_prefix, reader_id);
    let base_sn = reader_sn_state.base().0;
    let missing_sn: Vec<_> = reader_sn_state
        .iter()
        .map(|SequenceNumber(sn)| sn)
        .collect();

    // println!("ack_nack {}\t{reader_sn_state:?}", writer_id.display());

    AckNackEvent {
        writer_guid,
        reader_guid,
        count,
        missing_sn,
        base_sn,
    }
    .into()
}

fn deserialize_payload<T>(entity_id: EntityId, payload: Option<&SerializedPayload>) -> Option<T>
where
    T: for<'de> Deserialize<'de> + PlCdrDeserialize,
{
    let Some(payload) = payload else {
        error!("no payload found for entity {}", entity_id.display());
        return None;
    };
    let result = PlCdrDeserializerAdapter::from_bytes(
        payload.value.as_ref(),
        payload.representation_identifier,
    );
    let data = match result {
        Ok(data) => data,
        Err(err) => {
            error!(
                "fail to parse payload for entity {}: {err}",
                entity_id.display()
            );
            return None;
        }
    };
    Some(data)
}
