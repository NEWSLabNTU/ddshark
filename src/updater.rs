use crate::{
    message::RtpsEvent,
    state::{EntityState, FragmentedMessage, State},
};
use rust_lapper::Interval;
use std::{
    cmp,
    sync::{Arc, Mutex},
};
use tracing::error;

pub(crate) fn run_updater(rx: flume::Receiver<RtpsEvent>, state: Arc<Mutex<State>>) {
    // Consume event messages from rx.
    loop {
        use flume::RecvError as E;

        let event = match rx.recv() {
            Ok(evt) => evt,
            Err(E::Disconnected) => break,
        };

        let Ok(mut state) = state.lock() else {
            error!("INTERNAL ERROR Mutex poision error");
            break;
        };

        match event {
            RtpsEvent::Data(event) => {
                let entity = state
                    .entities
                    .entry(event.writer_id)
                    .or_insert_with(EntityState::default);
                entity.last_sn = cmp::max(entity.last_sn, Some(event.writer_sn));
                entity.message_count += 1;

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
            }
        }
    }
}
