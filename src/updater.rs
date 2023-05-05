use crate::{
    dds::DiscoveryEvent,
    state::{Entry, State},
};
use std::sync::{Arc, Mutex};
use tracing::{error, warn};

pub(crate) fn run_updater(rx: flume::Receiver<DiscoveryEvent>, state: Arc<Mutex<State>>) {
    // Consume event messages from rx.
    loop {
        use flume::RecvError as E;

        let evt = match rx.recv() {
            Ok(evt) => evt,
            Err(E::Disconnected) => break,
        };

        let Ok(mut state) = state.lock() else {
            error!("INTERNAL ERROR Mutex poision error");
            break;
        };

        // TODO: update UI state

        use DiscoveryEvent as D;
        match evt {
            D::DiscoveredPublication { entity } => {
                let entry = state
                    .pub_keys
                    .entry(entity.key.clone())
                    .or_insert_with(|| Entry::new(entity));

                entry.acc_msgs += 1;
            }
            D::UndiscoveredPublication { key } => {
                let removed = state.pub_keys.remove(&key);
                if removed.is_none() {
                    warn!("The key '{key}' is undiscovered but was not detected");
                }
            }
            D::DiscoveredSubscription { entity } => {
                let entry = state
                    .sub_keys
                    .entry(entity.key.clone())
                    .or_insert_with(|| Entry::new(entity));

                entry.acc_msgs += 1;
            }
            D::UndiscoveredSubscription { key } => {
                let removed = state.sub_keys.remove(&key);
                if removed.is_none() {
                    warn!("The key '{key}' is undiscovered but was not detected");
                }
            }
        };
    }
}
