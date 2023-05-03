mod dds;
mod opts;
mod qos;
mod state;
mod ui;
mod updater;
mod utils;

use crate::{opts::Opts, state::State};
use anyhow::Result;
use clap::Parser;
use dds::DdsDiscoveryHandle;
use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use ui::Tui;

fn main() -> Result<()> {
    let opts = Opts::parse();
    let domain_id = opts.domain_id.unwrap_or_else(opts::default_domain);
    let state = Arc::new(Mutex::new(State::default()));
    let (tx, rx) = flume::bounded(4);

    // Start DDS discovery processer
    let dds_handle = DdsDiscoveryHandle::start(domain_id, tx)?;

    // Start state updater
    let updater_handle = {
        let state = state.clone();
        thread::spawn(|| {
            crate::updater::run_updater(rx, state);
        })
    };

    // Run TUI
    let tick_dur = Duration::from_secs(1) / opts.refresh_rate;
    let tui = Tui::new(tick_dur, state);
    tui.run()?;
    // ui::run_tui(tick_dur, state)?;

    // Finalize
    dds_handle.stop();
    updater_handle.join().unwrap();

    Ok(())
}
