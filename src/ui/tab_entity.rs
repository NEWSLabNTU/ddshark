use crate::{
    state::{EntityState, State},
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

pub(crate) struct TabEntity {
    table_state: TableState,
    num_entries: usize,
}

impl TabEntity {
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
        const NONE_TEXT: &str = "<none>";
        const TITLE_GUID: &str = "GUID";
        const TITLE_TOPIC: &str = "topic";
        const TITLE_SERIAL_NUMBER: &str = "sn";
        const TITLE_MESSAGE_COUNT: &str = "msgs";
        const TITLE_BYTE_COUNT: &str = "bytes";
        const TITLE_NUM_FRAGMENTED_MESSAGES: &str = "fragments";
        const TITLE_LAST_HEARTBEAT: &str = "last heartbeat";

        struct TableEntry {
            guid: String,
            topic_name: String,
            last_sn: String,
            byte_count: String,
            message_count: String,
            frag_msg_count: String,
            last_heartbeat: String,
        }

        let entities = state
            .participants
            .iter()
            .flat_map(|(&guid_prefix, p_entry)| {
                p_entry.entities.iter().map(move |(&entity_id, e_entry)| {
                    let guid = GUID::new(guid_prefix, entity_id);
                    (guid, e_entry)
                })
            });

        let rows: Vec<_> = entities
            .clone()
            .map(|(guid, entity)| {
                let topic_name = entity.topic_name().unwrap_or(NONE_TEXT);
                let EntityState {
                    // ref context,
                    last_sn,
                    ref frag_messages,
                    message_count,
                    recv_count,
                    since,
                    ref heartbeat,
                    ..
                } = *entity;

                let byte_count = format!("{recv_count}");
                let last_sn = last_sn
                    .map(|sn| format!("{}", sn.0))
                    .unwrap_or_else(|| NONE_TEXT.to_string());

                let last_heartbeat = match heartbeat {
                    Some(heartbeat) => {
                        format!("{:?}", heartbeat.since.elapsed())
                    }
                    None => NONE_TEXT.to_string(),
                };

                TableEntry {
                    guid: format!("{}", guid.display()),
                    topic_name: topic_name.to_string(),
                    last_sn,
                    message_count: format!("{message_count}"),
                    frag_msg_count: format!("{}", frag_messages.len()),
                    last_heartbeat,
                    byte_count,
                }
            })
            .collect();

        let topic_col_len = rows
            .iter()
            .map(|row| row.topic_name.len())
            .max()
            .unwrap_or(0)
            .max(TITLE_TOPIC.len());
        let sn_col_len = rows
            .iter()
            .map(|row| row.last_sn.len())
            .max()
            .unwrap_or(0)
            .max(TITLE_SERIAL_NUMBER.len());
        let msg_count_col_len = rows
            .iter()
            .map(|row| row.message_count.len())
            .max()
            .unwrap_or(0)
            .max(TITLE_MESSAGE_COUNT.len());
        let byte_count_col_len = rows
            .iter()
            .map(|row| row.byte_count.len())
            .max()
            .unwrap_or(0)
            .max(TITLE_BYTE_COUNT.len());
        let num_frag_msgs_col_len = rows
            .iter()
            .map(|row| row.frag_msg_count.len())
            .max()
            .unwrap_or(0)
            .max(TITLE_NUM_FRAGMENTED_MESSAGES.len());
        let last_heartbeat_col_len = rows
            .iter()
            .map(|row| row.last_heartbeat.len())
            .max()
            .unwrap_or(0)
            .max(TITLE_LAST_HEARTBEAT.len());

        let header = Row::new(vec![
            TITLE_GUID,
            TITLE_TOPIC,
            TITLE_SERIAL_NUMBER,
            TITLE_MESSAGE_COUNT,
            TITLE_BYTE_COUNT,
            TITLE_NUM_FRAGMENTED_MESSAGES,
            TITLE_LAST_HEARTBEAT,
        ]);
        let widths = &[
            Constraint::Min(35),
            Constraint::Max(topic_col_len as u16),
            Constraint::Min(sn_col_len as u16),
            Constraint::Min(msg_count_col_len as u16),
            Constraint::Min(byte_count_col_len as u16),
            Constraint::Min(num_frag_msgs_col_len as u16),
            Constraint::Min(last_heartbeat_col_len as u16),
        ];

        let table_block = Block::default().title("Entities").borders(Borders::ALL);

        let rows = rows.into_iter().map(|row| {
            let TableEntry {
                guid,
                topic_name,
                last_sn,
                message_count,
                byte_count,
                frag_msg_count,
                last_heartbeat,
            } = row;
            Row::new(vec![
                guid,
                topic_name,
                last_sn,
                message_count,
                byte_count,
                frag_msg_count,
                last_heartbeat,
            ])
        });

        // Save the # of entires
        self.num_entries = rows.len();

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
