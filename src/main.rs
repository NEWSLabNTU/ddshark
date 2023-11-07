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
use anyhow::anyhow;
use anyhow::{bail, Result};
use clap::Parser;
use futures::{join, try_join};
use rtps::PacketSource;
use rtps_watcher::rtps_watcher;
use std::future::Future;
use std::thread;
use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;
use tracing::warn;
use ui::Tui;

fn main() -> Result<()> {
    let opts = Opts::parse();
    // let Opts {
    //     refresh_rate,
    //     no_tui,
    //     fast_replay,
    //     ..
    // } = opts;

    // if no_tui {
    //     tracing_subscriber::fmt().with_writer(io::stderr).init();
    // }

    let state = Arc::new(Mutex::new(State::default()));
    let cancel_token = CancellationToken::new();

    let backend_handle = {
        let opts = opts.clone();
        let state = state.clone();
        let cancel_token = cancel_token.clone();

        thread::spawn(move || -> Result<()> {
            let rt = Runtime::new()?;
            rt.block_on(run_backend(&opts, cancel_token, state))
        })
    };

    // Run TUI
    if !opts.no_tui {
        let tick_dur = Duration::from_secs(1) / opts.refresh_rate;
        let tui = Tui::new(tick_dur, cancel_token, state);
        tui.run()?;
    }

    // Finalize
    backend_handle.join().unwrap()?;

    Ok(())
}

async fn run_backend(
    opts: &Opts,
    cancel_token: CancellationToken,
    state: Arc<Mutex<State>>,
) -> Result<()> {
    let (tx, rx) = flume::bounded(64);

    let rpts_watcher_task = {
        let packet_src = match (&opts.file, &opts.interface) {
            (Some(_), Some(_)) => {
                bail!("--file and --interface cannot be specified simultaneously")
            }
            (Some(file), None) => PacketSource::File {
                path: file.clone(),
                sync_time: !opts.fast_replay,
            },
            (None, Some(interface)) => {
                if opts.fast_replay {
                    warn!("--fast-replay has no effect in conjunction with --interface");
                }
                PacketSource::Interface(interface.clone())
            }
            (None, None) => PacketSource::Default,
        };

        let watcher = rtps_watcher::rtps_watcher(packet_src, tx);
        spawn(cancel_token.clone(), watcher)
    };

    // Start state updater
    let updater_task = {
        let state = state.clone();
        // let cancel_token = cancel_token.clone();

        let updater = crate::updater::Updater::new(rx, cancel_token.clone(), state, &opts);
        spawn(cancel_token.clone(), updater.run())
    };

    try_join!(rpts_watcher_task, updater_task)?;

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
