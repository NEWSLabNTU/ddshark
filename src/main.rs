mod dds;
mod opts;
mod qos;
mod utils;

use anyhow::Result;
use clap::Parser;
use cyclors as cy;
use futures::StreamExt;
use opts::Opts;
use std::ptr;

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();
    let domain_id = opts.domain_id.unwrap_or_else(opts::default_domain);

    let (tx, rx) = flume::bounded(4);
    let dp = unsafe { cy::dds_create_participant(domain_id, ptr::null(), ptr::null()) };
    dds::run_discovery(dp, tx);

    rx.into_stream()
        .for_each(|msg| async {
            dbg!(msg);
        })
        .await;

    Ok(())
}
