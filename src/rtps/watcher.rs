use super::PacketSource;
use crate::{
    message::{DataEvent, DataFragEvent, RtpsEvent, RtpsMessage},
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
use tracing::{error, warn};
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
            EntitySubmessage::Data(data, _) => {
                let event = handle_submsg_data(msg, submsg, data);
                Some(event)
            }
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

fn handle_submsg_data(msg: &Message, _submsg: &SubMessage, data: &Data) -> RtpsEvent {
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
                error!(
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

fn handle_submsg_datafrag(msg: &Message, _submsg: &SubMessage, data: &DataFrag) -> RtpsEvent {
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
