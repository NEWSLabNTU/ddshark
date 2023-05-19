use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
pub struct Opts {
    #[clap(long, default_value = "4")]
    pub refresh_rate: u32,

    #[clap(long)]
    pub file: Option<PathBuf>,

    #[clap(long)]
    pub interface: Option<String>,
}
