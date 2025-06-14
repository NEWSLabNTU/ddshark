//! The text-user-interface.

mod tab_abnormality;
mod tab_participant;
mod tab_reader;
mod tab_stat;
mod tab_topic;
mod tab_writer;
mod value;
mod xtable;

use self::{
    tab_abnormality::{AbnormalityTable, AbnormalityTableState},
    tab_participant::{ParticipantTable, ParticipantTableState},
    tab_reader::{ReaderTable, ReaderTableState},
    tab_stat::{StatTable, StatTableState},
    tab_topic::{TopicTable, TopicTableState},
    tab_writer::{WriterTable, WriterTableState},
};
use crate::{message::UpdateEvent, metrics::MetricsCollector, state::State};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use flume::SendTimeoutError;
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
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};

const TAB_TITLES: &[&str] = &[
    "Participants",
    "Writers",
    "Reader",
    "Topics",
    "Statistics",
    "Metrics",
    "Abnormalities",
];
const TAB_IDX_PARTICIPANT: usize = 0;
const TAB_IDX_WRITER: usize = 1;
const TAB_IDX_READER: usize = 2;
const TAB_IDX_TOPIC: usize = 3;
const TAB_IDX_STATISTICS: usize = 4;
const TAB_IDX_METRICS: usize = 5;
const TAB_IDX_ABNORMALITIES: usize = 6;

pub(crate) struct Tui {
    tab_participant: ParticipantTableState,
    tab_writer: WriterTableState,
    tab_reader: ReaderTableState,
    tab_topic: TopicTableState,
    tab_stat: StatTableState,
    tab_abnormality: AbnormalityTableState,
    tick_dur: Duration,
    tab_index: usize,
    focus: Focus,
    cancel_token: CancellationToken,
    tx: flume::Sender<UpdateEvent>,
    state: Arc<Mutex<State>>,
    metrics: MetricsCollector,
}

