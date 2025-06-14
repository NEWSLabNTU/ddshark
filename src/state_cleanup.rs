use crate::state::{State, FragmentedMessage};
use std::time::{Duration, Instant};
use tracing::debug;

const FRAGMENT_TIMEOUT: Duration = Duration::from_secs(30);
const ABNORMALITY_RETENTION: Duration = Duration::from_secs(300); // 5 minutes
const MAX_ABNORMALITIES: usize = 1000;

pub struct StateCleanup {
    last_cleanup: Instant,
    cleanup_interval: Duration,
}

impl StateCleanup {
    pub fn new(cleanup_interval: Duration) -> Self {
        Self {
            last_cleanup: Instant::now(),
            cleanup_interval,
        }
    }

    /// Returns true if cleanup should run
    pub fn should_cleanup(&self) -> bool {
        self.last_cleanup.elapsed() > self.cleanup_interval
    }

    /// Perform cleanup on the state
    pub fn cleanup(&mut self, state: &mut State) {
        let now = Instant::now();
        
        // Clean up old fragmented messages
        let mut cleaned_fragments = 0;
        for participant in state.participants.values_mut() {
            for writer in participant.writers.values_mut() {
                writer.fragmented_messages.retain(|_, frag_msg| {
                    let should_keep = now.duration_since(frag_msg.last_update) < FRAGMENT_TIMEOUT;
                    if !should_keep {
                        cleaned_fragments += 1;
                    }
                    should_keep
                });
            }
        }

        if cleaned_fragments > 0 {
            debug!("Cleaned up {} timed-out fragmented messages", cleaned_fragments);
        }

        // Clean up old abnormalities
        let cutoff_time = now - ABNORMALITY_RETENTION;
        let original_len = state.abnormalities.len();
        
        // Keep recent abnormalities or limit to MAX_ABNORMALITIES
        if state.abnormalities.len() > MAX_ABNORMALITIES {
            // Keep only the most recent MAX_ABNORMALITIES
            state.abnormalities.sort_by(|a, b| b.when.cmp(&a.when));
            state.abnormalities.truncate(MAX_ABNORMALITIES);
        } else {
            // Remove old abnormalities
            state.abnormalities.retain(|abnormality| {
                abnormality.when > cutoff_time
            });
        }

        let removed = original_len - state.abnormalities.len();
        if removed > 0 {
            debug!("Removed {} old abnormalities", removed);
        }

        // Clean up empty participants
        state.participants.retain(|_, participant| {
            !participant.writers.is_empty() || !participant.readers.is_empty()
        });

        // Clean up empty topics
        state.topics.retain(|_, topic| {
            !topic.writers.is_empty() || !topic.readers.is_empty()
        });

        self.last_cleanup = now;
    }
}

impl FragmentedMessage {
    fn last_update(&self) -> Instant {
        // This would need to be added to FragmentedMessage struct
        // For now, using a placeholder
        Instant::now()
    }
}