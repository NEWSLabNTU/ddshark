use crate::{
    message::{
        DataEvent, DataFragEvent, DiscoveredReaderEvent, DiscoveredTopicEvent,
        DiscoveredWriterEvent, RtpsEvent, RtpsMessage,
    },
    opts::Opts,
    otlp,
    state::{EntityState, FragmentedMessage, State},
};
use std::{
    cmp,
    sync::{Arc, Mutex},
};
use tracing::{error, warn};

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
pub(crate) async fn run_updater(
    rx: flume::Receiver<RtpsMessage>,
    state: Arc<Mutex<State>>,
    opt: Opts,
) {
    // Enable OTLP if `otlp_enable` is true.
    let otlp_handle = match opt.otlp_enable {
        true => Some(otlp::TraceHandle::new(&opt)),
        false => None,
    };

    // Consume event messages from rx.
    loop {
        use flume::RecvError as E;

        let message = match rx.recv() {
            Ok(evt) => evt,
            Err(E::Disconnected) => break,
        };

        let Ok(mut state) = state.lock() else {
            error!("INTERNAL ERROR Mutex poision error");
            break;
        };

        let otlp_message = message.clone();
        let (_, event) = (message.headers, message.event);

        match event {
            RtpsEvent::Data(event) => {
                handle_data_event(&event, &mut state, otlp_handle.as_ref(), otlp_message);
            }
            RtpsEvent::DataFrag(event) => {
                handle_data_frag_event(&event, &mut state, otlp_handle.as_ref(), otlp_message);
            }
            RtpsEvent::DiscoveredTopic(event) => {
                handle_discovered_topic_event(
                    &event,
                    &mut state,
                    otlp_handle.as_ref(),
                    otlp_message,
                );
            }
            RtpsEvent::DiscoveredWriter(event) => {
                handle_discovered_writer_event(
                    &event,
                    &mut state,
                    otlp_handle.as_ref(),
                    otlp_message,
                );
            }
            RtpsEvent::DiscoveredReader(event) => {
                handle_discovered_reader_event(
                    &event,
                    &mut state,
                    otlp_handle.as_ref(),
                    otlp_message,
                );
            }
        }
    }
}

fn handle_data_event(
    event: &DataEvent,
    state: &mut State,
    otlp_handle: Option<&otlp::TraceHandle>,
    otlp_message: RtpsMessage,
) {
    let entity = state
        .entities
        .entry(event.writer_id)
        .or_insert_with(EntityState::default);
    entity.last_sn = cmp::max(entity.last_sn, Some(event.writer_sn));
    entity.message_count += 1;

    // let topic_name = entity.topic_name().unwrap_or("<none>").to_string();
    // if let Some(otlp) = otlp_handle.as_ref() {
    //     otlp.send_trace(&otlp_message, topic_name);
    // }
}

fn handle_data_frag_event(
    event: &DataFragEvent,
    state: &mut State,
    otlp_handle: Option<&otlp::TraceHandle>,
    otlp_message: RtpsMessage,
) {
    let entity = state
        .entities
        .entry(event.writer_id)
        .or_insert_with(EntityState::default);
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
        entity.last_sn = cmp::max(entity.last_sn, Some(event.writer_sn));
        entity.message_count += 1;
    }

    let topic_name = entity.topic_name().unwrap_or("<none>");
    if let Some(otlp) = otlp_handle.as_ref() {
        otlp.send_trace(&otlp_message, topic_name.to_string());
    }
}

fn handle_discovered_topic_event(
    event: &DiscoveredTopicEvent,
    state: &mut State,
    otlp_handle: Option<&otlp::TraceHandle>,
    otlp_message: RtpsMessage,
) {
    // noop
    // todo!();
}

fn handle_discovered_writer_event(
    event: &DiscoveredWriterEvent,
    state: &mut State,
    otlp_handle: Option<&otlp::TraceHandle>,
    otlp_message: RtpsMessage,
) {
    let entity = state
        .entities
        .entry(event.data.publication_topic_data.key)
        .or_insert_with(EntityState::default);
    // entity.last_sn = cmp::max(entity.last_sn, Some(event.writer_sn));
    entity.message_count += 1;

    let topic_name = entity.topic_name().unwrap_or("<none>").to_string();

    // Update discovered data in state.entities
    if entity.topic_info.is_some() {
        // TODO: show warning
    }

    // Insert the discovery data into state.entities with remote_writer_guid,
    // if it doesn't exist, then create a new entity corresponding to the remote_writer_guid.
    // if it exists, then update the entity with the discovery data.
    let entity = state
        .entities
        .entry(event.data.writer_proxy.remote_writer_guid)
        .or_insert_with(EntityState::default);
    entity.topic_info = Some(event.data.clone());

    if let Some(otlp) = otlp_handle.as_ref() {
        otlp.send_trace(&otlp_message, topic_name);
    }
}

fn handle_discovered_reader_event(
    event: &DiscoveredReaderEvent,
    state: &mut State,
    otlp_handle: Option<&otlp::TraceHandle>,
    otlp_message: RtpsMessage,
) {
    // noop
    // todo!();
}
