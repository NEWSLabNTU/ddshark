//! Command-line options.

use clap::Parser;
use std::path::PathBuf;

/// A quick DDS sniffer.
#[derive(Debug, Clone, Parser)]
pub struct Opts {
    #[clap(long, default_value = "4")]
    pub refresh_rate: u32,

    /// Size of the message buffer between components.
    #[clap(long, default_value = "1024")]
    pub buffer_size: usize,

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

    /// Start logging when the program starts.
    #[clap(long)]
    pub log_on_start: bool,

    /// Enable metrics logging to file.
    #[clap(long)]
    pub metrics_log: bool,

    /// Metrics log file path.
    #[clap(long, default_value = "ddshark-metrics.csv")]
    pub metrics_log_file: PathBuf,
}
