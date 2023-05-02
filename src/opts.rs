use clap::Parser;
use std::env;

pub const DEFAULT_DOMAIN: u32 = 0;

#[derive(Debug, Clone, Parser)]
pub struct Opts {
    #[clap(long)]
    pub domain_id: Option<u32>,
}

pub fn default_domain() -> u32 {
    if let Ok(s) = env::var("ROS_DOMAIN_ID") {
        s.parse::<u32>().unwrap_or(DEFAULT_DOMAIN)
    } else {
        DEFAULT_DOMAIN
    }
}
