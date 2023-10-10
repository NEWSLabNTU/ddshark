use super::xtable::XTableState;
use crate::{
    state::{State, Statistics},
    ui::xtable::XTable,
};
use ratatui::{prelude::*, widgets::StatefulWidget};

pub struct StatTable {
    rows: Vec<Vec<String>>,
}

impl StatTable {
    pub fn new(state: &State) -> Self {
        let Statistics {
            packet_count,
            data_submsg_count,
            datafrag_submsg_count,
            acknack_submsg_count,
            ackfrag_submsg_count,
            heartbeat_submsg_count,
            heartbeat_frag_submsg_count,
        } = state.stat;

        let rows = vec![
            vec!["packets".to_string(), format!("{packet_count}")],
            vec!["data submsg".to_string(), format!("{data_submsg_count}")],
            vec![
                "datafrag submsg".to_string(),
                format!("{datafrag_submsg_count}"),
            ],
            vec![
                "acknack submsg".to_string(),
                format!("{acknack_submsg_count}"),
            ],
            vec![
                "ackfrag submsg".to_string(),
                format!("{ackfrag_submsg_count}"),
            ],
            vec![
                "heartbeat submsg".to_string(),
                format!("{heartbeat_submsg_count}"),
            ],
            vec![
                "heartbeat_frag submsg".to_string(),
                format!("{heartbeat_frag_submsg_count}"),
            ],
        ];

        Self { rows }
    }
}

impl StatefulWidget for StatTable {
    type State = StatTableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        const TITLE_ITEM: &str = "item";
        const TITLE_VALUE: &str = "value";

        let header = vec![TITLE_ITEM, TITLE_VALUE];

        let table = XTable::new("Statistics", &header, &self.rows);
        table.render(area, buf, &mut state.table_state);
    }
}

pub struct StatTableState {
    table_state: XTableState,
}

impl StatTableState {
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
