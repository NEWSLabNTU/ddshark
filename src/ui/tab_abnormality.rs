use super::{value::Value, xtable::XTableState};
use crate::{
    state::{Abnormality, State},
    ui::xtable::XTable,
    utils::GUIDExt,
};
use ratatui::{prelude::*, widgets::StatefulWidget};
use rustdds::GUID;

/// The table that keeps a list of abnormal events.
pub struct AbnormalityTable {
    rows: Vec<Vec<Value>>,
}

impl AbnormalityTable {
    pub fn new(state: &State) -> Self {
        let mut abnormalities: Vec<_> = state.abnormalities.iter().collect();
        abnormalities.sort_unstable_by(|lhs, rhs| lhs.when.cmp(&rhs.when).reverse());

        let rows: Vec<Vec<Value>> = abnormalities
            .into_iter()
            .map(|report| {
                let Abnormality {
                    when,
                    writer_guid,
                    reader_guid,
                    ref topic_name,
                    ref desc,
                } = *report;
                let guid_to_string = |guid: Option<GUID>| match guid {
                    Some(guid) => format!("{}", guid.display()),
                    None => "-".to_string(),
                };

                let when = when.to_rfc3339().into();
                let reader_id = guid_to_string(reader_guid).into();
                let writer_id = guid_to_string(writer_guid).into();
                let topic_name = topic_name
                    .to_owned()
                    .unwrap_or_else(|| "-".to_string())
                    .into();
                let desc = desc.clone().into();

                vec![when, writer_id, reader_id, topic_name, desc]
            })
            .collect();

        Self { rows }
    }
}

impl StatefulWidget for AbnormalityTable {
    type State = AbnormalityTableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        const TITLE_WHEN: &str = "when";
        const TITLE_WRITER_ID: &str = "writer";
        const TITLE_READER_ID: &str = "reader";
        const TITLE_TOPIC_NAME: &str = "topic";
        const TITLE_DESC: &str = "desc";

        let header = vec![
            TITLE_WHEN,
            TITLE_WRITER_ID,
            TITLE_READER_ID,
            TITLE_TOPIC_NAME,
            TITLE_DESC,
        ];

        let table = XTable::new("Abnormalities", &header, &self.rows);
        table.render(area, buf, &mut state.table_state);
    }
}

pub struct AbnormalityTableState {
    table_state: XTableState,
}

impl AbnormalityTableState {
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
