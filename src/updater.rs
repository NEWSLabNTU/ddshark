use crate::{
    message::{RtpsEvent, RtpsMessage},
    opts::Opts,
    otlp,
    state::{EntityState, FragmentedMessage, State},
};
use rust_lapper::Interval;
use std::{
    cmp,
    sync::{Arc, Mutex},
};
use tracing::error;

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
                let entity = state
                    .entities
                    .entry(event.writer_id)
                    .or_insert_with(EntityState::default);
                entity.last_sn = cmp::max(entity.last_sn, Some(event.writer_sn));
                entity.message_count += 1;

                let topic_name = match &entity.topic_info {
                    Some(info) => info.publication_topic_data.topic_name.clone(),
                    None => "<none>".to_string(),
                };

                // Update discovered data in state.entities
                if let Some(discovery_data) = event.discovery_data {
                    if entity.topic_info.is_some() {
                        // TODO: show warning
                    }

                    // Insert the discovery data into state.entities with remote_writer_guid,
                    // if it doesn't exist, then create a new entity corresponding to the remote_writer_guid.
                    // if it exists, then update the entity with the discovery data.
                    let entity = state
                        .entities
                        .entry(discovery_data.writer_proxy.remote_writer_guid)
                        .or_insert_with(EntityState::default);
                    entity.topic_info = Some(discovery_data);
                }

                if let Some(otlp) = otlp_handle.as_ref() {
                    otlp.send_trace(&otlp_message, topic_name.clone());
                }
            }
            RtpsEvent::DataFrag(event) => {
                let entity = state
                    .entities
                    .entry(event.writer_id)
                    .or_insert_with(EntityState::default);
                let msg_state = entity
                    .frag_messages
                    .entry(event.writer_sn)
                    .or_insert_with(|| FragmentedMessage::new(event.data_size as usize));

                if msg_state.data_size != event.data_size as usize {
                    todo!("Handle inconsistent data_size");
                }

                // Compute the submessage payload range
                let intervals = &mut msg_state.intervals;
                let interval = {
                    let start =
                        (event.fragment_starting_num - 1) as usize * event.fragment_size as usize;
                    let stop = start
                        + event.fragments_in_submessage as usize * event.fragment_size as usize;
                    Interval {
                        start,
                        stop,
                        val: (),
                    }
                };
                intervals.insert(interval);
                intervals.merge_overlaps();

                // Check if the message is finished.
                let is_finished = matches!(&intervals.intervals[..],
                                           [int]
                                           if int.start == 0 && int.stop == msg_state.data_size);

                if is_finished {
                    entity.frag_messages.remove(&event.writer_sn);
                    entity.last_sn = cmp::max(entity.last_sn, Some(event.writer_sn));
                    entity.message_count += 1;
                }

                let topic_name = match &entity.topic_info {
                    Some(info) => info.publication_topic_data.topic_name.clone(),
                    None => "<none>".to_string(),
                };

                if let Some(otlp) = otlp_handle.as_ref() {
                    otlp.send_trace(&otlp_message, topic_name.clone());
                }
            }
        }
    }
}
