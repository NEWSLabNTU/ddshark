use crate::dds::{DdsEntity, DiscoveryEvent};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    collections::HashMap,
    io,
    time::{Duration, Instant},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders},
    Frame, Terminal,
};

/// The TUI state.
struct State {
    pub pub_keys: HashMap<String, DdsEntity>,
    pub sub_keys: HashMap<String, DdsEntity>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            pub_keys: HashMap::new(),
            sub_keys: HashMap::new(),
        }
    }
}

pub(crate) fn run_tui(tick_rate: Duration, rx: flume::Receiver<DiscoveryEvent>) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = State::default();
    run_loop(&mut terminal, &mut state, tick_rate, rx)?;

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

fn run_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    state: &mut State,
    tick_rate: Duration,
    rx: flume::Receiver<DiscoveryEvent>,
) -> io::Result<()> {
    let mut last_tick = Instant::now();

    'ui_loop: loop {
        // Consume event messages from rx.
        'evt_loop: loop {
            use flume::TryRecvError as E;

            let evt = match rx.try_recv() {
                Ok(evt) => evt,
                Err(E::Disconnected) => break 'ui_loop,
                Err(E::Empty) => break 'evt_loop,
            };

            // TODO: update UI state
            use DiscoveryEvent as D;
            match evt {
                D::DiscoveredPublication { entity } => {
                    state.pub_keys.insert(entity.key.clone(), entity);
                }
                D::UndiscoveredPublication { key } => {
                    state.pub_keys.remove(&key);
                }
                D::DiscoveredSubscription { entity } => {
                    state.sub_keys.insert(entity.key.clone(), entity);
                }
                D::UndiscoveredSubscription { key } => {
                    state.sub_keys.remove(&key);
                }
            };
        }

        // Draw UI
        terminal.draw(|f| draw_ui(f, state))?;

        // Wait for key event
        {
            let timeout = tick_rate
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

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    Ok(())
}

fn draw_ui<B: Backend>(f: &mut Frame<B>, state: &State) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Percentage(10),
                Constraint::Percentage(80),
                Constraint::Percentage(10),
            ]
            .as_ref(),
        )
        .split(f.size());

    let block = Block::default().title("Block").borders(Borders::ALL);
    f.render_widget(block, chunks[0]);

    let block = Block::default().title("Block 2").borders(Borders::ALL);
    f.render_widget(block, chunks[1]);

    // TODO: draw items from `state`
}
