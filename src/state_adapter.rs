use crate::{
    lockfree_state::{LockFreeState, StatisticsSnapshot},
    state::State,
};
use std::sync::{Arc, Mutex};

/// An adapter that provides a migration path from the old State to LockFreeState
/// This allows us to gradually transition the codebase while maintaining compatibility
pub struct StateAdapter {
    pub lockfree: Arc<LockFreeState>,
    // Keep a reference to the old state for gradual migration
    pub legacy: Arc<Mutex<State>>,
    // Flag to determine which state to use for reads
    pub use_lockfree: bool,
}

impl StateAdapter {
    pub fn new() -> Self {
        Self {
            lockfree: Arc::new(LockFreeState::new()),
            legacy: Arc::new(Mutex::new(State::default())),
            use_lockfree: false, // Start with legacy for compatibility
        }
    }

    /// Enable lock-free mode (call this when ready to switch)
    pub fn enable_lockfree(&mut self) {
        self.use_lockfree = true;
    }

    /// Get legacy state for components that haven't been migrated yet
    pub fn legacy_state(&self) -> Arc<Mutex<State>> {
        self.legacy.clone()
    }

    /// Get lock-free state for new components
    pub fn lockfree_state(&self) -> Arc<LockFreeState> {
        self.lockfree.clone()
    }

    /// Update statistics in both states during transition period
    pub fn increment_packet_count(&self) {
        // Update lock-free statistics
        self.lockfree
            .stat
            .packet_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Update legacy statistics if needed
        if !self.use_lockfree {
            if let Ok(mut state) = self.legacy.try_lock() {
                state.stat.packet_count += 1;
            }
        }
    }

    pub fn increment_data_submsg_count(&self) {
        self.lockfree
            .stat
            .data_submsg_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if !self.use_lockfree {
            if let Ok(mut state) = self.legacy.try_lock() {
                state.stat.data_submsg_count += 1;
            }
        }
    }

    pub fn increment_datafrag_submsg_count(&self) {
        self.lockfree
            .stat
            .datafrag_submsg_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if !self.use_lockfree {
            if let Ok(mut state) = self.legacy.try_lock() {
                state.stat.datafrag_submsg_count += 1;
            }
        }
    }

    pub fn increment_heartbeat_submsg_count(&self) {
        self.lockfree
            .stat
            .heartbeat_submsg_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if !self.use_lockfree {
            if let Ok(mut state) = self.legacy.try_lock() {
                state.stat.heartbeat_submsg_count += 1;
            }
        }
    }

    pub fn increment_acknack_submsg_count(&self) {
        self.lockfree
            .stat
            .acknack_submsg_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if !self.use_lockfree {
            if let Ok(mut state) = self.legacy.try_lock() {
                state.stat.acknack_submsg_count += 1;
            }
        }
    }

    pub fn increment_ackfrag_submsg_count(&self) {
        self.lockfree
            .stat
            .ackfrag_submsg_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if !self.use_lockfree {
            if let Ok(mut state) = self.legacy.try_lock() {
                state.stat.ackfrag_submsg_count += 1;
            }
        }
    }

    pub fn increment_heartbeat_frag_submsg_count(&self) {
        self.lockfree
            .stat
            .heartbeat_frag_submsg_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        if !self.use_lockfree {
            if let Ok(mut state) = self.legacy.try_lock() {
                state.stat.heartbeat_frag_submsg_count += 1;
            }
        }
    }

    pub fn increment_gap_submsg_count(&self) {
        self.lockfree
            .stat
            .gap_submsg_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Note: gap_submsg_count doesn't exist in the original Statistics struct
        // so we don't update the legacy state for this one
    }

    /// Add an abnormality to both states during transition
    pub fn add_abnormality(&self, message: String) {
        self.lockfree.add_abnormality(message.clone());

        if !self.use_lockfree {
            if let Ok(mut state) = self.legacy.try_lock() {
                state.abnormalities.push(crate::state::Abnormality {
                    when: chrono::Local::now(),
                    writer_guid: None,
                    reader_guid: None,
                    topic_name: None,
                    desc: message,
                });
            }
        }
    }

    /// Get statistics snapshot from the appropriate state
    pub fn get_statistics_snapshot(&self) -> StatisticsSnapshot {
        self.lockfree.stat.snapshot()
    }
}

impl Default for StateAdapter {
    fn default() -> Self {
        Self::new()
    }
}
