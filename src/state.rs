use rust_lapper::Lapper;
use rustdds::{SequenceNumber, GUID};
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
    pub frag_messages: HashMap<SequenceNumber, FragmentedMessage>,
}

impl Default for EntityState {
    fn default() -> Self {
        Self {
            frag_messages: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct FragmentedMessage {
    pub fragments: Lapper<usize, FragmentState>,
}

impl Default for FragmentedMessage {
    fn default() -> Self {
        Self {
            fragments: Lapper::new(vec![]),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentState {}
