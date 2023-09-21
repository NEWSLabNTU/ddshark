use super::PacketSource;
use crate::message::{
    DataEvent, DataFragEvent, DiscoveredReaderEvent, DiscoveredTopicEvent, DiscoveredWriterEvent,
    RtpsEvent, RtpsMessage,
};
use anyhow::Result;
use rustdds::{
    dds::{traits::serde_adapters::no_key::DeserializerAdapter, DiscoveredTopicData},
    discovery::data_types::topic_data::{DiscoveredReaderData, DiscoveredWriterData},
    messages::submessages::{
        submessage_elements::serialized_payload::SerializedPayload,
        submessages::{Data, DataFrag, EntitySubmessage, InterpreterSubmessage},
    },
    serialization::{
        pl_cdr_deserializer::{PlCdrDeserialize, PlCdrDeserializerAdapter},
        Message, SubMessage, SubmessageBody,
    },
    structure::{guid::EntityId, sequence_number::FragmentNumber},
    GUID,
};
use serde::Deserialize;
use tracing::warn;

pub fn rtps_watcher(source: PacketSource, tx: flume::Sender<RtpsMessage>) -> Result<()> {
    let iter = source.into_message_iter()?;

    'msg_loop: for msg in iter {
        let (headers, msg) = msg?;

        let events = msg
            .submessages
            .iter()
            .filter_map(|submsg| handle_submsg(&msg, submsg));

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

fn handle_submsg(msg: &Message, submsg: &SubMessage) -> Option<RtpsEvent> {
    match &submsg.body {
        SubmessageBody::Entity(emsg) => match emsg {
            EntitySubmessage::AckNack(_, _) => None,
            EntitySubmessage::Data(data, _) => handle_submsg_data(msg, submsg, data),
            EntitySubmessage::DataFrag(data, _) => {
                let event = handle_submsg_datafrag(msg, submsg, data);
                Some(event)
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

fn handle_submsg_data(msg: &Message, submsg: &SubMessage, data: &Data) -> Option<RtpsEvent> {
    let guid_prefix = msg.header.guid_prefix;

    let Data {
        reader_id,
        writer_id,
        writer_sn,
        ref inline_qos,
        ref serialized_payload,
    } = *data;

    let event = match writer_id {
        EntityId::SEDP_BUILTIN_TOPIC_WRITER => {
            let data: DiscoveredTopicData =
                deserialize_payload(serialized_payload.as_ref()?).ok()?;
            DiscoveredTopicEvent { data }.into()
        }
        EntityId::SEDP_BUILTIN_TOPIC_READER => {
            let data: DiscoveredTopicData =
                deserialize_payload(serialized_payload.as_ref()?).ok()?;
            DiscoveredTopicEvent { data }.into()
        }
        EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER => {
            let data: DiscoveredWriterData =
                deserialize_payload(serialized_payload.as_ref()?).ok()?;
            DiscoveredWriterEvent { data }.into()
        }
        EntityId::SEDP_BUILTIN_PUBLICATIONS_READER => {
            let data: DiscoveredWriterData =
                deserialize_payload(serialized_payload.as_ref()?).ok()?;
            DiscoveredWriterEvent { data }.into()
        }
        EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER => {
            let data: DiscoveredReaderData =
                deserialize_payload(serialized_payload.as_ref()?).ok()?;
            DiscoveredReaderEvent { data }.into()
        }
        EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER => {
            let data: DiscoveredReaderData =
                deserialize_payload(serialized_payload.as_ref()?).ok()?;
            DiscoveredReaderEvent { data }.into()
        }
        EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER => {
            return None;
        }
        EntityId::SPDP_BUILTIN_PARTICIPANT_READER => {
            return None;
        }
        EntityId::P2P_BUILTIN_PARTICIPANT_MESSAGE_WRITER => {
            return None;
        }
        EntityId::P2P_BUILTIN_PARTICIPANT_MESSAGE_READER => {
            return None;
        }
        _ => {
            let payload_size = match serialized_payload {
                Some(payload) => payload.value.len(),
                None => 0,
            };

            DataEvent {
                writer_id: GUID::new(guid_prefix, writer_id),
                reader_id: GUID::new(guid_prefix, reader_id),
                writer_sn,
                payload_size,
            }
            .into()
        }
    };

    Some(event)
}

fn handle_submsg_datafrag(msg: &Message, submsg: &SubMessage, data: &DataFrag) -> RtpsEvent {
    let guid_prefix = msg.header.guid_prefix;

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
    .into()
}

fn deserialize_payload<T>(
    payload: &SerializedPayload,
) -> Result<T, rustdds::serialization::error::Error>
where
    T: for<'de> Deserialize<'de> + PlCdrDeserialize,
{
    PlCdrDeserializerAdapter::from_bytes(payload.value.as_ref(), payload.representation_identifier)
}
