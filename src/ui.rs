use crate::{
    state::{ParticipantState, State},
    utils::{num_base10_digits_i64, num_base10_digits_usize, GUIDExt, GuidPrefixExt},
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use itertools::Itertools;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    symbols::DOT,
    widgets::{Block, Borders, Row, Table, TableState, Tabs},
    Frame, Terminal,
};
use std::{
    cmp::Reverse,
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
        const NONE_TEXT: &str = "<none>";

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
                const TITLE_GUID: &str = "GUID";
                const TITLE_TOPIC: &str = "topic";
                const TITLE_SERIAL_NUMBER: &str = "sn";
                const TITLE_MESSAGE_COUNT: &str = "msg count";
                const TITLE_NUM_FRAGMENTED_MESSAGES: &str = "# frag msgs";

                let mut entities: Vec<_> = state.participants.iter().collect();
                entities.sort_by_cached_key(|(guid, entity)| {
                    let topic_name = entity
                        .topic_info
                        .as_ref()
                        .map(|info| info.publication_topic_data.topic_name.to_string());
                    (Reverse(topic_name), *guid)
                });

                let rows: Vec<_> = entities
                    .iter()
                    .map(|(guid, entity)| {
                        let ParticipantState {
                            ref topic_info,
                            last_sn,
                            message_count,
                            ref frag_messages,
                            ..
                        } = *entity;

                        let topic_name = topic_info
                            .as_ref()
                            .map(|topic_info| topic_info.publication_topic_data.topic_name.as_str())
                            .unwrap_or(NONE_TEXT);
                        let last_sn = last_sn
                            .map(|sn| format!("{}", sn.0))
                            .unwrap_or_else(|| NONE_TEXT.to_string());

                        Row::new(vec![
                            format!("{}", guid.display()),
                            topic_name.to_string(),
                            last_sn,
                            format!("{message_count}"),
                            format!("{}", frag_messages.len()),
                        ])
                    })
                    .collect();

                let topic_col_len = state
                    .participants
                    .values()
                    .map(|entity| {
                        let Some(info) = entity.topic_info.as_ref() else {
                            return NONE_TEXT.len();
                        };
                        info.publication_topic_data.topic_name.as_str().len()
                    })
                    .max()
                    .unwrap_or(0)
                    .max(TITLE_TOPIC.len());
                let sn_col_len = state
                    .participants
                    .values()
                    .map(|entity| {
                        let Some(last_sn) = entity.last_sn else {
                            return NONE_TEXT.len();
                        };
                        num_base10_digits_i64(last_sn.0) as usize
                    })
                    .max()
                    .unwrap_or(0)
                    .max(TITLE_SERIAL_NUMBER.len());
                let msg_count_col_len = state
                    .participants
                    .values()
                    .map(|entity| num_base10_digits_usize(entity.message_count) as usize)
                    .max()
                    .unwrap_or(0)
                    .max(TITLE_MESSAGE_COUNT.len());
                let num_frag_msgs_col_len = state
                    .participants
                    .values()
                    .map(|entity| num_base10_digits_usize(entity.frag_messages.len()) as usize)
                    .max()
                    .unwrap_or(0)
                    .max(TITLE_NUM_FRAGMENTED_MESSAGES.len());

                let header = Row::new(vec![
                    TITLE_GUID,
                    TITLE_TOPIC,
                    TITLE_SERIAL_NUMBER,
                    TITLE_MESSAGE_COUNT,
                    TITLE_NUM_FRAGMENTED_MESSAGES,
                ]);
                let widths = &[
                    Constraint::Min(35),
                    Constraint::Max(topic_col_len as u16),
                    Constraint::Min(sn_col_len as u16),
                    Constraint::Min(msg_count_col_len as u16),
                    Constraint::Min(num_frag_msgs_col_len as u16),
                ];

                let table_block = Block::default().title("Writers").borders(Borders::ALL);
                let table = Table::new(rows)
                    .style(Style::default().fg(Color::White))
                    .header(header)
                    .block(table_block)
                    .widths(widths)
                    .column_spacing(1)
                    .highlight_style(Style::default().add_modifier(Modifier::BOLD))
                    .highlight_symbol(">");

                frame.render_stateful_widget(table, chunks[1], &mut self.writer_table_state);
            }
            1 => {
                const TITLE_TOPIC: &str = "topic";
                const TITLE_NUM_WRITERS: &str = "# writers";

                let mut topics: Vec<_> = state
                    .participants
                    .iter()
                    .filter_map(|(_, entity)| {
                        let info = entity.topic_info.as_ref()?;
                        let topic_name = info.publication_topic_data.topic_name.as_str();
                        Some((topic_name, entity))
                    })
                    .into_group_map()
                    .into_iter()
                    .collect();
                topics.sort_unstable_by_key(|(topic_name, _)| *topic_name);
                let rows: Vec<_> = topics
                    .into_iter()
                    .map(|(topic_name, entities)| {
                        Row::new(vec![topic_name.to_string(), format!("{}", entities.len())])
                    })
                    .collect();
                let header = Row::new(vec![TITLE_TOPIC, TITLE_NUM_WRITERS]);
                let widths = &[Constraint::Min(35), Constraint::Min(10)];

                let table_block = Block::default().title("Topics").borders(Borders::ALL);
                let table = Table::new(rows)
                    .style(Style::default().fg(Color::White))
                    .header(header)
                    .block(table_block)
                    .widths(widths)
                    .column_spacing(1)
                    .highlight_style(Style::default().add_modifier(Modifier::BOLD))
                    .highlight_symbol(">");

                frame.render_stateful_widget(table, chunks[1], &mut self.topic_table_state);
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
