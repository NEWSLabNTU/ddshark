mod message;
mod opts;
mod rtps;
mod state;
mod updater;
mod utils;
// mod ui;
// mod qos;
// mod dds;

use crate::{opts::Opts, state::State};
use anyhow::{bail, Result};
use clap::Parser;
use rtps::PacketSource;
use std::{
    sync::{Arc, Mutex},
    thread,
};
// use ui::Tui;

fn main() -> Result<()> {
    let opts = Opts::parse();
    let (tx, rx) = flume::bounded(8192);
    let state = Arc::new(Mutex::new(State::default()));

    let rpts_watcher_handle = {
        let packet_src = match (opts.file, opts.interface) {
            (Some(_), Some(_)) => {
                bail!("--file and --interface cannot be specified simultaneously")
            }
            (Some(file), None) => PacketSource::File(file),
            (None, Some(interface)) => PacketSource::Interface(interface),
            (None, None) => PacketSource::Default,
        };

        thread::spawn(|| rtps::rtps_watcher(packet_src, tx))
    };

    // Start state updater
    let updater_handle = {
        let state = state.clone();
        thread::spawn(|| {
            crate::updater::run_updater(rx, state);
        })
    };

    // Run TUI
    // let tick_dur = Duration::from_secs(1) / opts.refresh_rate;
    // let tui = Tui::new(tick_dur, state);
    // tui.run()?;
    // ui::run_tui(tick_dur, state)?;

    // Finalize
    rpts_watcher_handle.join().unwrap()?;
    updater_handle.join().unwrap();

    Ok(())
}
