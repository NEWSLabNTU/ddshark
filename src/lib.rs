//! ddshark — passive RTPS/DDS traffic monitor.
//!
//! The library exposes the pipeline so integration tests (and the binary) can drive it.

// Much of the lock-free state path, OTLP metrics, and capability helpers are
// intentionally unused scaffolding for in-progress migrations.
#![allow(dead_code)]

pub mod capabilities;
pub mod config;
pub mod lockfree_state;
pub mod logger;
pub mod message;
pub mod metrics;
pub mod metrics_logger;
pub mod opts;
pub mod otlp;
// pub mod otlp_metrics;
pub mod rtps;
pub mod rtps_watcher;
pub mod state;
pub mod state_adapter;
pub mod ui;
pub mod updater;
pub mod utils;
// pub mod qos;
// pub mod dds;

use crate::{
    metrics::MetricsCollector, opts::Opts, rtps::PacketSource, state::State, ui::Tui,
    updater::Updater,
};
use anyhow::{bail, Result};
use futures::future::{self, try_join3, BoxFuture};
use std::{
    future::Future,
    io, mem,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

/// Pick the packet source from CLI options.
pub fn packet_source_from_opts(opts: &Opts) -> Result<PacketSource> {
    match (&opts.file, &opts.interface) {
        (Some(_), Some(_)) => bail!("--file and --interface cannot be specified simultaneously"),
        (Some(file), None) => Ok(PacketSource::File { path: file.clone() }),
        (None, Some(interface)) => Ok(PacketSource::Interface(interface.clone())),
        (None, None) => Ok(PacketSource::Default),
    }
}

/// Run the full application (TUI unless `--no-tui`). This is the binary's entry point.
pub fn run(opts: Opts) -> Result<()> {
    // If TUI is disabled, show debug messages.
    if opts.no_tui {
        tracing_subscriber::fmt().with_writer(io::stderr).init();
    }

    let state = Arc::new(Mutex::new(State::default()));
    let cancel_token = CancellationToken::new();
    let metrics = MetricsCollector::new();

    // Exit at EOF for offline headless runs; keep the TUI up otherwise.
    let exit_on_eof = opts.exit_on_eof || (opts.file.is_some() && opts.no_tui);

    // Set Ctrl-C handler
    {
        let cancel_token = cancel_token.clone();
        ctrlc::set_handler(move || {
            cancel_token.cancel();
        })?;
    }

    let (tx, rx) = flume::bounded(opts.buffer_size);

    let backend_handle = {
        let opts = opts.clone();
        let state = state.clone();
        let cancel_token = cancel_token.clone();

        let rpts_watcher_task = {
            let packet_src = packet_source_from_opts(&opts)?;
            let watcher = rtps_watcher::rtps_watcher(
                packet_src,
                tx.clone(),
                cancel_token.clone(),
                metrics.clone(),
                exit_on_eof,
            );
            spawn(cancel_token.clone(), watcher)
        };

        // Start state updater
        let updater_task = {
            let state = state.clone();
            let updater = Updater::new(rx, cancel_token.clone(), state, &opts, metrics.clone())?;
            spawn(cancel_token.clone(), updater.run())
        };

        // Start metrics logger if enabled
        let metrics_logger_task = if opts.metrics_log {
            let logger_handle = crate::metrics_logger::spawn_metrics_logger(
                metrics.clone(),
                opts.metrics_log_file.clone(),
                cancel_token.clone(),
            )?;
            Some(spawn(cancel_token.clone(), async move {
                logger_handle.await.map_err(anyhow::Error::from)?
            }))
        } else {
            None
        };

        let future: BoxFuture<'_, Result<()>> = if let Some(logger_task) = metrics_logger_task {
            Box::pin(async move {
                try_join3(rpts_watcher_task, updater_task, logger_task)
                    .await
                    .map(|_| ())
            })
        } else {
            Box::pin(async move {
                future::try_join(rpts_watcher_task, updater_task)
                    .await
                    .map(|_| ())
            })
        };

        thread::spawn(move || -> Result<()> {
            let rt = Runtime::new()?;
            rt.block_on(future)?;
            Ok(())
        })
    };

    // Run TUI
    if !opts.no_tui {
        let tick_dur = Duration::from_secs(1) / opts.refresh_rate;
        let tui = Tui::new(tick_dur, tx, cancel_token, state, metrics);
        tui.run()?;
    } else {
        mem::drop(tx);
    }

    // Finalize
    backend_handle.join().unwrap()?;

    Ok(())
}

/// The result of a headless pipeline run: the final state and the metrics collector.
pub struct HeadlessRun {
    pub state: Arc<Mutex<State>>,
    pub metrics: MetricsCollector,
}

/// Run the watcher + updater over `source` to completion (no TUI), returning the final
/// state and metrics. Intended for integration tests. The source is run with
/// `exit_on_eof = true`, so an offline `.pcap` drains and the pipeline terminates.
pub fn run_pipeline_headless(source: PacketSource, opts: &Opts) -> Result<HeadlessRun> {
    let state = Arc::new(Mutex::new(State::default()));
    let cancel_token = CancellationToken::new();
    let metrics = MetricsCollector::new();
    let (tx, rx) = flume::bounded(opts.buffer_size);

    let watcher = rtps_watcher::rtps_watcher(
        source,
        tx,
        cancel_token.clone(),
        metrics.clone(),
        true, // exit on EOF so the run terminates
    );
    let updater = Updater::new(
        rx,
        cancel_token.clone(),
        state.clone(),
        opts,
        metrics.clone(),
    )?;

    let rt = Runtime::new()?;
    rt.block_on(async {
        future::try_join(
            spawn(cancel_token.clone(), watcher),
            spawn(cancel_token.clone(), updater.run()),
        )
        .await
        .map(|_| ())
    })?;

    Ok(HeadlessRun { state, metrics })
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
