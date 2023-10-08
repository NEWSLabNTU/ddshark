use crate::{
    message::{
        AckNackEvent, DataEvent, DataFragEvent, DataPayload, GapEvent, HeartbeatEvent,
        HeartbeatFragEvent, NackFragEvent, RtpsEvent, RtpsMessage,
    },
    opts::Opts,
    otlp,
    state::{
        Abnormality, EntityReaderContext, EntityWriterContext, FragmentedMessage, HeartbeatState,
        State, TopicState,
    },
};
use chrono::Local;
use itertools::chain;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Instant,
};
use tracing::{debug, error, warn};

pub struct Updater {
    rx: flume::Receiver<RtpsMessage>,
    state: Arc<Mutex<State>>,
    otlp_handle: Option<otlp::TraceHandle>,
}

impl Updater {
    pub(crate) fn new(
        rx: flume::Receiver<RtpsMessage>,
        state: Arc<Mutex<State>>,
        opt: &Opts,
    ) -> Self {
        // Enable OTLP if `otlp_enable` is true.
        let otlp_handle = match opt.otlp_enable {
            true => Some(otlp::TraceHandle::new(opt)),
            false => None,
        };

        Self {
            rx,
            state,
            otlp_handle,
        }
    }

    pub(crate) fn run(self) {
        // Consume event messages from rx.
        loop {
            use flume::RecvError as E;

            let message = match self.rx.recv() {
                Ok(evt) => evt,
                Err(E::Disconnected) => break,
            };

            let Ok(mut state) = self.state.lock() else {
                error!("INTERNAL ERROR Mutex poision error");
                break;
            };

            let event = &message.event;

            match event {
                RtpsEvent::Data(event) => {
                    self.handle_data_event(&mut state, &message, event);
                }
                RtpsEvent::DataFrag(event) => {
                    self.handle_data_frag_event(&mut state, &message, event);
                }
                RtpsEvent::Gap(event) => {
                    self.handle_gap_event(&mut state, &message, event);
                }
                RtpsEvent::Heartbeat(event) => {
                    self.handle_heartbeat_event(&mut state, &message, event);
                }
                RtpsEvent::AckNack(event) => {
                    self.handle_ack_nack_event(&mut state, &message, event);
                }
                RtpsEvent::NackFrag(event) => {
                    self.handle_nack_frag_event(&mut state, &message, event);
                }
                RtpsEvent::HeartbeatFrag(event) => {
                    self.handle_heartbeat_frag_event(&mut state, &message, event);
                }
            }
        }
    }

    fn handle_data_event(&self, state: &mut State, _message: &RtpsMessage, event: &DataEvent) {
        {
            let participant = state
                .participants
                .entry(event.writer_id.prefix)
                .or_default();
            let entity = participant
                .entities
                .entry(event.writer_id.entity_id)
                .or_default();

            entity.last_sn = Some(event.writer_sn);
            entity.message_count += 1;
            entity.recv_count += event.payload_size;
        }

        // println!(
        //     "{}\t{}\t{:.2}bps",
        //     event.writer_id.display(),
        //     entity.recv_count,
        //     entity.recv_bitrate()
        // );

        if let Some(payload) = &event.payload {
            match payload {
                DataPayload::DiscoveredTopic(data) => {
                    debug!("DiscoveredTopic not yet implemented");
                    // let topic_name = data.topic_data.name.clone();
                    // TODO
                }
                DataPayload::DiscoveredWriter(data) => {
                    let remote_writer_guid = data.writer_proxy.remote_writer_guid;
                    // TODO: Find the correct writer
                    assert_eq!(event.writer_id.prefix, remote_writer_guid.prefix);

                    let participant = state
                        .participants
                        .entry(remote_writer_guid.prefix)
                        .or_default();
                    let entity = participant
                        .entities
                        .entry(remote_writer_guid.entity_id)
                        .or_default();

                    // Update discovered data in state.entities
                    {
                        if !entity.context.is_unknown() {
                            // TODO: show warning
                        }

                        entity.context = EntityWriterContext {
                            data: (**data).clone(),
                        }
                        .into();
                    }

                    // Update stats on associated topic
                    {
                        let topic_name = data.publication_topic_data.topic_name.clone();
                        let topic_state =
                            state
                                .topics
                                .entry(topic_name.clone())
                                .or_insert_with(|| TopicState {
                                    readers: HashSet::new(),
                                    writers: HashSet::new(),
                                });
                        topic_state.writers.insert(remote_writer_guid);
                        entity.topic_name = Some(topic_name);
                    }
                }
                DataPayload::DiscoveredReader(data) => {
                    let remote_reader_guid = data.reader_proxy.remote_reader_guid;
                    // TODO: Find the correct writer
                    assert_eq!(event.reader_id.prefix, remote_reader_guid.prefix);

                    let participant = state
                        .participants
                        .entry(remote_reader_guid.prefix)
                        .or_default();

                    let entity = participant
                        .entities
                        .entry(remote_reader_guid.entity_id)
                        .or_default();

                    // Update discovered data in state.entities
                    {
                        if !entity.context.is_unknown() {
                            // TODO: show warning
                        }

                        entity.context = EntityReaderContext {
                            data: (**data).clone(),
                        }
                        .into();
                    }

                    // Update stats on associated topic
                    {
                        let topic_name = data.subscription_topic_data.topic_name().clone();
                        let topic_state =
                            state
                                .topics
                                .entry(topic_name.clone())
                                .or_insert_with(|| TopicState {
                                    readers: HashSet::new(),
                                    writers: HashSet::new(),
                                });
                        topic_state.readers.insert(remote_reader_guid);
                        entity.topic_name = Some(topic_name);
                    }
                }
                DataPayload::DiscoveredParticipant(data) => {
                    debug!("DiscoveredParticipant not yet implemented");
                    // TODO
                }
            }
        }
    }

