use super::xtable::XTableState;
use crate::{state::State, ui::xtable::XTable};
use ratatui::{prelude::*, widgets::StatefulWidget};

pub struct TopicTable {
    rows: Vec<Vec<String>>,
}

impl TopicTable {
    pub fn new(state: &State) -> Self {
        let mut topics: Vec<_> = state.topics.iter().collect();
        topics.sort_unstable_by(|(lname, _), (rname, _)| lname.cmp(rname));

        let rows: Vec<_> = topics
            .into_iter()
            .map(|(topic_name, topic)| {
                let topic_name = topic_name.clone();
                let n_readers = topic.readers.len().to_string();
                let n_writers = topic.writers.len().to_string();
                vec![topic_name, n_readers, n_writers]
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

        let header = vec![TITLE_NAME, TITLE_NUM_READERS, TITLE_NUM_WRITERS];

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
}
