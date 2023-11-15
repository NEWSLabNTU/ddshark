//! The updater that processes message events and maintains the
//! singleton state.

use crate::{
    config::TICK_INTERVAL,
    logger::Logger,
    message::{
        AckNackEvent, DataEvent, DataFragEvent, DataPayload, GapEvent, HeartbeatEvent,
        HeartbeatFragEvent, NackFragEvent, ParticipantInfo, RtpsSubmsgEvent, RtpsSubmsgEventKind,
        TickEvent, UpdateEvent,
    },
    opts::Opts,
    otlp,
    state::{Abnormality, AckNackState, FragmentedMessage, HeartbeatState, State},
};
use anyhow::Result;
use chrono::Local;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::{select, time::MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, warn};

pub struct Updater {
    rx: flume::Receiver<UpdateEvent>,
    state: Arc<Mutex<State>>,
    otlp_handle: Option<otlp::TraceHandle>,
    cancel_token: CancellationToken,
    logger: Option<Logger>,
}

impl Updater {
    pub(crate) fn new(
        rx: flume::Receiver<UpdateEvent>,
        cancel_token: CancellationToken,
        state: Arc<Mutex<State>>,
        opts: &Opts,
    ) -> Result<Self> {
        // Enable OTLP if `otlp_enable` is true.
        let otlp_handle = match opts.otlp {
            true => Some(otlp::TraceHandle::new(opts)),
            false => None,
        };

        let logger = if opts.log_on_start {
            Some(Logger::new()?)
        } else {
            None
        };

        Ok(Self {
            rx,
            state,
            otlp_handle,
            logger,
            cancel_token,
        })
    }

