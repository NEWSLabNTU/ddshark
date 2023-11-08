mod config;
mod logger;
mod message;
mod opts;
mod otlp;
mod rtps;
mod rtps_watcher;
mod state;
mod ui;
mod updater;
mod utils;
// mod qos;
// mod dds;

use crate::{opts::Opts, state::State};
use anyhow::{bail, Result};
use clap::Parser;
use futures::future;
use rtps::PacketSource;
use std::{
    future::Future,
    io, mem,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;
use ui::Tui;

fn main() -> Result<()> {
    let opts = Opts::parse();

    // If TUI is disabled, show debug messages.
    if opts.no_tui {
        tracing_subscriber::fmt().with_writer(io::stderr).init();
    }

    let state = Arc::new(Mutex::new(State::default()));
    let cancel_token = CancellationToken::new();

    // Set Ctrl-C handler
    {
        let cancel_token = cancel_token.clone();
        ctrlc::set_handler(move || {
            cancel_token.cancel();
        })?;
    }

    let (tx, rx) = flume::bounded(64);

    let backend_handle = {
        let opts = opts.clone();
        let state = state.clone();
        let cancel_token = cancel_token.clone();

        let rpts_watcher_task = {
            let packet_src = match (&opts.file, &opts.interface) {
                (Some(_), Some(_)) => {
                    bail!("--file and --interface cannot be specified simultaneously")
                }
                (Some(file), None) => PacketSource::File { path: file.clone() },
                (None, Some(interface)) => PacketSource::Interface(interface.clone()),
                (None, None) => PacketSource::Default,
            };

            let watcher = rtps_watcher::rtps_watcher(packet_src, tx.clone(), cancel_token.clone());
            spawn(cancel_token.clone(), watcher)
        };

        // Start state updater
        let updater_task = {
            let state = state.clone();

            let updater = crate::updater::Updater::new(rx, cancel_token.clone(), state, &opts)?;
            spawn(cancel_token.clone(), updater.run())
        };

        let future = future::try_join(rpts_watcher_task, updater_task);

        thread::spawn(move || -> Result<()> {
            let rt = Runtime::new()?;
            rt.block_on(future)?;
            Ok(())
        })
    };

    // Run TUI
    if !opts.no_tui {
        let tick_dur = Duration::from_secs(1) / opts.refresh_rate;
        let tui = Tui::new(tick_dur, tx, cancel_token, state);
        tui.run()?;
    } else {
        mem::drop(tx);
    }

    // Finalize
    backend_handle.join().unwrap()?;

    Ok(())
}

async fn spawn<T, E, F>(cancel_token: CancellationToken, future: F) -> Result<T>
where
    F: Future<Output = Result<T, E>> + Send + 'static,
    T: Send + 'static,
    E: Sync + Send + Into<anyhow::Error> + 'static,
{
    match tokio::spawn(future).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(err)) => Err(err.into()),
        Err(join_err) => {
            cancel_token.cancel();
            Err(join_err.into())
        }
    }
}
