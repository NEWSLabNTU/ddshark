use super::{value::Value, xtable::XTableState};
use crate::{
    state::{ParticipantState, State},
    ui::xtable::XTable,
    utils::{GuidPrefixExt, LocatorExt},
};
use ratatui::{prelude::*, widgets::StatefulWidget};
use rustdds::structure::locator::Locator;

/// The table that keeps a list of observed participants.
pub struct ParticipantTable {
    rows: Vec<Vec<Value>>,
}

impl ParticipantTable {
    pub fn new(state: &State) -> Self {
        let mut participants: Vec<_> = state.participants.iter().collect();
        participants.sort_unstable_by(|(lprefix, _), (rprefix, _)| lprefix.cmp(rprefix));

        let format_locator_list = |locators: Option<&[Locator]>| -> String {
            match locators {
                None | Some(&[]) => "-".to_string(),
                Some(&[locator]) => {
                    format!("{}", locator.display())
                }
                Some(locators) => {
                    let locators: Vec<_> = locators
                        .iter()
                        .map(|locator| format!("{}", locator.display()))
                        .collect();
                    format!("[{}]", locators.join(", "))
                }
            }
        };

        let rows: Vec<Vec<Value>> = participants
            .into_iter()
            .map(|(guid_prefix, part)| {
                let ParticipantState {
                    ref readers,
                    ref writers,
                    ref unicast_locator_list,
                    ref multicast_locator_list,
                    total_msg_count,
                    total_byte_count,
                    total_acknack_count,
                    ref msg_rate_stat,
                    ref bit_rate_stat,
                    ref acknack_rate_stat,
                } = *part;

                let guid_prefix = format!("{}", guid_prefix.display()).into();
                let unicast_locator_list =
                    format_locator_list(unicast_locator_list.as_deref()).into();
                let multicast_locator_list =
                    format_locator_list(multicast_locator_list.as_deref()).into();

                vec![
                    guid_prefix,
                    unicast_locator_list,
                    multicast_locator_list,
                    readers.len().try_into().unwrap(),
                    writers.len().try_into().unwrap(),
                    total_msg_count.try_into().unwrap(),
                    total_byte_count.try_into().unwrap(),
                    total_acknack_count.try_into().unwrap(),
                    msg_rate_stat.stat().mean.into(),
                    bit_rate_stat.stat().mean.into(),
                    acknack_rate_stat.stat().mean.into(),
                ]
            })
            .collect();

        Self { rows }
    }
}

impl StatefulWidget for ParticipantTable {
    type State = ParticipantTableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        const TITLE_GUID_PREFIX: &str = "GUID_prefix";
        const TITLE_UNICAST_ADDRS: &str = "unicast_addrs";
        const TITLE_MULTICAST_ADDRS: &str = "multicast_addrs";
        const TITLE_READER_COUNT: &str = "readers";
        const TITLE_WRITER_COUNT: &str = "writers";
        const TITLE_MESSAGE_COUNT: &str = "msgs";
        const TITLE_BYTE_COUNT: &str = "bytes";
        const TITLE_ACKNACK_COUNT: &str = "acknacks";
        const TITLE_MSGRATE: &str = "msg rate";
        const TITLE_BITRATE: &str = "bit rate";
        const TITLE_ACKNACK_RATE: &str = "acknack rate";

        let header = vec![
            TITLE_GUID_PREFIX,
            TITLE_UNICAST_ADDRS,
            TITLE_MULTICAST_ADDRS,
            TITLE_READER_COUNT,
            TITLE_WRITER_COUNT,
            TITLE_MESSAGE_COUNT,
            TITLE_BYTE_COUNT,
            TITLE_ACKNACK_COUNT,
            TITLE_MSGRATE,
            TITLE_BITRATE,
            TITLE_ACKNACK_RATE,
        ];

        let table = XTable::new("Participants", &header, &self.rows);
        table.render(area, buf, &mut state.table_state);
    }
}

pub struct ParticipantTableState {
    table_state: XTableState,
}

impl ParticipantTableState {
    pub fn new() -> Self {
        let table_state = XTableState::new();

        Self { table_state }
    }

    pub fn previous_item(&mut self) {
        self.table_state.previous_item();
    }

    pub fn next_item(&mut self) {
        self.table_state.next_item();
    }

    pub fn previous_page(&mut self) {
        self.table_state.previous_page();
    }

    pub fn next_page(&mut self) {
        self.table_state.next_page();
    }

    pub fn first_item(&mut self) {
        self.table_state.first_item();
    }

    pub fn last_item(&mut self) {
        self.table_state.last_item();
    }

    pub fn previous_column(&mut self) {
        self.table_state.previous_column();
    }

    pub fn next_column(&mut self) {
        self.table_state.next_column();
    }

    pub fn first_column(&mut self) {
        self.table_state.first_column();
    }

    pub fn last_column(&mut self) {
        self.table_state.last_column();
    }

    pub fn toggle_show(&mut self) {
        self.table_state.toggle_show();
    }

    pub fn toggle_sort(&mut self) {
        self.table_state.toggle_sort();
    }
}
