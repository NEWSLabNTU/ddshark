use ratatui::{
    layout::Constraint,
    prelude::{Rect, *},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, StatefulWidget, Table, TableState},
};

pub struct XTable<'a> {
    title: &'a str,
    header: &'a [&'a str],
    rows: &'a [Vec<String>],
}

impl<'a> XTable<'a> {
    pub fn new(title: &'a str, header: &'a [&str], rows: &'a [Vec<String>]) -> Self {
        Self {
            header,
            rows,
            title,
        }
    }
}

impl<'a> StatefulWidget for XTable<'a> {
    type State = XTableState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let widths: Vec<_> = self
            .header
            .iter()
            .enumerate()
            .map(|(idx, title)| {
                let max_len = self
                    .rows
                    .iter()
                    .map(|row| row[idx].len())
                    .max()
                    .unwrap_or(0)
                    .max(title.len());
                Constraint::Max(max_len as u16)
            })
            .collect();

        let header = Row::new(self.header.iter().map(|title| {
            let cell: Cell = title.to_string().into();
            cell.style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED),
            )
        }));
        let rows: Vec<_> = self
            .rows
            .iter()
            .map(|row| Row::new(row.iter().cloned()))
            .collect();

        let table_block = Block::default().title(self.title).borders(Borders::ALL);

        // Save the # of entires
        state.num_entries = rows.len();
        state.page_height = (area.height as usize).saturating_sub(3).max(1);

        let table = Table::new(rows)
            .style(Style::default().fg(Color::White))
            .header(header)
            .block(table_block)
            .widths(&widths)
            .column_spacing(2)
            .highlight_style(Style::default().fg(Color::Black).bg(Color::White));

        table.render(area, buf, &mut state.table_state);
    }
}

pub struct XTableState {
    table_state: TableState,
    num_entries: usize,
    page_height: usize,
}

impl XTableState {
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));

        Self {
            table_state,
            num_entries: 0,
            page_height: 1,
        }
    }

    pub fn previous_item(&mut self) {
        if self.num_entries > 0 {
            let new_idx = match self.table_state.selected() {
                Some(idx) => idx.saturating_sub(1),
                None => 0,
            };
            self.table_state.select(Some(new_idx));
        }
    }

    pub fn next_item(&mut self) {
        if let Some(last_idx) = self.num_entries.checked_sub(1) {
            let new_idx = match self.table_state.selected() {
                Some(idx) => idx.saturating_add(1).min(last_idx),
                None => 0,
            };
            self.table_state.select(Some(new_idx));
        }
    }

    pub fn previous_page(&mut self) {
        if self.num_entries > 0 {
            let orig_idx = self.table_state.selected().unwrap_or(0);
            let new_idx = orig_idx.saturating_sub(self.page_height);
            self.table_state.select(Some(new_idx));
            *self.table_state.offset_mut() -= orig_idx - new_idx;
        }
    }

    pub fn next_page(&mut self) {
        if let Some(last_idx) = self.num_entries.checked_sub(1) {
            let orig_idx = self.table_state.selected().unwrap_or(0);
            let new_idx = orig_idx.saturating_add(self.page_height).min(last_idx);
            self.table_state.select(Some(new_idx));
            *self.table_state.offset_mut() += new_idx - orig_idx;
        }
    }

    pub fn first_item(&mut self) {
        if self.num_entries > 0 {
            self.table_state.select(Some(0));
        }
    }

    pub fn last_item(&mut self) {
        if let Some(idx) = self.num_entries.checked_sub(1) {
            self.table_state.select(Some(idx));
        }
    }
}
