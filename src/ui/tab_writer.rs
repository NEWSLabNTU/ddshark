use super::{value::Value, xtable::XTableState};
use crate::{
    state::{HeartbeatState, State, WriterState},
    ui::xtable::XTable,
    utils::GUIDExt,
};
use ratatui::{prelude::*, widgets::StatefulWidget};
use rustdds::GUID;

/// The table that keeps a list of observed writer entities.
pub struct WriterTable {
    rows: Vec<Vec<Value>>,
}

impl WriterTable {
    pub fn new(state: &State) -> Self {
        let mut writers: Vec<_> = state
            .participants
            .iter()
            .flat_map(|(&guid_prefix, part)| {
                part.writers.iter().map(move |(&entity_id, writer)| {
                    let guid = GUID::new(guid_prefix, entity_id);
                    (guid, writer)
                })
            })
            .collect();
        writers.sort_unstable_by(|(lid, _), (rid, _)| lid.cmp(rid));

        let rows: Vec<_> = writers
            .into_iter()
            .map(|(guid, writer)| {
                let WriterState {
                    last_sn,
                    ref frag_messages,
                    total_msg_count,
                    total_byte_count,
                    ref bit_rate_stat,
                    ref msg_rate_stat,
                    ref heartbeat,
                    ..
                } = *writer;

                let guid = format!("{}", guid.display()).into();
                let topic_name = writer.topic_name().unwrap_or("").into();
                let type_name = writer.type_name().unwrap_or("-").into();
                let byte_count = total_byte_count.try_into().unwrap();
                let message_count = total_msg_count.try_into().unwrap();
                let avg_msgrate = msg_rate_stat.stat().mean.into();
                let avg_bitrate = bit_rate_stat.stat().mean.into();
                let frag_msg_count = if frag_messages.is_empty() {
                    Value::None
                } else {
                    frag_messages.len().try_into().unwrap()
                };
                let last_sn = last_sn.map(|sn| sn.0.into()).unwrap_or(Value::None);

                let heartbeat_range = match heartbeat {
                    Some(heartbeat) => {
                        let HeartbeatState {
                            first_sn, last_sn, ..
                        } = heartbeat;
                        format!("{first_sn}..{last_sn}").into()
                    }
                    None => Value::None,
                };

                vec![
                    guid,
                    last_sn,
                    message_count,
                    avg_msgrate,
                    byte_count,
                    avg_bitrate,
                    frag_msg_count,
                    heartbeat_range,
                    type_name,
                    topic_name,
                ]
            })
            .collect();

        Self { rows }
    }
}

impl StatefulWidget for WriterTable {
    type State = WriterTableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        const TITLE_GUID: &str = "GUID";
        const TITLE_TOPIC: &str = "topic";
        const TITLE_TYPE: &str = "type";
        const TITLE_SERIAL_NUMBER: &str = "sn";
        const TITLE_MESSAGE_COUNT: &str = "msgs";
        const TITLE_BYTE_COUNT: &str = "bytes";
        const TITLE_MSGRATE: &str = "msgrate";
        const TITLE_BITRATE: &str = "bitrate";
        const TITLE_NUM_FRAGMENTED_MESSAGES: &str = "unfrag_msgs";
        const TITLE_HEARTBEAT: &str = "cached_sn";

        let header = vec![
            TITLE_GUID,
            TITLE_SERIAL_NUMBER,
            TITLE_MESSAGE_COUNT,
            TITLE_MSGRATE,
            TITLE_BYTE_COUNT,
            TITLE_BITRATE,
            TITLE_NUM_FRAGMENTED_MESSAGES,
            TITLE_HEARTBEAT,
            TITLE_TYPE,
            TITLE_TOPIC,
        ];

        let table = XTable::new("Writers", &header, &self.rows);
        table.render(area, buf, &mut state.table_state);
    }
}

pub struct WriterTableState {
    table_state: XTableState,
}

impl WriterTableState {
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
