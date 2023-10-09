use chrono::{DateTime, Local};
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
    pub tick_since: Instant,
    pub participants: HashMap<GuidPrefix, ParticipantState>,
    pub topics: HashMap<String, TopicState>,
    pub abnormalities: Vec<Abnormality>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tick_since: Instant::now(),
            participants: HashMap::new(),
            topics: HashMap::new(),
            abnormalities: vec![],
        }
    }
}

#[derive(Debug)]
pub struct ParticipantState {
    pub writers: HashMap<EntityId, WriterState>,
    pub readers: HashMap<EntityId, ReaderState>,
}

impl Default for ParticipantState {
    fn default() -> Self {
        Self {
            writers: HashMap::new(),
            readers: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct WriterState {
    pub last_sn: Option<SequenceNumber>,
    pub frag_messages: HashMap<SequenceNumber, FragmentedMessage>,
    pub total_msg_count: usize,
    pub total_byte_count: usize,
    pub acc_msg_count: usize,
    pub acc_byte_count: usize,
    pub avg_msgrate: f64,
    pub avg_bitrate: f64,
    pub heartbeat: Option<HeartbeatState>,
    pub data: Option<DiscoveredWriterData>,
}

impl WriterState {
    pub fn topic_name(&self) -> Option<&str> {
        let topic_name = &self.data.as_ref()?.publication_topic_data.topic_name;
        Some(topic_name)
    }
}

impl Default for WriterState {
    fn default() -> Self {
        Self {
            frag_messages: HashMap::new(),
            last_sn: None,
            acc_msg_count: 0,
            acc_byte_count: 0,
            heartbeat: None,
            total_msg_count: 0,
            total_byte_count: 0,
            avg_bitrate: 0.0,
            avg_msgrate: 0.0,
            data: None,
        }
    }
}

#[derive(Debug)]
pub struct ReaderState {
    pub last_sn: Option<SequenceNumber>,
    pub data: Option<DiscoveredReaderData>,
}

impl ReaderState {
    pub fn topic_name(&self) -> Option<&str> {
        let topic_name = self.data.as_ref()?.subscription_topic_data.topic_name();
        Some(topic_name)
    }
}

impl Default for ReaderState {
    fn default() -> Self {
        Self {
            last_sn: None,
            data: None,
        }
    }
}

#[derive(Debug)]
pub struct TopicState {
    pub readers: HashSet<GUID>,
    pub writers: HashSet<GUID>,
}

impl Default for TopicState {
    fn default() -> Self {
        Self {
            readers: HashSet::new(),
            writers: HashSet::new(),
        }
    }
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

#[derive(Debug)]
pub struct Abnormality {
    pub when: DateTime<Local>,
    pub writer_id: Option<GUID>,
    pub reader_id: Option<GUID>,
    pub topic_name: Option<String>,
    pub desc: String,
}
