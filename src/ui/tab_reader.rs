use crate::{
    state::{ReaderState, State},
    ui::xtable::XTable,
    utils::GUIDExt,
};
use ratatui::{prelude::*, widgets::StatefulWidget};
use rustdds::GUID;

use super::xtable::XTableState;

pub struct ReaderTable {
    rows: Vec<Vec<String>>,
}

impl ReaderTable {
    pub fn new(state: &State) -> Self {
        let readers = state.participants.iter().flat_map(|(&guid_prefix, part)| {
            part.readers.iter().map(move |(&entity_id, reader)| {
                let guid = GUID::new(guid_prefix, entity_id);
                (guid, reader)
            })
        });

        let rows: Vec<_> = readers
            .clone()
            .map(|(guid, entity)| {
                let ReaderState {
                    last_sn,
                    total_acknack_count,
                    avg_acknack_rate,
                    ref acknack,
                    ..
                } = *entity;

                let guid = format!("{}", guid.display());
                let sn = match last_sn {
                    Some(sn) => format!("{sn}"),
                    None => "-".to_string(),
                };
                let type_name = entity.type_name().unwrap_or("").to_string();
                let topic_name = entity.topic_name().unwrap_or("").to_string();
                let missing_sn = match acknack {
                    Some(acknack) => format!("{:?}", acknack.missing_sn),
                    None => "-".to_string(),
                };
                let total_acks = format!("{total_acknack_count}");
                let avg_ack_rate = format!("{avg_acknack_rate:.2}");

                vec![
                    guid,
                    sn,
                    missing_sn,
                    total_acks,
                    avg_ack_rate,
                    type_name,
                    topic_name,
                ]
            })
            .collect();

        Self { rows }
    }
}

impl StatefulWidget for ReaderTable {
    type State = ReaderTableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        const TITLE_GUID: &str = "GUID";
        const TITLE_LAST_SN: &str = "sn";
        const TITLE_MISSING_SN: &str = "missing_sn";
        const TITLE_TOTAL_ACKNACK_COUNT: &str = "acks";
        const TITLE_AVERAGE_ACKNACK_RATE: &str = "ack_rate";
        const TITLE_TYPE: &str = "type";
        const TITLE_TOPIC: &str = "topic";

        let header = vec![
            TITLE_GUID,
            TITLE_LAST_SN,
            TITLE_MISSING_SN,
            TITLE_TOTAL_ACKNACK_COUNT,
            TITLE_AVERAGE_ACKNACK_RATE,
            TITLE_TYPE,
            TITLE_TOPIC,
        ];

        let table = XTable::new("Readers", &header, &self.rows);
        table.render(area, buf, &mut state.table_state);
    }
}

pub struct ReaderTableState {
    table_state: XTableState,
}

impl ReaderTableState {
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
