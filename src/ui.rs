use crate::{
    state::{EntityState, State},
    utils::GUIDExt,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Color, Modifier, Style},
    symbols::DOT,
    widgets::{Block, Borders, Row, Table, TableState, Tabs},
    Frame, Terminal,
};
use rustdds::GUID;
use std::{
    io,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::error;

const TAB_TITLES: &[&str] = &["Writers", "Topics"];

pub(crate) struct Tui {
    writer_table_state: TableState,
    topic_table_state: TableState,
    tick_dur: Duration,
    tab_index: usize,
    state: Arc<Mutex<State>>,
}

impl Tui {
    pub fn new(tick_dur: Duration, state: Arc<Mutex<State>>) -> Self {
        Self {
            writer_table_state: TableState::default(),
            topic_table_state: TableState::default(),
            tick_dur,
            state,
            tab_index: 0,
        }
    }

    pub fn run(mut self) -> io::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        self.run_loop(&mut terminal)?;

        // restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        Ok(())
    }

    fn run_loop<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        // let mut table_state = TableState::default();
        let mut last_tick = Instant::now();

        loop {
            // Wait for key event
            {
                let timeout = self
                    .tick_dur
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_else(|| Duration::from_secs(0));

                if event::poll(timeout)? {
                    if let Event::Key(key) = event::read()? {
                        use KeyCode as C;

                        let n_tabs = TAB_TITLES.len();

                        match key.code {
                            C::Char('q') => break,
                            C::Up => {
                                self.previous_item();
                            }
                            C::Down => {
                                self.next_item();
                            }
                            C::Left => {
                                // *self.table_state.offset_mut() =
                                //     self.table_state.offset().saturating_sub(1);
                            }
                            C::Right => {
                                // *self.table_state.offset_mut() =
                                //     self.table_state.offset().saturating_add(1);
                            }
                            C::PageUp => {
                                self.previous_page();
                            }
                            C::PageDown => {
                                self.next_page();
                            }
                            C::Tab => {
                                self.tab_index = (self.tab_index + 1) % n_tabs;
                            }
                            C::BackTab => {
                                self.tab_index = (self.tab_index + (n_tabs - 1)) % n_tabs;
                            }
                            _ => {}
                        }
                    }
                }
            }

            let elapsed_time = last_tick.elapsed();
            if elapsed_time >= self.tick_dur {
                // Draw UI
                terminal.draw(|frame| self.draw_ui(frame, elapsed_time))?;

                // Clean up state
                last_tick = Instant::now();
            }
        }

        Ok(())
    }

    fn draw_ui<B: Backend>(&mut self, frame: &mut Frame<B>, elapsed_time: Duration) {
        let Ok(state) = self.state.lock() else {
            // TODO: show error
            error!("Mutex is poisoned");
            return;
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
            .split(frame.size());

        let tabs_block = Block::default().title("Tabs").borders(Borders::ALL);
        let tabs = Tabs::new(TAB_TITLES.to_vec())
            .block(tabs_block)
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow))
            .divider(DOT);
        frame.render_widget(tabs, chunks[0]);

        match self.tab_index {
            0 => {
                make_entity_table(&state, frame, chunks[1], &mut self.writer_table_state);
            }
            1 => {
                make_topic_table(&state, frame, chunks[1], &mut self.topic_table_state);
            }
            _ => unreachable!(),
        }
    }

    fn previous_item(&mut self) {
        match self.tab_index {
            0 => {
                let new_idx = match self.writer_table_state.selected() {
                    Some(idx) => idx.saturating_sub(1),
                    None => 0,
                };
                self.writer_table_state.select(Some(new_idx));
            }
            1 => {
                let new_idx = match self.topic_table_state.selected() {
                    Some(idx) => idx.saturating_sub(1),
                    None => 0,
                };
                self.topic_table_state.select(Some(new_idx));
            }
            _ => unreachable!(),
        }
    }

    fn next_item(&mut self) {
        match self.tab_index {
            0 => {
                let new_idx = match self.writer_table_state.selected() {
                    Some(idx) => idx.saturating_add(1),
                    None => 0,
                };
                self.writer_table_state.select(Some(new_idx));
            }
            1 => {
                let new_idx = match self.topic_table_state.selected() {
                    Some(idx) => idx.saturating_add(1),
                    None => 0,
                };
                self.topic_table_state.select(Some(new_idx));
            }
            _ => unreachable!(),
        }
    }

    fn previous_page(&mut self) {
        match self.tab_index {
            0 => {
                let new_idx = match self.writer_table_state.selected() {
                    Some(idx) => idx.saturating_sub(30),
                    None => 0,
                };
                self.writer_table_state.select(Some(new_idx));
            }
            1 => {
                let new_idx = match self.topic_table_state.selected() {
                    Some(idx) => idx.saturating_sub(30),
                    None => 0,
                };
                self.topic_table_state.select(Some(new_idx));
            }
            _ => unreachable!(),
        }
    }

    fn next_page(&mut self) {
        match self.tab_index {
            0 => {
                let new_idx = match self.writer_table_state.selected() {
                    Some(idx) => idx.saturating_add(30),
                    None => 0,
                };
                self.writer_table_state.select(Some(new_idx));
            }
            1 => {
                let new_idx = match self.topic_table_state.selected() {
                    Some(idx) => idx.saturating_add(30),
                    None => 0,
                };
                self.topic_table_state.select(Some(new_idx));
            }
            _ => unreachable!(),
        }
    }
}

