use anyhow::Result;
use clap::Parser;
use ddshark::{opts::Opts, run};

fn main() -> Result<()> {
    run(Opts::parse())
}
