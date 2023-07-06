use rustdds::{discovery::data_types::topic_data::DiscoveredWriterData, SequenceNumber, GUID};
use std::collections::HashMap;

use crate::utils::DefragBuf;

/// The TUI state.
#[derive(Debug)]
pub(crate) struct State {
    pub entities: HashMap<GUID, EntityState>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            entities: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct EntityState {
    pub topic_info: Option<DiscoveredWriterData>,
    pub last_sn: Option<SequenceNumber>,
    pub frag_messages: HashMap<SequenceNumber, FragmentedMessage>,
    pub message_count: usize,
}

impl Default for EntityState {
    fn default() -> Self {
        Self {
            topic_info: None,
            frag_messages: HashMap::new(),
            last_sn: None,
            message_count: 0,
        }
    }
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
