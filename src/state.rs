use crate::dds::DdsEntity;
use std::collections::HashMap;

/// The TUI state.
pub(crate) struct State {
    pub pub_keys: HashMap<String, Entry>,
    pub sub_keys: HashMap<String, Entry>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            pub_keys: HashMap::new(),
            sub_keys: HashMap::new(),
        }
    }
}

pub(crate) struct Entry {
    pub entity: DdsEntity,
    pub acc_msgs: usize,
    pub acc_bytes: usize,
}

impl Entry {
    pub fn new(entity: DdsEntity) -> Self {
        Self {
            entity,
            acc_msgs: 0,
            acc_bytes: 0,
        }
    }
}
