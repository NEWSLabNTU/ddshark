use super::{value::Value, xtable::XTableState};
use crate::{
    state::{State, TopicState},
    ui::xtable::XTable,
};
use ratatui::{prelude::*, widgets::StatefulWidget};

pub struct TopicTable {
    rows: Vec<Vec<Value>>,
}

impl TopicTable {
    pub fn new(state: &State) -> Self {
        let mut topics: Vec<_> = state.topics.iter().collect();
        topics.sort_unstable_by(|(lname, _), (rname, _)| lname.cmp(rname));

        let rows: Vec<_> = topics
            .into_iter()
            .map(|(topic_name, topic)| {
                let TopicState {
                    total_msg_count,
                    total_byte_count,
                    ref msg_rate_stat,
                    ref bit_rate_stat,
                    total_acknack_count,
                    ref acknack_rate_stat,
                    ref readers,
                    ref writers,
                } = *topic;

                let topic_name = topic_name.clone().into();
                let n_readers = readers.len().try_into().unwrap();
                let n_writers = writers.len().try_into().unwrap();

                let total_msg_count = total_msg_count.try_into().unwrap();
                let total_byte_count = total_byte_count.try_into().unwrap();
                let total_acknack_count = total_acknack_count.try_into().unwrap();

                let avg_msgrate = msg_rate_stat.stat().mean.into();
                let avg_bitrate = bit_rate_stat.stat().mean.into();
                let avg_acknack_rate = acknack_rate_stat.stat().mean.into();

                vec![
                    topic_name,
                    n_readers,
                    n_writers,
                    total_msg_count,
                    avg_msgrate,
                    total_byte_count,
                    avg_bitrate,
                    total_acknack_count,
                    avg_acknack_rate,
                ]
            })
            .collect();

        Self { rows }
    }
}

impl StatefulWidget for TopicTable {
    type State = TopicTableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        const TITLE_NAME: &str = "name";
        const TITLE_NUM_READERS: &str = "# readers";
        const TITLE_NUM_WRITERS: &str = "# writers";
        const TITLE_TOTAL_MSGS: &str = "msgs";
        const TITLE_AVG_MSGRATE: &str = "msgrate";
        const TITLE_TOTAL_BYTES: &str = "bytes";
        const TITLE_AVG_BITRATE: &str = "bitrate";
        const TITLE_TOTAL_ACKNACK: &str = "acks";
        const TITLE_AVG_ACKNACK_RATE: &str = "ack_rate";

        let header = vec![
            TITLE_NAME,
            TITLE_NUM_READERS,
            TITLE_NUM_WRITERS,
            TITLE_TOTAL_MSGS,
            TITLE_AVG_MSGRATE,
            TITLE_TOTAL_BYTES,
            TITLE_AVG_BITRATE,
            TITLE_TOTAL_ACKNACK,
            TITLE_AVG_ACKNACK_RATE,
        ];

        let table = XTable::new("Topics", &header, &self.rows);
        table.render(area, buf, &mut state.table_state);
    }
}

pub struct TopicTableState {
    table_state: XTableState,
}

impl TopicTableState {
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
