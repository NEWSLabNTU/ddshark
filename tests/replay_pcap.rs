//! L3 integration: drive the headless pipeline over an offline `.pcap`.
//!
//! This first test validates the seam — `run_pipeline_headless` + `--exit-on-eof` — by
//! confirming an offline capture drains and the run returns (no hang) with an inspectable
//! `State`. Real-RTPS golden fixtures (asserting discovered participants/topics) land with
//! the L4 recorder.

use clap::Parser;
use ddshark::{opts::Opts, rtps::PacketSource, run_pipeline_headless};
use std::{fs, path::PathBuf};

/// Write a minimal libpcap file (LINKTYPE_ETHERNET) with the given raw frames.
fn write_pcap(path: &PathBuf, frames: &[Vec<u8>]) {
    let mut buf = Vec::new();
    // Global header (little-endian).
    buf.extend_from_slice(&0xa1b2_c3d4u32.to_le_bytes()); // magic
    buf.extend_from_slice(&2u16.to_le_bytes()); // version major
    buf.extend_from_slice(&4u16.to_le_bytes()); // version minor
    buf.extend_from_slice(&0i32.to_le_bytes()); // thiszone
    buf.extend_from_slice(&0u32.to_le_bytes()); // sigfigs
    buf.extend_from_slice(&65535u32.to_le_bytes()); // snaplen
    buf.extend_from_slice(&1u32.to_le_bytes()); // network = ethernet
    for frame in frames {
        let len = frame.len() as u32;
        buf.extend_from_slice(&0u32.to_le_bytes()); // ts_sec (keep equal → no rate sleep)
        buf.extend_from_slice(&0u32.to_le_bytes()); // ts_usec
        buf.extend_from_slice(&len.to_le_bytes()); // incl_len
        buf.extend_from_slice(&len.to_le_bytes()); // orig_len
        buf.extend_from_slice(frame);
    }
    fs::write(path, buf).unwrap();
}

#[test]
fn headless_run_terminates_on_offline_pcap() {
    let path = std::env::temp_dir().join(format!("ddshark_replay_{}.pcap", std::process::id()));
    // A single non-RTPS Ethernet frame: the decoder classifies it as "Other".
    write_pcap(&path, &[vec![0u8; 60]]);

    let opts = Opts::parse_from(["ddshark", "--no-tui", "--exit-on-eof"]);
    let run = run_pipeline_headless(PacketSource::File { path: path.clone() }, &opts)
        .expect("headless pipeline should run to completion");

    let state = run.state.lock().unwrap();
    assert!(
        state.participants.is_empty(),
        "non-RTPS input should yield no participants"
    );

    let _ = fs::remove_file(&path);
}
