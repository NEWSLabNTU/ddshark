use crate::{
    state::{HeartbeatState, ReaderState, State, WriterState},
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

pub(crate) struct TabReader {
    table_state: TableState,
    num_entries: usize,
}

impl TabReader {
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
        const TITLE_GUID: &str = "GUID";
        const TITLE_SERIAL_NUMBER: &str = "sn";
        const TITLE_TOPIC: &str = "topic";

        let readers = state.participants.iter().flat_map(|(&guid_prefix, part)| {
            part.readers.iter().map(move |(&entity_id, reader)| {
                let guid = GUID::new(guid_prefix, entity_id);
                (guid, reader)
            })
        });

        let header = vec![TITLE_GUID, TITLE_SERIAL_NUMBER, TITLE_TOPIC];
        let rows: Vec<_> = readers
            .clone()
            .map(|(guid, entity)| {
                let topic_name = entity.topic_name().unwrap_or("");
                let ReaderState { last_sn, .. } = *entity;

                let guid = format!("{}", guid.display());
                let topic_name = topic_name.to_string();
                let last_sn = last_sn
                    .map(|sn| format!("{}", sn.0))
                    .unwrap_or_else(|| "-".to_string());

                vec![guid, last_sn, topic_name]
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

        let table_block = Block::default().title("Readers").borders(Borders::ALL);

        // Save the # of entires
        self.num_entries = rows.len();

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
