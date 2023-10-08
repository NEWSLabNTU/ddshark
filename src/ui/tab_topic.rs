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
}
impl TabTopic {
    pub(crate) fn new() -> Self {
        Self {
            table_state: TableState::default(),
        }
    }

    pub(crate) fn render<B>(&mut self, state: &State, frame: &mut Frame<B>, rect: Rect)
    where
        B: Backend,
    {
        const TITLE_NAME: &str = "name";
        const TITLE_NUM_READERS: &str = "# of readers";
        const TITLE_NUM_WRITERS: &str = "# of writers";

        struct TableEntry {
            name: String,
            n_readers: String,
            n_writers: String,
        }

        let mut rows: Vec<_> = state
            .topics
            .iter()
            .map(|(topic_name, topic)| {
                let topic_name = topic_name.clone();
                let n_readers = topic.readers.len().to_string();
                let n_writers = topic.writers.len().to_string();

                TableEntry {
                    name: topic_name,
                    n_readers,
                    n_writers,
                }
            })
            .collect();

        rows.sort_unstable_by(|lhs, rhs| lhs.name.cmp(&rhs.name));

        let name_col_len = rows
            .iter()
            .map(|row| row.name.len())
            .max()
            .unwrap_or(0)
            .max(TITLE_NAME.len());
        let n_readers_col_len = rows
            .iter()
            .map(|row| row.n_readers.len())
            .max()
            .unwrap_or(0)
            .max(TITLE_NUM_READERS.len());
        let n_writers_col_len = rows
            .iter()
            .map(|row| row.n_writers.len())
            .max()
            .unwrap_or(0)
            .max(TITLE_NUM_WRITERS.len());

        let header = Row::new(vec![TITLE_NAME, TITLE_NUM_READERS, TITLE_NUM_WRITERS]);
        let widths = &[
            Constraint::Min(name_col_len as u16),
            Constraint::Min(n_readers_col_len as u16),
            Constraint::Min(n_writers_col_len as u16),
        ];

        let rows: Vec<_> = rows
            .into_iter()
            .map(|row| {
                let TableEntry {
                    name,
                    n_readers,
                    n_writers,
                } = row;
                Row::new(vec![name, n_readers, n_writers])
            })
            .collect();

        let table_block = Block::default().title("Topics").borders(Borders::ALL);
        let table = Table::new(rows)
            .style(Style::default().fg(Color::White))
            .header(header)
            .block(table_block)
            .widths(widths)
            .column_spacing(1)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol(">");

        frame.render_stateful_widget(table, rect, &mut self.table_state);
    }

    pub(crate) fn previous_item(&mut self) {
        let new_idx = match self.table_state.selected() {
            Some(idx) => idx.saturating_sub(1),
            None => 0,
        };
        self.table_state.select(Some(new_idx));
    }

    pub(crate) fn next_item(&mut self) {
        let new_idx = match self.table_state.selected() {
            Some(idx) => idx.saturating_add(1),
            None => 0,
        };
        self.table_state.select(Some(new_idx));
    }

    pub(crate) fn previous_page(&mut self) {
        let new_idx = match self.table_state.selected() {
            Some(idx) => idx.saturating_sub(30),
            None => 0,
        };
        self.table_state.select(Some(new_idx));
    }

    pub(crate) fn next_page(&mut self) {
        // TODO: get correct page size

        let new_idx = match self.table_state.selected() {
            Some(idx) => idx.saturating_add(30),
            None => 0,
        };
        self.table_state.select(Some(new_idx));
    }

    pub(crate) fn first_item(&mut self) {
        self.table_state.select(Some(0));
    }

    pub(crate) fn last_item(&mut self) {
        // TODO
    }
}
