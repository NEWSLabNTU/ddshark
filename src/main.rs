mod logger;
mod message;
mod opts;
mod otlp;
mod rtps;
mod state;
mod ui;
mod updater;
mod utils;
// mod qos;
// mod dds;

use crate::{opts::Opts, state::State};
use anyhow::{bail, Result};
use clap::Parser;
use rtps::PacketSource;
use std::{
    io,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tracing::warn;
use ui::Tui;

fn main() -> Result<()> {
    let opts = Opts::parse();
    let Opts {
        refresh_rate,
        no_tui,
        fast_replay,
        ..
    } = opts;

    if no_tui {
        tracing_subscriber::fmt().with_writer(io::stderr).init();
    }

    let (tx, rx) = flume::unbounded();
    let state = Arc::new(Mutex::new(State::default()));

    let rpts_watcher_handle = {
        let packet_src = match (&opts.file, &opts.interface) {
            (Some(_), Some(_)) => {
                bail!("--file and --interface cannot be specified simultaneously")
            }
            (Some(file), None) => PacketSource::File {
                path: file.clone(),
                sync_time: !fast_replay,
            },
            (None, Some(interface)) => {
                if fast_replay {
                    warn!("--fast-replay has no effect in conjunction with --interface");
                }
                PacketSource::Interface(interface.clone())
            }
            (None, None) => PacketSource::Default,
        };

        thread::spawn(|| rtps::rtps_watcher(packet_src, tx))
    };

    // Start state updater
    let updater_handle = {
        let state = state.clone();

        thread::spawn(move || {
            let updater = crate::updater::Updater::new(rx, state, &opts);
            updater.run();
        })
    };

    // Run TUI
    if !no_tui {
        let tick_dur = Duration::from_secs(1) / refresh_rate;
        let tui = Tui::new(tick_dur, state);
        tui.run()?;
    }

    // Finalize
    rpts_watcher_handle.join().unwrap()?;
    updater_handle.join().unwrap();

    Ok(())
}
