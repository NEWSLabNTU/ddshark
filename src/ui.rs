use crate::{
    state::{EntityState, State},
    utils::{num_base10_digits_i64, num_base10_digits_usize, GUIDExt},
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    cmp::Reverse,
    io,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::error;
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table, TableState},
    Frame, Terminal,
};

pub(crate) struct Tui {
    table_state: TableState,
    tick_dur: Duration,
    state: Arc<Mutex<State>>,
}

impl Tui {
    pub fn new(tick_dur: Duration, state: Arc<Mutex<State>>) -> Self {
        Self {
            table_state: TableState::default(),
            tick_dur,
            state,
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
        let mut table_state = TableState::default();
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
                        if let KeyCode::Char('q') = key.code {
                            break;
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

        let block_top = Block::default().title("dashboard").borders(Borders::ALL);
        let block_bottom = Block::default().title("topics").borders(Borders::ALL);

        let mut entities: Vec<_> = state.entities.iter().collect();
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
                let EntityState {
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
                ])
            })
            .collect();

        let topic_col_len = state
            .entities
            .values()
            .map(|entity| {
                let Some(info) = entity.topic_info.as_ref() else {
                    return NONE_TEXT.len();
                };
                info.publication_topic_data.topic_name.as_str().len()
            })
            .min()
            .unwrap_or(0);
        let sn_col_len = state
            .entities
            .values()
            .map(|entity| {
                let Some(last_sn) = entity.last_sn else {
                    return NONE_TEXT.len() as u32;
                };
                num_base10_digits_i64(last_sn.0)
            })
            .max()
            .unwrap_or(0);
        let msg_count_col_len = state
            .entities
            .values()
            .map(|entity| num_base10_digits_usize(entity.message_count))
            .max()
            .unwrap_or(0);

        let header = Row::new(vec!["GUID", "topic", "sn", "msg_count", "fragments"]);
        let widths = &[
            Constraint::Length(32),
            Constraint::Length(topic_col_len as u16),
            Constraint::Length(sn_col_len as u16),
            Constraint::Length(msg_count_col_len as u16),
            Constraint::Length(100),
        ];

        let table = Table::new(rows)
            .style(Style::default().fg(Color::White))
            .header(header)
            .block(block_bottom)
            .widths(widths)
            .column_spacing(1)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol(">>");

        frame.render_widget(block_top, chunks[0]);
        frame.render_stateful_widget(table, chunks[1], &mut self.table_state);
    }
}
