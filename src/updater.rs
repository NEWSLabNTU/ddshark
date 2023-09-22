use crate::{
    message::{DataEvent, DataFragEvent, DataPayload, RtpsEvent, RtpsMessage},
    opts::Opts,
    otlp,
    state::{EntityReaderContext, EntityWriterContext, FragmentedMessage, State},
};
use std::sync::{Arc, Mutex};
use tracing::{error, warn};

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
            }
        }
    }

    fn handle_data_event(&self, state: &mut State, message: &RtpsMessage, event: &DataEvent) {
        let participant = state
            .participants
            .entry(event.writer_id.prefix)
            .or_default();
        let entity = participant
            .entities
            .entry(event.writer_id.entity_id)
            .or_default();

        entity.last_sn = Some(event.writer_sn.clone());
        entity.message_count += 1;

        if let Some(payload) = &event.payload {
            match payload {
                DataPayload::DiscoveredTopic(data) => {
                    error!("DiscoveredTopic not yet implemented");
                    // TODO
                }
                DataPayload::DiscoveredWriter(data) => {
                    let remote_writer_guid = data.writer_proxy.remote_writer_guid;

                    let participant = state
                        .participants
                        .entry(remote_writer_guid.prefix)
                        .or_default();

                    let entity = participant
                        .entities
                        .entry(remote_writer_guid.entity_id)
                        .or_default();

                    // Update discovered data in state.entities
                    if !entity.context.is_unknown() {
                        // TODO: show warning
                    }

                    entity.context = EntityWriterContext {
                        data: (**data).clone(),
                    }
                    .into();
                }
                DataPayload::DiscoveredReader(data) => {
                    let remote_reader_guid = data.reader_proxy.remote_reader_guid;

                    let participant = state
                        .participants
                        .entry(remote_reader_guid.prefix)
                        .or_default();

                    let entity = participant
                        .entities
                        .entry(remote_reader_guid.entity_id)
                        .or_default();

                    // Update discovered data in state.entities
                    if !entity.context.is_unknown() {
                        // TODO: show warning
                    }

                    entity.context = EntityReaderContext {
                        data: (**data).clone(),
                    }
                    .into();
                }
                DataPayload::DiscoveredParticipant(data) => {
                    error!("DiscoveredParticipant not yet implemented");
                    // TODO
                }
            }
        }
    }

    fn handle_data_frag_event(
        &self,
        state: &mut State,
        message: &RtpsMessage,
        event: &DataFragEvent,
    ) {
        let participant = state
            .participants
            .entry(event.writer_id.prefix)
            .or_default();
        let entity = participant
            .entities
            .entry(event.writer_id.entity_id)
            .or_default();

        let msg_state = entity
            .frag_messages
            .entry(event.writer_sn)
            .or_insert_with(|| {
                FragmentedMessage::new(event.data_size as usize, event.fragment_size as usize)
            });

        if event.data_size as usize != msg_state.data_size {
            error!("event.data_size changes! Ignore this message.");
            return;
        }

        // Compute the submessage payload range
        let interval = {
            let DataFragEvent {
                fragment_starting_num,
                fragments_in_submessage,
                ..
            } = *event;

            let start = fragment_starting_num as usize - 1;
            let end = start + fragments_in_submessage as usize;
            start..end
        };

        let free_intervals = &mut msg_state.free_intervals;
        if free_intervals.insert(interval).is_err() {
            warn!("Overlapping fragments detected. Ignore this message");
            return;
        }

        msg_state.recvd_fragments += event.fragments_in_submessage as usize;

        if free_intervals.is_full() {
            entity.frag_messages.remove(&event.writer_sn);
            entity.last_sn = Some(event.writer_sn);
            entity.message_count += 1;
        }
    }
}
