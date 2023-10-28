use super::value::Value;
use itertools::izip;
use ratatui::{
    layout::Constraint,
    prelude::{Rect, *},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, StatefulWidget, Table, TableState},
};

pub struct XTable<'a> {
    title: &'a str,
    header: &'a [&'a str],
    rows: &'a [Vec<Value>],
}

impl<'a> XTable<'a> {
    pub fn new(title: &'a str, header: &'a [&str], rows: &'a [Vec<Value>]) -> Self {
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
        let mut rows: Vec<_> = self.rows.iter().collect();
        if let Some(sort) = &state.sort {
            rows.sort_unstable_by(|lrow, rrow| {
                let lhs = &lrow[sort.column_index];
                let rhs = &rrow[sort.column_index];
                let ord = lhs.partial_cmp(rhs).unwrap();

                if sort.ascending {
                    ord
                } else {
                    ord.reverse()
                }
            });
        }

        let header: Vec<String> = izip!(0.., &state.show, self.header)
            .map(|(index, &show, title)| {
                if show {
                    let sort_symbol = match &state.sort {
                        Some(sort) if sort.column_index == index => {
                            if sort.ascending {
                                "↑"
                            } else {
                                "↓"
                            }
                        }
                        _ => "",
                    };

                    format!("{title}{sort_symbol}")
                } else {
                    let ch = title.chars().next().unwrap_or(' ');
                    format!("{ch}")
                }
            })
            .collect();

        let rows: Vec<Vec<String>> = rows
            .iter()
            .cloned()
            .map(|row| {
                let row: Vec<String> = izip!(&state.show, row)
                    .map(|(&show, value)| {
                        if show {
                            value.to_string()
                        } else {
                            "".to_string()
                        }
                    })
                    .collect();
                row
            })
            .collect();

        let widths: Vec<_> = izip!(0.., &state.show, &header)
            .map(|(idx, &show, title)| {
                if show {
                    let max_len = rows
                        .iter()
                        .map(|row| row[idx].len())
                        .max()
                        .unwrap_or(0)
                        .max(title.len());
                    Constraint::Max(max_len as u16)
                } else {
                    Constraint::Max(1)
                }
            })
            .collect();

        let rows: Vec<_> = rows
            .into_iter()
            .map(|row| {
                let row: Vec<_> = row
                    .into_iter()
                    .enumerate()
                    .map(|(index, text)| {
                        let cell: Cell = text.into();
                        let mut style = Style::default();

                        if Some(index) == state.column_index {
                            style = style.add_modifier(Modifier::BOLD);
                        }

                        cell.style(style)
                    })
                    .collect();

                Row::new(row)
            })
            .collect();

        let header = Row::new(izip!(0.., header).map(|(index, title)| {
            let cell: Cell = title.into();
            let mut style = Style::default()
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED);

            if Some(index) == state.column_index {
                style = style.fg(Color::Black).bg(Color::Gray);
            }

            cell.style(style)
        }));

        let table_block = Block::default().title(self.title).borders(Borders::ALL);

        // Save the # of entires
        state.num_entries = rows.len();
        state.page_height = (area.height as usize).saturating_sub(3).max(1);
        state.num_columns = self.header.len();

        if let Some(column_index) = state.column_index {
            if column_index >= self.header.len() {
                state.column_index = None;
            }
        }
        state.show.resize(self.header.len(), true);

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
    num_columns: usize,
    page_height: usize,
    column_index: Option<usize>,
    show: Vec<bool>,
    sort: Option<Sort>,
}

impl XTableState {
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));

        Self {
            table_state,
            num_entries: 0,
            page_height: 1,
            num_columns: 0,
            column_index: None,
            show: vec![],
            sort: None,
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

    pub fn next_column(&mut self) {
        if let Some(column_index) = &mut self.column_index {
            *column_index = if let Some(max_index) = self.num_columns.checked_sub(1) {
                (*column_index + 1).clamp(0, max_index)
            } else {
                0
            };
        } else if self.num_columns > 0 {
            self.column_index = Some(0);
        }
    }

    pub fn previous_column(&mut self) {
        if let Some(column_index) = &mut self.column_index {
            *column_index = column_index.saturating_sub(1);
        } else if self.num_columns > 0 {
            self.column_index = Some(0);
        }
    }

    pub fn first_column(&mut self) {
        if self.num_columns > 0 {
            self.column_index = Some(0);
        }
    }

    pub fn last_column(&mut self) {
        if let Some(max_index) = self.num_columns.checked_sub(1) {
            self.column_index = Some(max_index);
        }
    }

    pub fn toggle_show(&mut self) {
        if let Some(column_index) = self.column_index {
            self.show[column_index] = !self.show[column_index];
        }
    }

    pub fn toggle_sort(&mut self) {
        if let Some(column_index) = self.column_index {
            if let Some(sort) = &mut self.sort {
                if sort.column_index == column_index {
                    sort.ascending = !sort.ascending;
                } else {
                    *sort = Sort {
                        column_index,
                        ascending: true,
                    };
                }
            } else {
                self.sort = Some(Sort {
                    column_index,
                    ascending: true,
                });
            }
        }
    }
}

#[derive(Debug, Clone)]
struct Sort {
    pub column_index: usize,
    pub ascending: bool,
}
