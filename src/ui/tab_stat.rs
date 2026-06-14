use super::{value::Value, xtable::XTableState};
use crate::{
    metrics::MetricsSnapshot,
    state::{State, Statistics},
    ui::xtable::XTable,
};
use ratatui::{prelude::*, widgets::StatefulWidget};

/// The table that presents general traffic statistics.
pub struct StatTable {
    rows: Vec<Vec<Value>>,
}

impl StatTable {
    pub fn new(state: &State, metrics: &MetricsSnapshot) -> Self {
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
            // `packet_count` is incremented once per submessage handler, so it is the
            // total submessage count, not a packet count (see issue 006).
            vec!["total submsg".into(), format!("{packet_count}").into()],
            vec!["data submsg".into(), format!("{data_submsg_count}").into()],
            vec![
                "datafrag submsg".into(),
                format!("{datafrag_submsg_count}").into(),
            ],
            vec![
                "acknack submsg".into(),
                format!("{acknack_submsg_count}").into(),
            ],
            vec![
                "ackfrag submsg".into(),
                format!("{ackfrag_submsg_count}").into(),
            ],
            vec![
                "heartbeat submsg".into(),
                format!("{heartbeat_submsg_count}").into(),
            ],
            vec![
                "heartbeat_frag submsg".into(),
                format!("{heartbeat_frag_submsg_count}").into(),
            ],
            // Pipeline health: when dropped > 0 the counts above are undercounts.
            vec![
                "dropped events".into(),
                format!("{}", metrics.messages_dropped).into(),
            ],
            vec![
                "queue depth".into(),
                format!("{} (max {})", metrics.queue_depth, metrics.max_queue_depth).into(),
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
