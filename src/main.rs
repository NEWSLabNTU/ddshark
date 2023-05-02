mod dds;
mod opts;
mod qos;
mod ui;
mod utils;

use crate::opts::Opts;
use anyhow::Result;
use clap::Parser;
use std::time::Duration;

fn main() -> Result<()> {
    let opts = Opts::parse();
    let domain_id = opts.domain_id.unwrap_or_else(opts::default_domain);

    let (tx, rx) = flume::bounded(4);
    dds::run_discovery(domain_id, tx);

    let tick_rate = Duration::from_millis(250);
    ui::run_tui(tick_rate, rx)?;

    Ok(())
}
