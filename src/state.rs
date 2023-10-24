use crate::logger::Logger;
use chrono::{DateTime, Local};
use rbtree_defrag_buffer::DefragBuf;
use rustdds::{
    discovery::data_types::topic_data::{DiscoveredReaderData, DiscoveredWriterData},
    structure::{
        guid::{EntityId, GuidPrefix},
        locator::Locator,
    },
    SequenceNumber, GUID,
};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    time::Instant,
};

/// The TUI state.
#[derive(Debug)]
pub struct State {
    pub tick_since: Instant,
    pub participants: HashMap<GuidPrefix, ParticipantState>,
    pub topics: HashMap<String, TopicState>,
    pub abnormalities: Vec<Abnormality>,
    pub stat: Statistics,
    pub logger: Option<Logger>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tick_since: Instant::now(),
            participants: HashMap::new(),
            topics: HashMap::new(),
            abnormalities: vec![],
            stat: Statistics::default(),
            logger: None,
        }
    }
}

#[derive(Debug)]
pub struct ParticipantState {
    pub writers: HashMap<EntityId, WriterState>,
    pub readers: HashMap<EntityId, ReaderState>,
    pub unicast_locator_list: Option<Vec<Locator>>,
    pub multicast_locator_list: Option<Vec<Locator>>,
}

impl Default for ParticipantState {
    fn default() -> Self {
        Self {
            writers: HashMap::new(),
            readers: HashMap::new(),
            unicast_locator_list: None,
            multicast_locator_list: None,
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

    pub fn type_name(&self) -> Option<&str> {
        let type_name = &self.data.as_ref()?.publication_topic_data.type_name;
        Some(type_name)
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
    pub data: Option<DiscoveredReaderData>,
    pub acknack: Option<AckNackState>,
    pub last_sn: Option<i64>,
    pub total_acknack_count: usize,
    pub acc_acknack_count: usize,
    pub avg_acknack_rate: f64,
}

impl ReaderState {
    pub fn topic_name(&self) -> Option<&str> {
        let topic_name = self.data.as_ref()?.subscription_topic_data.topic_name();
        Some(topic_name)
    }

    pub fn type_name(&self) -> Option<&str> {
        let type_name = self.data.as_ref()?.subscription_topic_data.type_name();
        Some(type_name)
    }
}

impl Default for ReaderState {
    fn default() -> Self {
        Self {
            last_sn: None,
            data: None,
            acknack: None,
            total_acknack_count: 0,
            acc_acknack_count: 0,
            avg_acknack_rate: 0.0,
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
    pub writer_guid: Option<GUID>,
    pub reader_guid: Option<GUID>,
    pub topic_name: Option<String>,
    pub desc: String,
}

#[derive(Debug)]
pub struct AckNackState {
    pub missing_sn: Vec<i64>,
    pub count: i32,
    pub since: Instant,
}

#[derive(Debug)]
pub struct Statistics {
    pub packet_count: usize,
    pub data_submsg_count: usize,
    pub datafrag_submsg_count: usize,
    pub acknack_submsg_count: usize,
    pub ackfrag_submsg_count: usize,
    pub heartbeat_submsg_count: usize,
    pub heartbeat_frag_submsg_count: usize,
}

impl Default for Statistics {
    fn default() -> Self {
        Self {
            packet_count: 0,
            data_submsg_count: 0,
            datafrag_submsg_count: 0,
            acknack_submsg_count: 0,
            ackfrag_submsg_count: 0,
            heartbeat_submsg_count: 0,
            heartbeat_frag_submsg_count: 0,
        }
    }
}
