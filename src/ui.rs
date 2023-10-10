mod tab_abnormality;
mod tab_reader;
mod tab_stat;
mod tab_topic;
mod tab_writer;
mod xtable;

use crate::state::State;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    prelude::*,
    style::{Color, Style},
    symbols::DOT,
    widgets::{Block, Borders, Clear, Paragraph, Tabs},
    Frame, Terminal,
};
use std::{
    io,
    ops::ControlFlow,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::error;

use self::{
    tab_abnormality::{AbnormalityTable, AbnormalityTableState},
    tab_reader::{ReaderTable, ReaderTableState},
    tab_stat::{StatTable, StatTableState},
    tab_topic::{TopicTable, TopicTableState},
    tab_writer::{WriterTable, WriterTableState},
};

const TAB_TITLES: &[&str] = &["Writers", "Reader", "Topics", "Statistics", "Abnormalities"];

pub(crate) struct Tui {
    tab_writer: WriterTableState,
    tab_reader: ReaderTableState,
    tab_topic: TopicTableState,
    tab_stat: StatTableState,
    tab_abnormality: AbnormalityTableState,
    tick_dur: Duration,
    tab_index: usize,
    focus: Focus,
    state: Arc<Mutex<State>>,
}

impl Tui {
    pub fn new(tick_dur: Duration, state: Arc<Mutex<State>>) -> Self {
        Self {
            tick_dur,
            state,
            tab_index: 0,
            tab_writer: WriterTableState::new(),
            tab_topic: TopicTableState::new(),
            tab_abnormality: AbnormalityTableState::new(),
            tab_reader: ReaderTableState::new(),
            tab_stat: StatTableState::new(),
            focus: Focus::Dashboard,
        }
    }

    pub fn run(mut self) -> io::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        terminal.clear()?;

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

    fn run_loop<B>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()>
    where
        B: Backend,
    {
        let mut last_tick = Instant::now();

        loop {
            // Wait for key event
            {
                let timeout = self
                    .tick_dur
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_else(|| Duration::from_secs(0));

                // Process keyboard events
                let ctrl_flow = self.process_events(timeout)?;
                if let ControlFlow::Break(_) = ctrl_flow {
                    break;
                }
            }

            let elapsed_time = last_tick.elapsed();
            if elapsed_time >= self.tick_dur {
                // Draw UI
                terminal.draw(|frame| self.render(frame))?;

                // Clean up state
                last_tick = Instant::now();
            }
        }

        Ok(())
    }

    fn process_events(&mut self, timeout: Duration) -> io::Result<ControlFlow<()>> {
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                use KeyCode as C;

                let n_tabs = TAB_TITLES.len();

                match key.code {
                    C::Char('q') => match self.focus {
                        Focus::Dashboard => return Ok(ControlFlow::Break(())),
                        Focus::Help => self.focus = Focus::Dashboard,
                    },
                    C::Char('h') => self.focus = Focus::Help,
                    C::Up => {
                        self.key_up();
                    }
                    C::Down => {
                        self.key_down();
                    }
                    C::Left => {}
                    C::Right => {}
                    C::PageUp => {
                        self.key_page_up();
                    }
                    C::PageDown => {
                        self.key_page_down();
                    }
                    C::Home => {
                        self.key_home();
                    }
                    C::End => {
                        self.key_end();
                    }
                    C::Tab => {
                        // Jump to next tab
                        self.tab_index = (self.tab_index + 1) % n_tabs;
                    }
                    C::BackTab => {
                        // Go to previous tab
                        self.tab_index = (self.tab_index + (n_tabs - 1)) % n_tabs;
                    }
                    _ => {}
                }
            }
        }

        Ok(ControlFlow::Continue(()))
    }

    fn render<B>(&mut self, frame: &mut Frame<B>)
    where
        B: Backend,
    {
        // Unlock the state
        let Ok(state) = self.state.lock() else {
            // TODO: show error
            error!("State lock is poisoned");
            return;
        };

        // Split the screen vertically into two chunks.
        let content_height = frame.size().height.saturating_sub(2);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints(
                [
                    Constraint::Min(1),
                    Constraint::Length(content_height),
                    Constraint::Min(1),
                ]
                .as_ref(),
            )
            .split(frame.size());

        // Build the container for tabs
        let tabs_block = Block::default();
        let tabs = Tabs::new(TAB_TITLES.to_vec())
            .block(tabs_block)
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow))
            .divider(DOT)
            .select(self.tab_index);
        frame.render_widget(tabs, chunks[0]);

        // Render the tab content according to the current tab index.
        match self.tab_index {
            0 => frame.render_stateful_widget(
                WriterTable::new(&state),
                chunks[1],
                &mut self.tab_writer,
            ),
            1 => frame.render_stateful_widget(
                ReaderTable::new(&state),
                chunks[1],
                &mut self.tab_reader,
            ),
            2 => frame.render_stateful_widget(
                TopicTable::new(&state),
                chunks[1],
                &mut self.tab_topic,
            ),
            3 => {
                frame.render_stateful_widget(StatTable::new(&state), chunks[1], &mut self.tab_stat);
            }

            4 => frame.render_stateful_widget(
                AbnormalityTable::new(&state),
                chunks[1],
                &mut self.tab_abnormality,
            ),
            _ => unreachable!(),
        }

        // Render the bottom tray
        let tray_block = Block::default();
        let tray = Paragraph::new("Q: Exit  H: Help  TAB: Next tab").block(tray_block);
        frame.render_widget(tray, chunks[2]);

        // Render dialogs
        match self.focus {
            Focus::Dashboard => {}
            Focus::Help => {
                Self::render_help_dialog(frame);
            }
        }
    }

    fn render_help_dialog<B>(frame: &mut Frame<B>)
    where
        B: Backend,
    {
        let text = format!(
            "\
            ddshark {}
- (C) 2023 Lin Hsiang-Jui, Taiyou Kuo
- (C) 2023 NEWSLAB, Depart. of CSIE, National Taiwan University

TAB       Next tab
Shift+TAB Previous tab
↑         Previous item
↓         Next item
PageUp    Previous page
PageDown  Next page
h         Show help
a         Close dialog or exit
q         Close dialog or exit
",
            env!("CARGO_PKG_VERSION")
        );

        let area = centered_rect(50, 50, frame.size());
        let block = Block::default()
            .title("Help")
            .borders(Borders::ALL)
            .on_blue();
        let dialog = Paragraph::new(text).block(block);

        frame.render_widget(Clear, area);
        frame.render_widget(dialog, area);
    }

    fn key_up(&mut self) {
        match self.tab_index {
            0 => self.tab_writer.previous_item(),
            1 => self.tab_reader.previous_item(),
            2 => self.tab_topic.previous_item(),
            3 => self.tab_stat.previous_item(),
            4 => self.tab_abnormality.previous_item(),
            _ => unreachable!(),
        }
    }

    fn key_down(&mut self) {
        match self.tab_index {
            0 => self.tab_writer.next_item(),
            1 => self.tab_reader.next_item(),
            2 => self.tab_topic.next_item(),
            3 => self.tab_stat.next_item(),
            4 => self.tab_abnormality.next_item(),
            _ => unreachable!(),
        }
    }

    fn key_page_up(&mut self) {
        match self.tab_index {
            0 => self.tab_writer.previous_page(),
            1 => self.tab_reader.previous_page(),
            2 => self.tab_topic.previous_page(),
            3 => self.tab_stat.previous_page(),
            4 => self.tab_abnormality.previous_page(),
            _ => unreachable!(),
        }
    }

    fn key_page_down(&mut self) {
        match self.tab_index {
            0 => self.tab_writer.next_page(),
            1 => self.tab_reader.next_page(),
            2 => self.tab_topic.next_page(),
            3 => self.tab_stat.next_page(),
            4 => self.tab_abnormality.next_page(),
            _ => unreachable!(),
        }
    }

    fn key_home(&mut self) {
        match self.tab_index {
            0 => self.tab_writer.first_item(),
            1 => self.tab_reader.first_item(),
            2 => self.tab_topic.first_item(),
            3 => self.tab_stat.first_item(),
            4 => self.tab_abnormality.first_item(),
            _ => unreachable!(),
        }
    }

    fn key_end(&mut self) {
        match self.tab_index {
            0 => self.tab_writer.last_item(),
            1 => self.tab_reader.last_item(),
            2 => self.tab_topic.last_item(),
            3 => self.tab_stat.last_item(),
            4 => self.tab_abnormality.last_item(),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Focus {
    Dashboard,
    Help,
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
