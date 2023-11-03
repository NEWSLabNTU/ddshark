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
use chrono::Local;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::{debug, error, warn};

pub struct Updater {
    rx: flume::Receiver<UpdateEvent>,
    state: Arc<Mutex<State>>,
    otlp_handle: Option<otlp::TraceHandle>,
    logger: Logger,
}

impl Updater {
    pub(crate) fn new(
        rx: flume::Receiver<UpdateEvent>,
        state: Arc<Mutex<State>>,
        opt: &Opts,
    ) -> Self {
        // Enable OTLP if `otlp_enable` is true.
        let otlp_handle = match opt.otlp {
            true => Some(otlp::TraceHandle::new(opt)),
            false => None,
        };

        let logger = Logger::new().unwrap();

        Self {
            rx,
            state,
            otlp_handle,
            logger,
        }
    }

    pub(crate) fn run(mut self) {
        // Wait for the first message
        let (first_instant, first_recv_time) = {
            let Ok(message) = self.rx.recv() else { todo!() };

            let state = self.state.clone();
            let Ok(mut state) = state.lock() else {
                error!("INTERNAL ERROR Mutex poision error");
                todo!();
            };

            // Remember the difference b/w the current and receipt time.
            let now = Instant::now();
            let recv_time = match &message {
                UpdateEvent::RtpsMsg(_) => todo!(),
                UpdateEvent::RtpsSubmsg(msg) => msg.recv_time,
                UpdateEvent::ParticipantInfo(msg) => msg.recv_time,
                UpdateEvent::Tick(_) => unreachable!(),
            };

            self.handle_message(&mut state, &message);

            (now, recv_time)
        };

        let mut deadline = Instant::now() + TICK_INTERVAL;
        loop {
            use flume::RecvTimeoutError as E;

            let message = match self.rx.recv_deadline(deadline) {
                Ok(evt) => evt,
                Err(E::Disconnected) => break,
                Err(E::Timeout) => {
                    deadline += TICK_INTERVAL;

                    let now = Instant::now();
                    while now >= deadline {
                        deadline += TICK_INTERVAL;
                    }

                    let elapsed = now.duration_since(first_instant);
                    let recv_time = first_recv_time + chrono::Duration::from_std(elapsed).unwrap();
                    TickEvent { recv_time }.into()
                }
            };

            let state = self.state.clone();
            let Ok(mut state) = state.lock() else {
                error!("INTERNAL ERROR Mutex poision error");
                break;
            };

            self.handle_message(&mut state, &message);
        }
    }

    fn handle_message(&mut self, state: &mut State, message: &UpdateEvent) {
        match message {
            UpdateEvent::Tick(msg) => {
                self.handle_tick(state, msg);
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
        }
    }

    fn handle_tick(&mut self, state: &mut State, msg: &TickEvent) {
        let now = Instant::now();
        state.tick_since = now;

        let ts = msg.recv_time;

        for participant in state.participants.values_mut() {
            for writer in participant.writers.values_mut() {
                writer.bit_rate_stat.set_last_ts(ts);
                writer.msg_rate_stat.set_last_ts(ts);
            }

            for reader in participant.readers.values_mut() {
                reader.acknack_rate_stat.set_last_ts(ts);
            }
        }

        self.logger.save(state).unwrap();
    }

    fn handle_data_event(&self, state: &mut State, msg: &RtpsSubmsgEvent, event: &DataEvent) {
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

            writer.last_sn = Some(event.writer_sn);

            // Increase message count
            writer.total_msg_count += 1;
            let result = writer.msg_rate_stat.push(msg.recv_time, 1f64);
            if result.is_err() {
                todo!();
            }

            // Increase byte count
            writer.total_byte_count += event.payload_size;
            let result = writer
                .bit_rate_stat
                .push(msg.recv_time, (event.payload_size * 8) as f64);
            if result.is_err() {
                todo!();
            }
        }

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
                    writer.frag_messages.remove(&event.writer_sn).unwrap();
                    writer.last_sn = Some(event.writer_sn);

                    // Increase message count
                    writer.total_msg_count += 1;
                    let result = writer.msg_rate_stat.push(msg.recv_time, 1.0);
                    if result.is_err() {
                        todo!();
                    }

                    writer.total_byte_count += event.payload_size;
                    let result = writer
                        .bit_rate_stat
                        .push(msg.recv_time, (event.payload_size * 8) as f64);
                    if result.is_err() {
                        todo!();
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

    fn handle_acknack_event(
        &self,
        state: &mut State,
        _msg: &RtpsSubmsgEvent,
        event: &AckNackEvent,
    ) {
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

        reader.total_acknack_count += 1;
        // reader.acc_acknack_count += 1;

        // Save missing sequence numbers
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

        // Update last sn
        reader.last_sn = Some(event.base_sn);
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
}
