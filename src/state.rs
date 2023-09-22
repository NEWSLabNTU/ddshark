use rustdds::{
    discovery::data_types::topic_data::{DiscoveredReaderData, DiscoveredWriterData},
    structure::guid::{EntityId, GuidPrefix},
    SequenceNumber,
};
use std::collections::HashMap;

use crate::utils::DefragBuf;

/// The TUI state.
#[derive(Debug)]
pub(crate) struct State {
    pub participants: HashMap<GuidPrefix, ParticipantState>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            participants: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct ParticipantState {
    pub entities: HashMap<EntityId, EntityState>,
}

impl Default for ParticipantState {
    fn default() -> Self {
        Self {
            entities: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct EntityState {
    pub context: EntityContext,
    pub last_sn: Option<SequenceNumber>,
    pub frag_messages: HashMap<SequenceNumber, FragmentedMessage>,
    pub message_count: usize,
}

impl EntityState {
    pub fn topic_name(&self) -> Option<&str> {
        let EntityContext::Writer(ctx) = &self.context else {
            return None;
        };

        let topic_name = &ctx.data.publication_topic_data.topic_name;
        Some(topic_name)
    }
}

impl Default for EntityState {
    fn default() -> Self {
        Self {
            // topic_info: None,
            context: EntityContext::Unknown,
            frag_messages: HashMap::new(),
            last_sn: None,
            message_count: 0,
        }
    }
}

#[derive(Debug)]
pub enum EntityContext {
    Unknown,
    Writer(EntityWriterContext),
    Reader(EntityReaderContext),
}

impl From<EntityReaderContext> for EntityContext {
    fn from(v: EntityReaderContext) -> Self {
        Self::Reader(v)
    }
}

impl From<EntityWriterContext> for EntityContext {
    fn from(v: EntityWriterContext) -> Self {
        Self::Writer(v)
    }
}

impl EntityContext {
    /// Returns `true` if the entity context is [`Unknown`].
    ///
    /// [`Unknown`]: EntityContext::Unknown
    #[must_use]
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}

#[derive(Debug)]
pub struct EntityWriterContext {
    pub data: DiscoveredWriterData,
}

#[derive(Debug)]
pub struct EntityReaderContext {
    pub data: DiscoveredReaderData,
}

#[derive(Debug)]
pub struct FragmentedMessage {
    pub data_size: usize,
    pub num_fragments: usize,
    pub recvd_fragments: usize,
    pub free_intervals: DefragBuf,
}

impl FragmentedMessage {
    pub fn new(data_size: usize, fragment_size: usize) -> Self {
        let num_fragments = (data_size + fragment_size - 1) / fragment_size;
        Self {
            data_size,
            num_fragments,
            recvd_fragments: 0,
            free_intervals: DefragBuf::new(num_fragments),
        }
    }
}

#[derive(Debug)]
pub struct TopicStat {}