    fn handle_data_frag_event(
        &self,
        state: &mut State,
        _message: &RtpsMessage,
        event: &DataFragEvent,
    ) {
        let DataFragEvent {
            fragment_starting_num,
            fragments_in_submessage,
            writer_id,
            writer_sn,
            // fragment_size,
            // data_size,
            // payload_size,
            ..
        } = *event;

        let participant = state.participants.entry(writer_id.prefix).or_default();
        let entity = participant.entities.entry(writer_id.entity_id).or_default();

        // Increase recv count
        entity.recv_count += event.payload_size;
        // println!(
        //     "{}\t{}\t{:.2}bps",
        //     event.writer_id.display(),
        //     entity.recv_count,
        //     entity.recv_bitrate()
        // );

        // let topic_name = entity.topic_name().map(|t| t.to_string());
        let frag_msg = entity.frag_messages.entry(writer_sn).or_insert_with(|| {
            FragmentedMessage::new(event.data_size as usize, event.fragment_size as usize)
        });

        if event.data_size as usize != frag_msg.data_size {
            state.abnormalities.push(Abnormality {
                when: Local::now(),
                writer_id: Some(writer_id),
                reader_id: None,
                topic_name: entity.topic_name.clone(),
                desc: format!(
                    "data_size changes from {} to {} in DataFrag submsg",
                    frag_msg.data_size, event.data_size
                ),
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
                        writer_id: Some(writer_id),
                        reader_id: None,
                        topic_name: entity.topic_name.clone(),
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
                    entity.frag_messages.remove(&event.writer_sn).unwrap();
                    entity.last_sn = Some(event.writer_sn);
                    entity.message_count += 1;
                }
            }
        }
    }

    fn handle_gap_event(&self, state: &mut State, _message: &RtpsMessage, event: &GapEvent) {
        let GapEvent {
            writer_id,
            gap_start,
            ref gap_list,
            ..
        } = *event;

        let participant = state.participants.entry(writer_id.prefix).or_default();
        let entity = participant.entities.entry(writer_id.entity_id).or_default();

        let gaps: Vec<_> = chain!([gap_start], gap_list.iter())
            .map(|sn| sn.0)
            .collect();
        // println!("{}\t{gaps:?}", writer_id.display());

        // gap_list.iter();
        // todo!();
    }

    fn handle_heartbeat_event(
        &self,
        state: &mut State,
        _message: &RtpsMessage,
        event: &HeartbeatEvent,
    ) {
        let participant = state
            .participants
            .entry(event.writer_id.prefix)
            .or_default();
        let entity = participant
            .entities
            .entry(event.writer_id.entity_id)
            .or_default();

        if let Some(heartbeat) = &mut entity.heartbeat {
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
            entity.heartbeat = Some(HeartbeatState {
                first_sn: event.first_sn.0,
                last_sn: event.first_sn.0,
                count: event.count,
                since: Instant::now(),
            });
        }
    }

    fn handle_ack_nack_event(
        &self,
        state: &mut State,
        _message: &RtpsMessage,
        event: &AckNackEvent,
    ) {
    }

    fn handle_nack_frag_event(
        &self,
        state: &mut State,
        _message: &RtpsMessage,
        event: &NackFragEvent,
    ) {
    }

    fn handle_heartbeat_frag_event(
        &self,
        state: &mut State,
        _message: &RtpsMessage,
        event: &HeartbeatFragEvent,
    ) {
    }
}
