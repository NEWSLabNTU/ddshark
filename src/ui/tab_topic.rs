use crate::state::State;
use ratatui::{
    backend::Backend,
    layout::Constraint,
    prelude::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table, TableState},
    Frame,
};

pub(crate) struct TabTopic {
    table_state: TableState,
    num_entries: usize,
}

impl TabTopic {
    pub(crate) fn new() -> Self {
        Self {
            table_state: TableState::default(),
            num_entries: 0,
        }
    }

    pub(crate) fn render<B>(&mut self, state: &State, frame: &mut Frame<B>, rect: Rect)
    where
        B: Backend,
    {
        const TITLE_NAME: &str = "name";
        const TITLE_NUM_READERS: &str = "# readers";
        const TITLE_NUM_WRITERS: &str = "# writers";

        let mut topics: Vec<_> = state.topics.iter().collect();
        topics.sort_unstable_by(|(lname, _), (rname, _)| lname.cmp(rname));

        let header = vec![TITLE_NAME, TITLE_NUM_READERS, TITLE_NUM_WRITERS];
        let rows: Vec<_> = topics
            .into_iter()
            .map(|(topic_name, topic)| {
                let topic_name = topic_name.clone();
                let n_readers = topic.readers.len().to_string();
                let n_writers = topic.writers.len().to_string();
                vec![topic_name, n_readers, n_writers]
            })
            .collect();

        let widths: Vec<_> = header
            .iter()
            .enumerate()
            .map(|(idx, title)| {
                let max_len = rows
                    .iter()
                    .map(|row| row[idx].len())
                    .max()
                    .unwrap_or(0)
                    .max(title.len());
                Constraint::Max(max_len as u16)
            })
            .collect();

        let header = Row::new(header);
        let rows: Vec<_> = rows.into_iter().map(Row::new).collect();

        // Save the # of entires
        self.num_entries = rows.len();

        let table_block = Block::default().title("Topics").borders(Borders::ALL);
        let table = Table::new(rows)
            .style(Style::default().fg(Color::White))
            .header(header)
            .block(table_block)
            .widths(&widths)
            .column_spacing(1)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol(">");

        frame.render_stateful_widget(table, rect, &mut self.table_state);
    }

    pub(crate) fn previous_item(&mut self) {
        if self.num_entries > 0 {
            let new_idx = match self.table_state.selected() {
                Some(idx) => idx.saturating_sub(1),
                None => 0,
            };
            self.table_state.select(Some(new_idx));
        }
    }

    pub(crate) fn next_item(&mut self) {
        if let Some(last_idx) = self.num_entries.checked_sub(1) {
            let new_idx = match self.table_state.selected() {
                Some(idx) => idx.saturating_add(1).min(last_idx),
                None => 0,
            };
            self.table_state.select(Some(new_idx));
        }
    }

    pub(crate) fn previous_page(&mut self) {
        if self.num_entries > 0 {
            let new_idx = match self.table_state.selected() {
                Some(idx) => idx.saturating_sub(30),
                None => 0,
            };
            self.table_state.select(Some(new_idx));
        }
    }

    pub(crate) fn next_page(&mut self) {
        if let Some(last_idx) = self.num_entries.checked_sub(1) {
            let new_idx = match self.table_state.selected() {
                Some(idx) => idx.saturating_add(30).min(last_idx),
                None => 0,
            };
            self.table_state.select(Some(new_idx));
        }
    }

    pub(crate) fn first_item(&mut self) {
        if self.num_entries > 0 {
            self.table_state.select(Some(0));
        }
    }

    pub(crate) fn last_item(&mut self) {
        if let Some(idx) = self.num_entries.checked_sub(1) {
            self.table_state.select(Some(idx));
        }
    }
}
