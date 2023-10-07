use rbtree_defrag_buffer::DefragBuf;
use rustdds::{
    discovery::data_types::topic_data::{DiscoveredReaderData, DiscoveredWriterData},
    structure::guid::{EntityId, GuidPrefix},
    SequenceNumber, GUID,
};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    time::Instant,
};

/// The TUI state.
#[derive(Debug)]
pub(crate) struct State {
    pub participants: HashMap<GuidPrefix, ParticipantState>,
    pub topics: HashMap<String, TopicState>,
}

impl State {
    pub fn get_or_insert_entity(&mut self, guid: GUID) -> &mut EntityState {
        self.participants
            .entry(guid.prefix)
            .or_default()
            .entities
            .entry(guid.entity_id)
            .or_default()
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            participants: HashMap::new(),
            topics: HashMap::new(),
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
    pub recv_count: usize,
    pub since: Instant,
    pub topic_name: Option<String>,
    pub heartbeat: Option<HeartbeatState>,
}

impl EntityState {
    pub fn topic_name(&self) -> Option<&str> {
        let EntityContext::Writer(ctx) = &self.context else {
            return None;
        };

        let topic_name = &ctx.data.publication_topic_data.topic_name;
        Some(topic_name)
    }

    pub fn recv_bitrate(&self) -> f64 {
        let elapsed = self.since.elapsed();
        self.recv_count as f64 * 8.0 / elapsed.as_secs_f64()
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
            recv_count: 0,
            since: Instant::now(),
            topic_name: None,
            heartbeat: None,
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
pub struct TopicState {
    pub readers: HashSet<GUID>,
    pub writers: HashSet<GUID>,
}

#[derive(Debug)]
pub struct FragmentedMessage {
    pub data_size: usize,
    pub num_fragments: usize,
    pub recvd_fragments: usize,
    /// A range -> payload hash mapping
    pub intervals: HashMap<Range<usize>, u64>,
    pub defrag_buf: DefragBuf,
}

impl FragmentedMessage {
    pub fn new(data_size: usize, fragment_size: usize) -> Self {
        let num_fragments = (data_size + fragment_size - 1) / fragment_size;
        Self {
            data_size,
            num_fragments,
            recvd_fragments: 0,
            defrag_buf: DefragBuf::new(num_fragments),
            intervals: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FragmentInterval {
    pub range: Range<usize>,
    pub payload_hash: u64,
}

#[derive(Debug)]
pub struct HeartbeatState {
    pub first_sn: i64,
    pub last_sn: i64,
    pub count: i32,
    pub since: Instant,
}