    pub(crate) async fn run(mut self) -> Result<()> {
        // Wait for the first message
        let (first_instant, first_recv_time) = loop {
            let message = select! {
                _ = self.cancel_token.cancelled() => {
                    return Ok(());
                }
                result = self.rx.recv_async() => {
                    let Ok(msg) = result else {
                        return Ok(());
                    };
                    msg
                }
            };

            let state = self.state.clone();
            let Ok(mut state) = state.lock() else {
                panic!("INTERNAL ERROR Mutex poision error");
            };

            // Remember the difference b/w the current and receipt time.
            let now = Instant::now();
            let recv_time = match &message {
                UpdateEvent::RtpsMsg(_) => todo!(),
                UpdateEvent::RtpsSubmsg(msg) => msg.recv_time,
                UpdateEvent::ParticipantInfo(msg) => msg.recv_time,
                UpdateEvent::Tick(_) => unreachable!(),
                UpdateEvent::ToggleLogging => {
                    self.toggle_logging()?;
                    continue;
                }
            };

            self.handle_message(&mut state, &message)?;

            break (now, recv_time);
        };

        let mut interval = tokio::time::interval(TICK_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        // Loop to process input messages
        loop {
            let message = select! {
                _ = self.cancel_token.cancelled() => {
                    return Ok(());
                }
                now = interval.tick() => {
                    let elapsed = now.duration_since(first_instant.into());
                    let recv_time = first_recv_time + chrono::Duration::from_std(elapsed).unwrap();
                    TickEvent {recv_time, when: now.into() }.into()
                }
                result = self.rx.recv_async() => {
                    let Ok(message) = result else {
                        break;
                    };
                    message
                }
            };

            let state = self.state.clone();
            let Ok(mut state) = state.lock() else {
                error!("INTERNAL ERROR Mutex poision error");
                break;
            };

            self.handle_message(&mut state, &message)?;
        }

        // Turn off logging
        if let Some(logger) = self.logger.take() {
            logger.close()?;
        }

        Ok(())
    }

    fn handle_message(&mut self, state: &mut State, message: &UpdateEvent) -> Result<()> {
        match message {
            UpdateEvent::Tick(msg) => {
                self.handle_tick(state, msg)?;
            }
            UpdateEvent::RtpsMsg(_) => todo!(),
            UpdateEvent::ParticipantInfo(info) => {
                self.handle_participant_info(state, info);
            }
            UpdateEvent::RtpsSubmsg(msg) => match &msg.kind {
                RtpsSubmsgEventKind::Data(event) => {
                    self.handle_data_event(state, msg, event);
                }
                RtpsSubmsgEventKind::DataFrag(event) => {
                    self.handle_data_frag_event(state, msg, event);
                }
                RtpsSubmsgEventKind::Gap(event) => {
                    self.handle_gap_event(state, msg, event);
                }
                RtpsSubmsgEventKind::Heartbeat(event) => {
                    self.handle_heartbeat_event(state, msg, event);
                }
                RtpsSubmsgEventKind::AckNack(event) => {
                    self.handle_acknack_event(state, msg, event);
                }
                RtpsSubmsgEventKind::NackFrag(event) => {
                    self.handle_nackfrag_event(state, msg, event);
                }
                RtpsSubmsgEventKind::HeartbeatFrag(event) => {
                    self.handle_heartbeatfrag_event(state, msg, event);
                }
            },
            UpdateEvent::ToggleLogging => self.toggle_logging()?,
        }

        Ok(())
    }

    fn handle_tick(&mut self, state: &mut State, msg: &TickEvent) -> Result<()> {
        state.tick_since = msg.when;

        let ts = msg.recv_time;

        for participant in state.participants.values_mut() {
            participant.bit_rate_stat.set_last_ts(ts);
            participant.msg_rate_stat.set_last_ts(ts);
            participant.acknack_rate_stat.set_last_ts(ts);

            for writer in participant.writers.values_mut() {
                writer.bit_rate_stat.set_last_ts(ts);
                writer.msg_rate_stat.set_last_ts(ts);
            }

            for reader in participant.readers.values_mut() {
                reader.acknack_rate_stat.set_last_ts(ts);
            }
        }

        for topic in state.topics.values_mut() {
            topic.msg_rate_stat.set_last_ts(ts);
            topic.bit_rate_stat.set_last_ts(ts);
            topic.acknack_rate_stat.set_last_ts(ts);
        }

        if let Some(logger) = &mut self.logger {
            logger.save(state)?;
        }

        Ok(())
    }

    fn handle_data_event(&self, state: &mut State, msg: &RtpsSubmsgEvent, event: &DataEvent) {
        // println!(
        //     "{}\t{}\t{:.2}bps",
        //     event.writer_id.display(),
        //     entity.recv_count,
        //     entity.recv_bitrate()
        // );

        if let Some(payload) = &event.payload {
            match payload {
                DataPayload::Topic(_data) => {
                    debug!("DiscoveredTopic not yet implemented");
                    // let topic_name = data.topic_data.name.clone();
                    // TODO
                }
                DataPayload::Writer(data) => {
                    let remote_writer_guid = data.writer_proxy.remote_writer_guid;
                    // TODO: Find the correct writer
                    assert_eq!(event.writer_guid.prefix, remote_writer_guid.prefix);

                    let participant = state
                        .participants
                        .entry(remote_writer_guid.prefix)
                        .or_default();
                    let writer = participant
                        .writers
                        .entry(remote_writer_guid.entity_id)
                        .or_default();

                    // Update discovered data in state.entities
                    {
                        if let Some(orig_data) = &writer.data {
                            let orig_data = &orig_data.publication_topic_data;
                            let new_data = &data.publication_topic_data;

                            if orig_data.topic_name != new_data.topic_name {
                                state.abnormalities.push(Abnormality {
                                    when: Local::now(),
                                    writer_guid: Some(event.writer_guid),
                                    reader_guid: None,
                                    topic_name: None,
                                    desc: "topic name changed in DiscoveredWriterData".to_string(),
                                });
                            }
                        }

                        writer.data = Some((**data).clone());
                    }

                    // Update stats on associated topic
                    {
                        let topic_name = data.publication_topic_data.topic_name.clone();
                        let topic_state = state.topics.entry(topic_name.clone()).or_default();
                        topic_state.writers.insert(remote_writer_guid);
                    }
                }
                DataPayload::Reader(data) => {
                    let remote_reader_guid = data.reader_proxy.remote_reader_guid;
                    // TODO: Find the correct writer
                    // dbg!(
                    //     event.reader_guid.prefix,
                    //     event.writer_guid.prefix,
                    //     remote_reader_guid.prefix
                    // );
                    assert_eq!(event.writer_guid.prefix, remote_reader_guid.prefix);

                    let participant = state
                        .participants
                        .entry(remote_reader_guid.prefix)
                        .or_default();

                    let reader = participant
                        .readers
                        .entry(remote_reader_guid.entity_id)
                        .or_default();

                    // Update discovered data in state.entities
                    {
                        if let Some(orig_data) = &reader.data {
                            let orig_data = &orig_data.subscription_topic_data;
                            let new_data = &data.subscription_topic_data;

                            if orig_data.topic_name() != new_data.topic_name() {
                                state.abnormalities.push(Abnormality {
                                    when: Local::now(),
                                    writer_guid: Some(event.writer_guid),
                                    reader_guid: None,
                                    topic_name: None,
                                    desc: "topic name changed in DiscoveredWriterData".to_string(),
                                });
                            }
                        }

                        reader.data = Some((**data).clone());
                    }

                    // Update stats on associated topic
                    {
                        let topic_name = data.subscription_topic_data.topic_name().clone();
                        let topic_state = state.topics.entry(topic_name.clone()).or_default();
                        topic_state.readers.insert(remote_reader_guid);
                    }
                }
                DataPayload::Participant(_data) => {
                    debug!("DiscoveredParticipant not yet implemented");
                    // TODO
                }
            }
        }

        // Update general statistics
        state.stat.packet_count += 1;
        state.stat.data_submsg_count += 1;

        {
            let participant = state
                .participants
                .entry(event.writer_guid.prefix)
                .or_default();
            let writer = participant
                .writers
                .entry(event.writer_guid.entity_id)
                .or_default();

            // Update the participant state
            {
                participant.total_msg_count += 1;
                participant.msg_rate_stat.push(msg.recv_time, 1f64);

                participant.total_byte_count += event.payload_size;
                participant
                    .bit_rate_stat
                    .push(msg.recv_time, (event.payload_size * 8) as f64);
            }

            // Update the writer state
            {
                writer.last_sn = Some(event.writer_sn);

                // Increase message count on the writer state
                writer.total_msg_count += 1;
                writer.msg_rate_stat.push(msg.recv_time, 1f64);

                // Increase byte count on the writer state
                writer.total_byte_count += event.payload_size;
                writer
                    .bit_rate_stat
                    .push(msg.recv_time, (event.payload_size * 8) as f64);
            }

            // Update the stat on associated topic.
            if let Some(topic_name) = writer.topic_name() {
                let topic = state.topics.get_mut(topic_name).unwrap();

                topic.total_msg_count += 1;
                topic.msg_rate_stat.push(msg.recv_time, 1f64);

                topic.total_byte_count += event.payload_size;
                topic
                    .bit_rate_stat
                    .push(msg.recv_time, (event.payload_size * 8) as f64);
            }
        }
    }

    fn handle_data_frag_event(
        &self,
        state: &mut State,
        msg: &RtpsSubmsgEvent,
        event: &DataFragEvent,
    ) {
        state.stat.packet_count += 1;
        state.stat.datafrag_submsg_count += 1;

        let DataFragEvent {
            fragment_starting_num,
            fragments_in_submessage,
            writer_guid,
            writer_sn,
            // fragment_size,
            // data_size,
            // payload_size,
            ..
        } = *event;

        let participant = state.participants.entry(writer_guid.prefix).or_default();
        let writer = participant
            .writers
            .entry(writer_guid.entity_id)
            .or_default();

        // println!(
        //     "{}\t{}\t{:.2}bps",
        //     event.writer_id.display(),
        //     entity.recv_count,
        //     entity.recv_bitrate()
        // );

        // let topic_name = entity.topic_name().map(|t| t.to_string());
        let frag_msg = writer.frag_messages.entry(writer_sn).or_insert_with(|| {
            FragmentedMessage::new(event.data_size as usize, event.fragment_size as usize)
        });

        if event.data_size as usize != frag_msg.data_size {
            let desc = format!(
                "data_size changes from {} to {} in DataFrag submsg",
                frag_msg.data_size, event.data_size
            );

            state.abnormalities.push(Abnormality {
                when: Local::now(),
                writer_guid: Some(writer_guid),
                reader_guid: None,
                topic_name: writer.topic_name().map(|t| t.to_string()),
                desc,
            });
            return;
        }

        // Compute the submessage payload range
        let range = {
            let start = fragment_starting_num as usize - 1;
            let end = start + fragments_in_submessage as usize;
            start..end
        };

        let prev_hash = frag_msg.intervals.insert(range.clone(), event.payload_hash);

        // println!(
        //     "datafrag {}\t\
        //      start={fragment_starting_num}\t\
        //      n_msgs={fragments_in_submessage}\t\
        //      data_size={data_size}\t\
        //      frag_size={fragment_size}\t\
        //      payload_size={payload_size}",
        //     writer_id.display()
        // );

        match prev_hash {
            Some(prev_hash) => {
                if prev_hash != event.payload_hash {
                    warn!("DataFrag payload data differs in range {range:?}");
                }
            }
            None => {
                // println!(
                //     "{}|{:04}\t{}\t{}",
                //     event.writer_id.display(),
                //     event.writer_sn.0,
                //     range.start,
                //     range.end
                // );

                let defrag_buf = &mut frag_msg.defrag_buf;
                // let topic_name = topic_name.unwrap_or("<none>".to_string());

                if let Err(_err) = defrag_buf.insert(range.clone()) {
                    // warn!("Unable to insert interval {range:?}");
                    // warn!("{err}");
                    // let free_intervals: Vec<_> = defrag_buf.free_intervals().collect();

                    state.abnormalities.push(Abnormality {
                        when: Local::now(),
                        writer_guid: Some(writer_guid),
                        reader_guid: None,
                        topic_name: writer.topic_name().map(|t| t.to_string()),
                        desc: format!("unable to insert fragment {range:?} into defrag buffer"),
                    });

                    // println!(
                    //     "defrag {}\t{range:?}\t{topic_name}\t{free_intervals:?}\t!",
                    //     writer_id.display()
                    // );

                    return;
                } else {
                    // let free_intervals: Vec<_> = defrag_buf.free_intervals().collect();
                    // println!(
                    //     "defrag {}\t{range:?}\t{topic_name}\t{free_intervals:?}",
                    //     writer_id.display()
                    // );
                }

                frag_msg.recvd_fragments += event.fragments_in_submessage as usize;

                if defrag_buf.is_full() {
                    // Update the participant state
                    {
                        participant.total_msg_count += 1;
                        participant.msg_rate_stat.push(msg.recv_time, 1f64);

                        participant.total_byte_count += event.payload_size;
                        participant
                            .bit_rate_stat
                            .push(msg.recv_time, (event.payload_size * 8) as f64);
                    }

                    // Update the writer state
                    {
                        writer.frag_messages.remove(&event.writer_sn).unwrap();
                        writer.last_sn = Some(event.writer_sn);

                        // Increase message count on writer stat
                        writer.total_msg_count += 1;
                        writer.msg_rate_stat.push(msg.recv_time, 1.0);

                        writer.total_byte_count += event.payload_size;
                        writer
                            .bit_rate_stat
                            .push(msg.recv_time, (event.payload_size * 8) as f64);
                    }

                    // Update stat on associated topic stat
                    if let Some(topic_name) = writer.topic_name() {
                        let topic = state.topics.get_mut(topic_name).unwrap();

                        writer.total_msg_count += 1;
                        writer.msg_rate_stat.push(msg.recv_time, 1.0);

                        topic.total_byte_count += event.payload_size;
                        topic
                            .bit_rate_stat
                            .push(msg.recv_time, (event.payload_size * 8) as f64);
                    }
                }
            }
        }
    }

    fn handle_gap_event(&self, state: &mut State, _msg: &RtpsSubmsgEvent, _event: &GapEvent) {
        state.stat.packet_count += 1;

        // let GapEvent {
        //     writer_id,
        //     gap_start,
        //     ref gap_list,
        //     ..
        // } = *event;

        // let participant = state.participants.entry(writer_id.prefix).or_default();
        // let entity = participant.entities.entry(writer_id.entity_id).or_default();

        // let gaps: Vec<_> = chain!([gap_start], gap_list.iter())
        //     .map(|sn| sn.0)
        //     .collect();
        // println!("{}\t{gaps:?}", writer_id.display());

        // gap_list.iter();
        // todo!();
    }

    fn handle_heartbeat_event(
        &self,
        state: &mut State,
        _msg: &RtpsSubmsgEvent,
        event: &HeartbeatEvent,
    ) {
        state.stat.packet_count += 1;
        state.stat.heartbeat_submsg_count += 1;

        let participant = state
            .participants
            .entry(event.writer_guid.prefix)
            .or_default();
        let writer = participant
            .writers
            .entry(event.writer_guid.entity_id)
            .or_default();

        if let Some(heartbeat) = &mut writer.heartbeat {
            if heartbeat.count < event.count {
                if heartbeat.first_sn > event.first_sn.0 {
                    // TODO: warn
                }

                if heartbeat.last_sn > event.last_sn.0 {
                    // TODO: warn
                }

                *heartbeat = HeartbeatState {
                    first_sn: event.first_sn.0,
                    last_sn: event.last_sn.0,
                    count: event.count,
                    since: Instant::now(),
                };
            }
        } else {
            writer.heartbeat = Some(HeartbeatState {
                first_sn: event.first_sn.0,
                last_sn: event.first_sn.0,
                count: event.count,
                since: Instant::now(),
            });
        }
    }

    fn handle_acknack_event(&self, state: &mut State, msg: &RtpsSubmsgEvent, event: &AckNackEvent) {
        // Update statistics
        state.stat.packet_count += 1;
        state.stat.acknack_submsg_count += 1;

        // Update traffic statistics for associated reader
        let participant = state
            .participants
            .entry(event.reader_guid.prefix)
            .or_default();
        let reader = participant
            .readers
            .entry(event.reader_guid.entity_id)
            .or_default();

        // Update participant state.
        {
            participant.total_acknack_count += 1;
            participant.acknack_rate_stat.push(msg.recv_time, 1f64);
        }

        // Update reader state.
        {
            reader.total_acknack_count += 1;
            reader.acknack_rate_stat.push(msg.recv_time, 1f64);
        }

        // Save missing sequence numbers
        {
            if let Some(acknack) = &reader.acknack {
                if acknack.count >= event.count {
                    return;
                }
            }

            reader.acknack = Some(AckNackState {
                missing_sn: event.missing_sn.to_vec(),
                count: event.count,
                since: Instant::now(),
            });
        }

        // Update last sn
        reader.last_sn = Some(event.base_sn);

        // Update the stat on associated topic.
        if let Some(topic_name) = reader.topic_name() {
            let topic = state.topics.get_mut(topic_name).unwrap();

            topic.total_acknack_count += 1;
            topic.acknack_rate_stat.push(msg.recv_time, 1f64);
        }
    }

    fn handle_nackfrag_event(
        &self,
        state: &mut State,
        _msg: &RtpsSubmsgEvent,
        _event: &NackFragEvent,
    ) {
        state.stat.packet_count += 1;
        state.stat.ackfrag_submsg_count += 1;
    }

    fn handle_heartbeatfrag_event(
        &self,
        state: &mut State,
        _msg: &RtpsSubmsgEvent,
        _event: &HeartbeatFragEvent,
    ) {
        state.stat.packet_count += 1;
        state.stat.heartbeat_frag_submsg_count += 1;
    }

    fn handle_participant_info(&self, state: &mut State, info: &ParticipantInfo) {
        let ParticipantInfo {
            guid_prefix,
            ref unicast_locator_list,
            ref multicast_locator_list,
            ..
        } = *info;

        let participant = state.participants.entry(guid_prefix).or_default();
        participant.unicast_locator_list = Some(unicast_locator_list.clone());
        participant.multicast_locator_list = multicast_locator_list.clone();
    }

    fn toggle_logging(&mut self) -> Result<()> {
        if let Some(logger) = self.logger.take() {
            logger.close()?;
        } else {
            self.logger = Some(Logger::new()?);
        }

        Ok(())
    }
}
