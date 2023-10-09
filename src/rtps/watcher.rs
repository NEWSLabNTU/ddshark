use super::{packet_decoder::RtpsPacket, PacketSource};
use crate::{
    message::{
        AckNackEvent, DataEvent, DataFragEvent, GapEvent, HeartbeatEvent, HeartbeatFragEvent,
        NackFragEvent, RtpsContext, RtpsEvent, UpdateEvent,
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
    messages::submessages::{
        submessage_elements::serialized_payload::SerializedPayload,
        submessages::{
            AckNack, Data, DataFrag, EntitySubmessage, Gap, Heartbeat, HeartbeatFrag,
            InterpreterSubmessage, NackFrag,
        },
    },
    serialization::{
        pl_cdr_deserializer::{PlCdrDeserialize, PlCdrDeserializerAdapter},
        Message, SubMessage, SubmessageBody,
    },
    structure::{guid::EntityId, sequence_number::FragmentNumber},
    GUID,
};
use serde::Deserialize;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use tracing::{debug, error, warn};

pub fn rtps_watcher(source: PacketSource, tx: flume::Sender<UpdateEvent>) -> Result<()> {
    let iter = source.into_message_iter()?;

    'msg_loop: for msg in iter {
        let RtpsPacket { headers, message } = msg?;

        let events = message
            .submessages
            .iter()
            .filter_map(|submsg| handle_submsg(&message, submsg));

        for event in events {
            use flume::TrySendError as E;

            let msg = UpdateEvent::Rtps(RtpsEvent {
                headers: headers.clone(),
                event,
            });

            match tx.try_send(msg) {
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

fn handle_submsg(msg: &Message, submsg: &SubMessage) -> Option<RtpsContext> {
    match &submsg.body {
        SubmessageBody::Entity(emsg) => match emsg {
            EntitySubmessage::AckNack(data, _) => {
                let event = handle_submsg_ack_nack(msg, submsg, data);
                Some(event)
            }
            EntitySubmessage::Data(data, _) => {
                let event = handle_submsg_data(msg, submsg, data);
                Some(event)
            }
            EntitySubmessage::DataFrag(data, _) => {
                let event = handle_submsg_datafrag(msg, submsg, data);
                Some(event)
            }
            EntitySubmessage::Gap(data, _) => {
                let event = handle_submsg_gap(msg, submsg, data);
                Some(event)
            }
            EntitySubmessage::Heartbeat(data, _) => {
                let event = handle_submsg_heartbeat(msg, submsg, data);
                Some(event)
            }
            EntitySubmessage::HeartbeatFrag(data, _) => {
                let event = handle_submsg_heartbeat_frag(msg, submsg, data);
                Some(event)
            }
            EntitySubmessage::NackFrag(data, _) => {
                let event = handle_submsg_nack_frag(msg, submsg, data);
                Some(event)
            }
        },
        SubmessageBody::Interpreter(imsg) => match *imsg {
            InterpreterSubmessage::InfoSource(_, _) => None,
            InterpreterSubmessage::InfoDestination(_, _) => None,
            InterpreterSubmessage::InfoReply(_, _) => None,
            InterpreterSubmessage::InfoTimestamp(_, _) => None,
        },
    }
}

fn handle_submsg_data(msg: &Message, _submsg: &SubMessage, data: &Data) -> RtpsContext {
    let guid_prefix = msg.header.guid_prefix;

    let Data {
        reader_id,
        writer_id,
        writer_sn,
        inline_qos: _,
        ref serialized_payload,
    } = *data;

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
        writer_id: GUID::new(guid_prefix, writer_id),
        reader_id: GUID::new(guid_prefix, reader_id),
        writer_sn,
        payload_size,
        payload,
    }
    .into()
}

fn handle_submsg_datafrag(msg: &Message, _submsg: &SubMessage, data: &DataFrag) -> RtpsContext {
    let guid_prefix = msg.header.guid_prefix;

    let DataFrag {
        reader_id,
        writer_id,
        writer_sn,
        fragment_starting_num: FragmentNumber(fragment_starting_num),
        fragments_in_submessage,
        data_size,
        fragment_size,
        inline_qos: _,
        ref serialized_payload,
    } = *data;
    let writer_id = GUID::new(guid_prefix, writer_id);
    let reader_id = GUID::new(guid_prefix, reader_id);
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
        writer_id,
        reader_id,
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

fn handle_submsg_gap(msg: &Message, _submsg: &SubMessage, data: &Gap) -> RtpsContext {
    let guid_prefix = msg.header.guid_prefix;
    let Gap {
        reader_id,
        writer_id,
        gap_start,
        ref gap_list,
    } = *data;
    let writer_id = GUID::new(guid_prefix, writer_id);
    let reader_id = GUID::new(guid_prefix, reader_id);

    // println!("gap {}", writer_id.display());

    GapEvent {
        writer_id,
        reader_id,
        gap_start,
        gap_list: gap_list.clone(),
    }
    .into()
}

fn handle_submsg_nack_frag(msg: &Message, _submsg: &SubMessage, data: &NackFrag) -> RtpsContext {
    let guid_prefix = msg.header.guid_prefix;
    let NackFrag {
        reader_id,
        writer_id,
        writer_sn,
        // ref fragment_number_state,
        count,
        ..
    } = *data;
    let writer_id = GUID::new(guid_prefix, writer_id);
    let reader_id = GUID::new(guid_prefix, reader_id);

    // println!("nack {}\t{fragment_number_state:?}", writer_id.display());

    // let nums: Vec<_> = fragment_number_state
    //     .iter()
    //     .map(|FragmentNumber(n)| n)
    //     .collect();
    // println!("nack_frag {} {:?}", writer_id.display(), nums);

    NackFragEvent {
        writer_id,
        reader_id,
        writer_sn,
        count,
    }
    .into()
}

fn handle_submsg_heartbeat(msg: &Message, _submsg: &SubMessage, data: &Heartbeat) -> RtpsContext {
    let guid_prefix = msg.header.guid_prefix;
    let Heartbeat {
        writer_id,
        first_sn,
        last_sn,
        count,
        ..
    } = *data;
    let writer_id = GUID::new(guid_prefix, writer_id);

    // println!("heartbeat {}\t{first_sn}\t{last_sn}", writer_id.display());

    HeartbeatEvent {
        writer_id,
        first_sn,
        last_sn,
        count,
    }
    .into()
}

fn handle_submsg_heartbeat_frag(
    msg: &Message,
    _submsg: &SubMessage,
    data: &HeartbeatFrag,
) -> RtpsContext {
    let guid_prefix = msg.header.guid_prefix;
    let HeartbeatFrag {
        reader_id,
        writer_id,
        writer_sn,
        last_fragment_num,
        count,
    } = *data;
    let writer_id = GUID::new(guid_prefix, writer_id);
    // let reader_id = GUID::new(guid_prefix, reader_id);

    // println!(
    //     "heartbeat_frag {}\t{last_fragment_num}",
    //     writer_id.display()
    // );

    HeartbeatFragEvent {
        writer_id,
        writer_sn,
        last_fragment_num,
        count,
    }
    .into()
}

fn handle_submsg_ack_nack(msg: &Message, _submsg: &SubMessage, data: &AckNack) -> RtpsContext {
    let guid_prefix = msg.header.guid_prefix;
    let AckNack {
        reader_id,
        writer_id,
        // ref reader_sn_state,
        count,
        ..
    } = *data;
    let writer_id = GUID::new(guid_prefix, writer_id);
    let reader_id = GUID::new(guid_prefix, reader_id);

    // println!("ack_nack {}\t{reader_sn_state:?}", writer_id.display());

    AckNackEvent {
        writer_id,
        reader_id,
        count,
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