fn make_entity_table<B: Backend>(
    state: &State,
    frame: &mut Frame<B>,
    rect: Rect,
    writer_table_state: &mut TableState,
) {
    const NONE_TEXT: &str = "<none>";
    const TITLE_GUID: &str = "GUID";
    const TITLE_TOPIC: &str = "topic";
    const TITLE_SERIAL_NUMBER: &str = "sn";
    const TITLE_MESSAGE_COUNT: &str = "msg count";
    const TITLE_NUM_FRAGMENTED_MESSAGES: &str = "# frag msgs";
    const TITLE_LAST_HEARTBEAT: &str = "last heartbeat";

    struct TableEntry {
        guid: String,
        topic_name: String,
        last_sn: String,
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
                ref context,
                last_sn,
                ref frag_messages,
                message_count,
                recv_count,
                since,
                ref heartbeat,
                ..
            } = *entity;

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
        TITLE_NUM_FRAGMENTED_MESSAGES,
        TITLE_LAST_HEARTBEAT,
    ]);
    let widths = &[
        Constraint::Min(35),
        Constraint::Max(topic_col_len as u16),
        Constraint::Min(sn_col_len as u16),
        Constraint::Min(msg_count_col_len as u16),
        Constraint::Min(num_frag_msgs_col_len as u16),
        Constraint::Min(last_heartbeat_col_len as u16),
    ];

    let table_block = Block::default().title("Writers").borders(Borders::ALL);

    let rows = rows.into_iter().map(|row| {
        let TableEntry {
            guid,
            topic_name,
            last_sn,
            message_count,
            frag_msg_count,
            last_heartbeat,
        } = row;
        Row::new(vec![
            guid,
            topic_name,
            last_sn,
            message_count,
            frag_msg_count,
            last_heartbeat,
        ])
    });

    let table = Table::new(rows)
        .style(Style::default().fg(Color::White))
        .header(header)
        .block(table_block)
        .widths(widths)
        .column_spacing(1)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">");

    frame.render_stateful_widget(table, rect, writer_table_state);
}

fn make_topic_table<B: Backend>(
    state: &State,
    frame: &mut Frame<B>,
    rect: Rect,
    writer_table_state: &mut TableState,
) {
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

    frame.render_stateful_widget(table, rect, writer_table_state);
}
