use rust_lapper::Lapper;
use rustdds::{discovery::data_types::topic_data::DiscoveredWriterData, SequenceNumber, GUID};
use std::collections::HashMap;

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
    pub intervals: Lapper<usize, ()>,
}

impl FragmentedMessage {
    pub fn new(data_size: usize) -> Self {
        Self {
            data_size,
            intervals: Lapper::new(vec![]),
        }
    }
}
