use super::xtable::XTableState;
use crate::{
    state::{HeartbeatState, State, WriterState},
    ui::xtable::XTable,
    utils::GUIDExt,
};
use ratatui::{prelude::*, widgets::StatefulWidget};
use rustdds::GUID;

pub struct WriterTable {
    rows: Vec<Vec<String>>,
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
            .map(|(guid, entity)| {
                let topic_name = entity.topic_name().unwrap_or("");
                let WriterState {
                    last_sn,
                    ref frag_messages,
                    total_msg_count,
                    total_byte_count,
                    avg_bitrate,
                    avg_msgrate,
                    ref heartbeat,
                    ..
                } = *entity;

                let guid = format!("{}", guid.display());
                let topic_name = topic_name.to_string();
                let byte_count = format!("{total_byte_count}");
                let message_count = format!("{total_msg_count}");
                let avg_bitrate = format!("{avg_bitrate:.2}");
                let avg_msgrate = format!("{avg_msgrate:.2}");
                let frag_msg_count = if frag_messages.is_empty() {
                    "-".to_string()
                } else {
                    format!("{}", frag_messages.len())
                };
                let last_sn = last_sn
                    .map(|sn| format!("{}", sn.0))
                    .unwrap_or_else(|| "-".to_string());

                let heartbeat_range = match heartbeat {
                    Some(heartbeat) => {
                        let HeartbeatState {
                            first_sn, last_sn, ..
                        } = heartbeat;
                        format!("{first_sn}..{last_sn}")
                    }
                    None => "-".to_string(),
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
}
