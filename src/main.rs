mod dds;
mod opts;
mod qos;
mod state;
mod ui;
mod updater;
mod utils;

use crate::{opts::Opts, state::State};
use anyhow::Result;
use bytes::Bytes;
use clap::Parser;
use pcap::Capture;
use rustdds::{
    messages::submessages::submessages::{EntitySubmessage, SubmessageKind},
    serialization::{message::Message, SubmessageBody},
};
use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use ui::Tui;

fn main() -> Result<()> {
    let opts = Opts::parse();

    let mut cap = Capture::from_file(&opts.input_file)?;

    loop {
        let packet = match cap.next_packet() {
            Ok(packet) => packet,
            Err(pcap::Error::NoMorePackets) => break,
            Err(err) => {
                eprintln!("error: {err:?}");
                continue;
            }
        };

        // println!("{:?}", &packet.data[42..]);

        let payload = &packet.data[42..];
        if payload.get(0..4) != Some(b"RTPS") {
            continue;
        }

        let bytes = Bytes::copy_from_slice(payload);
        let message: Message = match Message::read_from_buffer(&bytes) {
            Ok(msg) => msg,
            Err(err) => {
                eprintln!("error: {err:?}");
                continue;
            }
        };

        for submsg in message.submessages {
            let data = match submsg.body {
                SubmessageBody::Entity(EntitySubmessage::Data(data, _)) => data,
                _ => continue,
            };

            dbg!(data);
        }
    }

    // let domain_id = opts.domain_id.unwrap_or_else(opts::default_domain);
    // let state = Arc::new(Mutex::new(State::default()));
    // let (tx, rx) = flume::bounded(4);

    // // Start DDS discovery processer
    // let dds_handle = thread::spawn(move || {
    //     dds::run_dds_discovery(domain_id, tx).unwrap();
    // });

    // // Start state updater
    // let updater_handle = {
    //     let state = state.clone();
    //     thread::spawn(|| {
    //         crate::updater::run_updater(rx, state);
    //     })
    // };

    // // Run TUI
    // let tick_dur = Duration::from_secs(1) / opts.refresh_rate;
    // let tui = Tui::new(tick_dur, state);
    // tui.run()?;
    // // ui::run_tui(tick_dur, state)?;

    // // Finalize
    // dds_handle.join().unwrap();
    // updater_handle.join().unwrap();

    Ok(())
}
