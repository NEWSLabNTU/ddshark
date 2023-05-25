use rust_lapper::Lapper;
use rustdds::{discovery::data_types::topic_data::DiscoveredReaderData, SequenceNumber, GUID};
use std::collections::{BTreeMap, HashMap};

/// The TUI state.
#[derive(Debug)]
pub(crate) struct State {
    pub entities: HashMap<GUID, EntityState>,
    pub topic_info: BTreeMap<GUID, DiscoveredReaderData>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            entities: HashMap::new(),
            topic_info: BTreeMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct EntityState {
    pub last_sn: Option<SequenceNumber>,
    pub frag_messages: HashMap<SequenceNumber, FragmentedMessage>,
}

impl Default for EntityState {
    fn default() -> Self {
        Self {
            frag_messages: HashMap::new(),
            last_sn: None,
        }
    }
}

#[derive(Debug)]
pub struct FragmentedMessage {
    pub data_size: usize,
    pub remaining_size: usize,
    pub intervals: Lapper<usize, ()>,
}

impl FragmentedMessage {
    pub fn new(data_size: usize) -> Self {
        Self {
            data_size,
            remaining_size: data_size,
            intervals: Lapper::new(vec![]),
        }
    }
}
