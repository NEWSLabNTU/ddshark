use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
pub struct Opts {
    #[clap(long, default_value = "4")]
    pub refresh_rate: u32,

    #[clap(short = 'f', long)]
    pub file: Option<PathBuf>,

    #[clap(short = 'i', long)]
    pub interface: Option<String>,

    #[clap(short = 'o', long)]
    pub otlp_enable: bool,

    #[clap(short = 'e', long, default_value = "http://localhost:4317")]
    pub otlp_endpoint: Option<String>,

    #[clap(long)]
    pub no_tui: bool,

    #[clap(long)]
    pub fast_replay: bool,
}