impl Tui {
    pub fn new(
        tick_dur: Duration,
        tx: flume::Sender<UpdateEvent>,
        cancel_token: CancellationToken,
        state: Arc<Mutex<State>>,
        metrics: MetricsCollector,
    ) -> Self {
        Self {
            tx,
            tick_dur,
            state,
            cancel_token,
            tab_index: 0,
            tab_participant: ParticipantTableState::new(),
            tab_writer: WriterTableState::new(),
            tab_topic: TopicTableState::new(),
            tab_abnormality: AbnormalityTableState::new(),
            tab_reader: ReaderTableState::new(),
            tab_stat: StatTableState::new(),
            focus: Focus::Dashboard,
            metrics,
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
        terminal.clear()?;
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

        while !self.cancel_token.is_cancelled() {
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
        assert!(!self.cancel_token.is_cancelled());

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                use KeyCode as C;

                let n_tabs = TAB_TITLES.len();

                match key.code {
                    C::Char('q') => match self.focus {
                        Focus::Dashboard => {
                            self.cancel_token.cancel();
                            return Ok(ControlFlow::Break(()));
                        }
                        Focus::Help => self.focus = Focus::Dashboard,
                    },
                    C::Char('h') => self.focus = Focus::Help,
                    C::Char('s') => {
                        self.toggle_sort();
                    }
                    C::Char('v') => {
                        self.toggle_show();
                    }
                    C::Char('r') => {
                        if let ControlFlow::Break(()) = self.toggle_logging() {
                            return Ok(ControlFlow::Break(()));
                        }
                    }
                    C::Up => {
                        self.key_up();
                    }
                    C::Down => {
                        self.key_down();
                    }
                    C::Left => {
                        self.key_left();
                    }
                    C::Right => {
                        self.key_right();
                    }
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
        // dbg!(state.participants.len());

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
            TAB_IDX_PARTICIPANT => frame.render_stateful_widget(
                ParticipantTable::new(&state),
                chunks[1],
                &mut self.tab_participant,
            ),
            TAB_IDX_WRITER => frame.render_stateful_widget(
                WriterTable::new(&state),
                chunks[1],
                &mut self.tab_writer,
            ),
            TAB_IDX_READER => frame.render_stateful_widget(
                ReaderTable::new(&state),
                chunks[1],
                &mut self.tab_reader,
            ),
            TAB_IDX_TOPIC => frame.render_stateful_widget(
                TopicTable::new(&state),
                chunks[1],
                &mut self.tab_topic,
            ),
            TAB_IDX_STATISTICS => {
                frame.render_stateful_widget(StatTable::new(&state), chunks[1], &mut self.tab_stat);
            }
            TAB_IDX_METRICS => {
                Self::render_metrics_panel(frame, chunks[1], &self.metrics);
            }
            TAB_IDX_ABNORMALITIES => frame.render_stateful_widget(
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
←         Previous column
→         Next column
PageUp    Previous page
PageDown  Next page
h         Show help
s         Sort by selected column
v         Hide/Show column
r         Enable/Disable data logging
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
            TAB_IDX_PARTICIPANT => self.tab_participant.previous_item(),
            TAB_IDX_WRITER => self.tab_writer.previous_item(),
            TAB_IDX_READER => self.tab_reader.previous_item(),
            TAB_IDX_TOPIC => self.tab_topic.previous_item(),
            TAB_IDX_STATISTICS => self.tab_stat.previous_item(),
            TAB_IDX_METRICS => {} // No navigation for metrics panel
            TAB_IDX_ABNORMALITIES => self.tab_abnormality.previous_item(),
            _ => unreachable!(),
        }
    }

    fn key_down(&mut self) {
        match self.tab_index {
            TAB_IDX_PARTICIPANT => self.tab_participant.next_item(),
            TAB_IDX_WRITER => self.tab_writer.next_item(),
            TAB_IDX_READER => self.tab_reader.next_item(),
            TAB_IDX_TOPIC => self.tab_topic.next_item(),
            TAB_IDX_STATISTICS => self.tab_stat.next_item(),
            TAB_IDX_METRICS => {} // No navigation for metrics panel
            TAB_IDX_ABNORMALITIES => self.tab_abnormality.next_item(),
            _ => unreachable!(),
        }
    }

    fn key_page_up(&mut self) {
        match self.tab_index {
            TAB_IDX_PARTICIPANT => self.tab_participant.previous_page(),
            TAB_IDX_WRITER => self.tab_writer.previous_page(),
            TAB_IDX_READER => self.tab_reader.previous_page(),
            TAB_IDX_TOPIC => self.tab_topic.previous_page(),
            TAB_IDX_STATISTICS => self.tab_stat.previous_page(),
            TAB_IDX_METRICS => {} // No navigation for metrics panel
            TAB_IDX_ABNORMALITIES => self.tab_abnormality.previous_page(),
            _ => unreachable!(),
        }
    }

    fn key_page_down(&mut self) {
        match self.tab_index {
            TAB_IDX_PARTICIPANT => self.tab_participant.next_page(),
            TAB_IDX_WRITER => self.tab_writer.next_page(),
            TAB_IDX_READER => self.tab_reader.next_page(),
            TAB_IDX_TOPIC => self.tab_topic.next_page(),
            TAB_IDX_STATISTICS => self.tab_stat.next_page(),
            TAB_IDX_METRICS => {} // No navigation for metrics panel
            TAB_IDX_ABNORMALITIES => self.tab_abnormality.next_page(),
            _ => unreachable!(),
        }
    }

    fn key_home(&mut self) {
        match self.tab_index {
            TAB_IDX_PARTICIPANT => self.tab_participant.first_item(),
            TAB_IDX_WRITER => self.tab_writer.first_item(),
            TAB_IDX_READER => self.tab_reader.first_item(),
            TAB_IDX_TOPIC => self.tab_topic.first_item(),
            TAB_IDX_STATISTICS => self.tab_stat.first_item(),
            TAB_IDX_METRICS => {} // No navigation for metrics panel
            TAB_IDX_ABNORMALITIES => self.tab_abnormality.first_item(),
            _ => unreachable!(),
        }
    }

    fn key_end(&mut self) {
        match self.tab_index {
            TAB_IDX_PARTICIPANT => self.tab_participant.last_item(),
            TAB_IDX_WRITER => self.tab_writer.last_item(),
            TAB_IDX_READER => self.tab_reader.last_item(),
            TAB_IDX_TOPIC => self.tab_topic.last_item(),
            TAB_IDX_STATISTICS => self.tab_stat.last_item(),
            TAB_IDX_METRICS => {} // No navigation for metrics panel
            TAB_IDX_ABNORMALITIES => self.tab_abnormality.last_item(),
            _ => unreachable!(),
        }
    }

    fn key_left(&mut self) {
        match self.tab_index {
            TAB_IDX_PARTICIPANT => self.tab_participant.previous_column(),
            TAB_IDX_WRITER => self.tab_writer.previous_column(),
            TAB_IDX_READER => self.tab_reader.previous_column(),
            TAB_IDX_TOPIC => self.tab_topic.previous_column(),
            TAB_IDX_STATISTICS => self.tab_stat.previous_column(),
            TAB_IDX_METRICS => {} // No navigation for metrics panel
            TAB_IDX_ABNORMALITIES => self.tab_abnormality.previous_column(),
            _ => unreachable!(),
        }
    }

    fn key_right(&mut self) {
        match self.tab_index {
            TAB_IDX_PARTICIPANT => self.tab_participant.next_column(),
            TAB_IDX_WRITER => self.tab_writer.next_column(),
            TAB_IDX_READER => self.tab_reader.next_column(),
            TAB_IDX_TOPIC => self.tab_topic.next_column(),
            TAB_IDX_STATISTICS => self.tab_stat.next_column(),
            TAB_IDX_METRICS => {} // No navigation for metrics panel
            TAB_IDX_ABNORMALITIES => self.tab_abnormality.next_column(),
            _ => unreachable!(),
        }
    }

    fn toggle_show(&mut self) {
        match self.tab_index {
            TAB_IDX_PARTICIPANT => self.tab_participant.toggle_show(),
            TAB_IDX_WRITER => self.tab_writer.toggle_show(),
            TAB_IDX_READER => self.tab_reader.toggle_show(),
            TAB_IDX_TOPIC => self.tab_topic.toggle_show(),
            TAB_IDX_STATISTICS => self.tab_stat.toggle_show(),
            TAB_IDX_METRICS => {} // No navigation for metrics panel
            TAB_IDX_ABNORMALITIES => self.tab_abnormality.toggle_show(),
            _ => unreachable!(),
        }
    }

    fn toggle_sort(&mut self) {
        match self.tab_index {
            TAB_IDX_PARTICIPANT => self.tab_participant.toggle_sort(),
            TAB_IDX_WRITER => self.tab_writer.toggle_sort(),
            TAB_IDX_READER => self.tab_reader.toggle_sort(),
            TAB_IDX_TOPIC => self.tab_topic.toggle_sort(),
            TAB_IDX_STATISTICS => self.tab_stat.toggle_sort(),
            TAB_IDX_METRICS => {} // No navigation for metrics panel
            TAB_IDX_ABNORMALITIES => self.tab_abnormality.toggle_sort(),
            _ => unreachable!(),
        }
    }

    fn toggle_logging(&self) -> ControlFlow<()> {
        let timeout = Duration::from_millis(100);
        let result = self.tx.send_timeout(UpdateEvent::ToggleLogging, timeout);

        type E<T> = SendTimeoutError<T>;

        match result {
            Ok(()) => ControlFlow::Continue(()),
            Err(E::Disconnected(_)) => ControlFlow::Break(()),
            Err(E::Timeout(_)) => {
                warn!("congestion occurs");
                ControlFlow::Continue(())
            }
        }
    }

    fn render_metrics_panel<B>(frame: &mut Frame<B>, area: Rect, metrics: &MetricsCollector)
    where
        B: Backend,
    {
        let snapshot = metrics.snapshot();

        // Create vertical layout for different metric categories
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8), // Performance overview
                Constraint::Length(6), // Throughput metrics
                Constraint::Length(6), // Queue metrics
                Constraint::Length(6), // Latency metrics
                Constraint::Min(1),    // Additional space
            ])
            .split(area);

        // Performance Overview
        let overview_text = format!(
            "Uptime: {:?}\nPackets: {} received, {} parsed ({} errors)\nRTPS Messages: {}\nDrop Rate: {:.2}%",
            snapshot.uptime,
            snapshot.packets_received,
            snapshot.packets_parsed,
            snapshot.parse_errors,
            snapshot.rtps_messages_found,
            snapshot.drop_rate,
        );
        let overview_block = Block::default()
            .title("Performance Overview")
            .borders(Borders::ALL);
        let overview = Paragraph::new(overview_text).block(overview_block);
        frame.render_widget(overview, chunks[0]);

        // Throughput Metrics
        let throughput_text = format!(
            "Packet Rate: {:.1} packets/sec\nMessage Rate: {:.1} messages/sec\nProcessing Rate: {:.1} messages/sec\nSend Timeouts: {}",
            snapshot.packet_rate,
            snapshot.message_rate,
            snapshot.processing_rate,
            snapshot.send_timeouts,
        );
        let throughput_block = Block::default().title("Throughput").borders(Borders::ALL);
        let throughput = Paragraph::new(throughput_text).block(throughput_block);
        frame.render_widget(throughput, chunks[1]);

        // Queue Metrics
        let queue_text = format!(
            "Current Queue Depth: {}\nMax Queue Depth: {}\nMessages Sent: {}\nMessages Dropped: {}",
            snapshot.queue_depth,
            snapshot.max_queue_depth,
            snapshot.messages_sent,
            snapshot.messages_dropped,
        );
        let queue_block = Block::default().title("Queue Status").borders(Borders::ALL);
        let queue = Paragraph::new(queue_text).block(queue_block);
        frame.render_widget(queue, chunks[2]);

        // Latency Metrics
        let latency_text = format!(
            "Processing Latency P50: {}μs\nProcessing Latency P99: {}μs\nLock Wait P50: {}μs\nLock Wait P99: {}μs",
            snapshot.processing_latency_p50,
            snapshot.processing_latency_p99,
            snapshot.lock_wait_p50,
            snapshot.lock_wait_p99,
        );
        let latency_block = Block::default().title("Latency").borders(Borders::ALL);
        let latency = Paragraph::new(latency_text).block(latency_block);
        frame.render_widget(latency, chunks[3]);
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
