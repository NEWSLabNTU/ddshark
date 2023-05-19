use crate::{
    message::RtpsEvent,
    state::{EntityState, FragmentedMessage, State},
};
use std::sync::{Arc, Mutex};
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

        // TODO: update UI state
        match event {
            RtpsEvent::Data(event) => {
                // TODO: update statistics
            }
            RtpsEvent::DataFrag(event) => {
                let entity = state
                    .entities
                    .entry(event.writer_id)
                    .or_insert_with(EntityState::default);
                let msg_state = entity
                    .frag_messages
                    .entry(event.writer_sn)
                    .or_insert_with(FragmentedMessage::default);
                // TODO: insert fragment range into msg_state
            }
        }
    }
}
