use clap::Parser;
use std::path::PathBuf;

/// A quick DDS sniffer.
#[derive(Debug, Clone, Parser)]
pub struct Opts {
    #[clap(long, default_value = "4")]
    pub refresh_rate: u32,

    /// The input packet dump to be inspected.
    #[clap(short = 'f', long)]
    pub file: Option<PathBuf>,

    /// The network interface to be inspected.
    #[clap(short = 'i', long)]
    pub interface: Option<String>,

    /// Enable OTLP logging.
    #[clap(short = 'o', long)]
    pub otlp: bool,

    /// Set the OTLP endpoint.
    #[clap(short = 'e', long, default_value = "http://localhost:4317")]
    pub otlp_endpoint: Option<String>,

    /// Disable text user interface.
    #[clap(long)]
    pub no_tui: bool,

    /// Do not respect the timestamps in the packet dump.This option
    /// is effective only when --file is used.
    #[clap(long)]
    pub fast_replay: bool,
}
