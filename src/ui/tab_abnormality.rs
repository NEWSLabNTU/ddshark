use crate::{
    state::{Abnormality, State},
    utils::GUIDExt,
};
use ratatui::{
    backend::Backend,
    layout::Constraint,
    prelude::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table, TableState},
    Frame,
};
use rustdds::GUID;

pub(crate) struct TabAbnormality {
    table_state: TableState,
}
impl TabAbnormality {
    pub(crate) fn new() -> Self {
        Self {
            table_state: TableState::default(),
        }
    }

    pub(crate) fn render<B>(&mut self, state: &State, frame: &mut Frame<B>, rect: Rect)
    where
        B: Backend,
    {
        const TITLE_WHEN: &str = "when";
        const TITLE_WRITER_ID: &str = "writer";
        const TITLE_READER_ID: &str = "reader";
        const TITLE_TOPIC_NAME: &str = "topic";
        const TITLE_DESC: &str = "desc";

        struct TableEntry {
            when: String,
            writer_id: String,
            reader_id: String,
            topic_name: String,
            desc: String,
        }

        let mut rows: Vec<_> = state
            .abnormalities
            .iter()
            .map(|report| {
                let Abnormality {
                    when,
                    writer_id,
                    reader_id,
                    ref topic_name,
                    ref desc,
                } = *report;
                let guid_to_string = |guid: Option<GUID>| match guid {
                    Some(guid) => format!("{}", guid.display()),
                    None => "<none>".to_string(),
                };

                let when = when.to_rfc3339();
                let reader_id = guid_to_string(reader_id);
                let writer_id = guid_to_string(writer_id);
                let topic_name = topic_name
                    .to_owned()
                    .unwrap_or_else(|| "<none>".to_string());
                let desc = desc.clone();

                TableEntry {
                    when,
                    writer_id,
                    reader_id,
                    topic_name,
                    desc,
                }
            })
            .collect();

        rows.sort_unstable_by(|lhs, rhs| lhs.when.cmp(&rhs.when).reverse());

        let when_col_len = rows
            .iter()
            .map(|row| row.when.len())
            .max()
            .unwrap_or(0)
            .max(TITLE_WHEN.len());
        let reader_id_col_len = rows
            .iter()
            .map(|row| row.reader_id.len())
            .max()
            .unwrap_or(0)
            .max(TITLE_READER_ID.len());
        let writer_id_col_len = rows
            .iter()
            .map(|row| row.writer_id.len())
            .max()
            .unwrap_or(0)
            .max(TITLE_WRITER_ID.len());
        let topic_name_col_len = rows
            .iter()
            .map(|row| row.topic_name.len())
            .max()
            .unwrap_or(0)
            .max(TITLE_TOPIC_NAME.len());
        let desc_col_len = rows
            .iter()
            .map(|row| row.desc.len())
            .max()
            .unwrap_or(0)
            .max(TITLE_DESC.len());

        let header = Row::new(vec![
            TITLE_WHEN,
            TITLE_WRITER_ID,
            TITLE_READER_ID,
            TITLE_TOPIC_NAME,
            TITLE_DESC,
        ]);
        let widths = &[
            Constraint::Min(when_col_len as u16),
            Constraint::Min(writer_id_col_len as u16),
            Constraint::Min(reader_id_col_len as u16),
            Constraint::Min(topic_name_col_len as u16),
            Constraint::Min(desc_col_len as u16),
        ];

        let rows: Vec<_> = rows
            .into_iter()
            .map(|row| {
                let TableEntry {
                    when,
                    writer_id,
                    reader_id,
                    topic_name,
                    desc,
                } = row;
                Row::new(vec![when, writer_id, reader_id, topic_name, desc])
            })
            .collect();

        let table_block = Block::default()
            .title("Abnormalities")
            .borders(Borders::ALL);
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
