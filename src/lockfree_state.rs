use crate::{config::TICK_INTERVAL, logger::Logger, utils::TimedStat};
use arc_swap::ArcSwap;
use crossbeam_queue::SegQueue;
use dashmap::DashMap;
use rustdds::{
    structure::{
        guid::{EntityId, GuidPrefix},
        locator::Locator,
    },
    GUID,
};
use std::{
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};

/// Lock-free statistics using atomic counters
#[derive(Default)]
pub struct LockFreeStatistics {
    pub packet_count: AtomicU64,
    pub data_submsg_count: AtomicU64,
    pub datafrag_submsg_count: AtomicU64,
    pub heartbeat_submsg_count: AtomicU64,
    pub acknack_submsg_count: AtomicU64,
    pub ackfrag_submsg_count: AtomicU64,
    pub heartbeat_frag_submsg_count: AtomicU64,
    pub gap_submsg_count: AtomicU64,
}

impl LockFreeStatistics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a snapshot of current statistics
    pub fn snapshot(&self) -> StatisticsSnapshot {
        StatisticsSnapshot {
            packet_count: self.packet_count.load(Ordering::Relaxed),
            data_submsg_count: self.data_submsg_count.load(Ordering::Relaxed),
            datafrag_submsg_count: self.datafrag_submsg_count.load(Ordering::Relaxed),
            heartbeat_submsg_count: self.heartbeat_submsg_count.load(Ordering::Relaxed),
            acknack_submsg_count: self.acknack_submsg_count.load(Ordering::Relaxed),
            ackfrag_submsg_count: self.ackfrag_submsg_count.load(Ordering::Relaxed),
            heartbeat_frag_submsg_count: self.heartbeat_frag_submsg_count.load(Ordering::Relaxed),
            gap_submsg_count: self.gap_submsg_count.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StatisticsSnapshot {
    pub packet_count: u64,
    pub data_submsg_count: u64,
    pub datafrag_submsg_count: u64,
    pub heartbeat_submsg_count: u64,
    pub acknack_submsg_count: u64,
    pub ackfrag_submsg_count: u64,
    pub heartbeat_frag_submsg_count: u64,
    pub gap_submsg_count: u64,
}

/// Lock-free abnormality queue
pub struct AbnormalityQueue {
    queue: SegQueue<Abnormality>,
    max_size: usize,
    current_size: AtomicUsize,
}

impl AbnormalityQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: SegQueue::new(),
            max_size,
            current_size: AtomicUsize::new(0),
        }
    }

    pub fn push(&self, abnormality: Abnormality) {
        // Remove old items if we exceed max_size
        while self.current_size.load(Ordering::Relaxed) >= self.max_size {
            if self.queue.pop().is_some() {
                self.current_size.fetch_sub(1, Ordering::Relaxed);
            } else {
                break;
            }
        }

        self.queue.push(abnormality);
        self.current_size.fetch_add(1, Ordering::Relaxed);
    }

    pub fn collect_all(&self) -> Vec<Abnormality> {
        let mut abnormalities = Vec::new();
        while let Some(abnormality) = self.queue.pop() {
            abnormalities.push(abnormality);
            self.current_size.fetch_sub(1, Ordering::Relaxed);
        }
        abnormalities
    }

    pub fn len(&self) -> usize {
        self.current_size.load(Ordering::Relaxed)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone)]
pub struct Abnormality {
    pub when: Instant,
    pub msg: String,
}

/// Lock-free participant state using atomic counters
#[derive(Debug)]
pub struct LockFreeParticipantState {
    pub writers: DashMap<EntityId, WriterState>,
    pub readers: DashMap<EntityId, ReaderState>,
    pub unicast_locator_list: ArcSwap<Option<Vec<Locator>>>,
    pub multicast_locator_list: ArcSwap<Option<Vec<Locator>>>,

    // Atomic counters instead of regular usize
    pub total_msg_count: AtomicUsize,
    pub total_byte_count: AtomicUsize,
    pub total_acknack_count: AtomicUsize,

    // Note: TimedStat still needs interior mutability, but we can use parking_lot::Mutex
    pub msg_rate_stat: parking_lot::Mutex<TimedStat>,
    pub bit_rate_stat: parking_lot::Mutex<TimedStat>,
    pub acknack_rate_stat: parking_lot::Mutex<TimedStat>,
}

impl Default for LockFreeParticipantState {
    fn default() -> Self {
        let window = chrono::Duration::from_std(TICK_INTERVAL).unwrap();

        Self {
            writers: DashMap::new(),
            readers: DashMap::new(),
            unicast_locator_list: ArcSwap::new(Arc::new(None)),
            multicast_locator_list: ArcSwap::new(Arc::new(None)),
            total_msg_count: AtomicUsize::new(0),
            total_byte_count: AtomicUsize::new(0),
            total_acknack_count: AtomicUsize::new(0),
            msg_rate_stat: parking_lot::Mutex::new(TimedStat::new(window)),
            bit_rate_stat: parking_lot::Mutex::new(TimedStat::new(window)),
            acknack_rate_stat: parking_lot::Mutex::new(TimedStat::new(window)),
        }
    }
}

/// Lock-free topic state
#[derive(Debug)]
pub struct LockFreeTopicState {
    pub writers: DashMap<GUID, String>, // GUID -> type_name mapping
    pub readers: DashMap<GUID, String>, // GUID -> type_name mapping

    // Atomic counters
    pub total_msg_count: AtomicUsize,
    pub total_byte_count: AtomicUsize,
    pub total_acknack_count: AtomicUsize,

    // Thread-safe stats
    pub msg_rate_stat: parking_lot::Mutex<TimedStat>,
    pub bit_rate_stat: parking_lot::Mutex<TimedStat>,
    pub acknack_rate_stat: parking_lot::Mutex<TimedStat>,
}

impl Default for LockFreeTopicState {
    fn default() -> Self {
        let window = chrono::Duration::from_std(TICK_INTERVAL).unwrap();

        Self {
            writers: DashMap::new(),
            readers: DashMap::new(),
            total_msg_count: AtomicUsize::new(0),
            total_byte_count: AtomicUsize::new(0),
            total_acknack_count: AtomicUsize::new(0),
            msg_rate_stat: parking_lot::Mutex::new(TimedStat::new(window)),
            bit_rate_stat: parking_lot::Mutex::new(TimedStat::new(window)),
            acknack_rate_stat: parking_lot::Mutex::new(TimedStat::new(window)),
        }
    }
}

/// The lock-free global state
pub struct LockFreeState {
    pub tick_since: ArcSwap<Instant>,
    pub participants: DashMap<GuidPrefix, LockFreeParticipantState>,
    pub topics: DashMap<String, LockFreeTopicState>,
    pub abnormalities: AbnormalityQueue,
    pub stat: LockFreeStatistics,
    pub logger: ArcSwap<Option<Logger>>,
}

impl Default for LockFreeState {
    fn default() -> Self {
        Self {
            tick_since: ArcSwap::new(Arc::new(Instant::now())),
            participants: DashMap::new(),
            topics: DashMap::new(),
            abnormalities: AbnormalityQueue::new(1000), // Max 1000 abnormalities
            stat: LockFreeStatistics::new(),
            logger: ArcSwap::new(Arc::new(None)),
        }
    }
}

impl LockFreeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a participant, creating it if it doesn't exist
    pub fn get_or_create_participant(
        &self,
        guid_prefix: GuidPrefix,
    ) -> dashmap::mapref::one::Ref<GuidPrefix, LockFreeParticipantState> {
        self.participants.entry(guid_prefix).or_default();
        self.participants.get(&guid_prefix).unwrap()
    }

    /// Get a topic, creating it if it doesn't exist
    pub fn get_or_create_topic(
        &self,
        topic_name: String,
    ) -> dashmap::mapref::one::Ref<String, LockFreeTopicState> {
        self.topics.entry(topic_name.clone()).or_default();
        self.topics.get(&topic_name).unwrap()
    }

    /// Add an abnormality
    pub fn add_abnormality(&self, message: String) {
        self.abnormalities.push(Abnormality {
            when: Instant::now(),
            msg: message,
        });
    }

    /// Update tick time
    pub fn update_tick_time(&self, new_time: Instant) {
        self.tick_since.store(Arc::new(new_time));
    }

    /// Set logger
    pub fn set_logger(&self, logger: Option<Logger>) {
        self.logger.store(Arc::new(logger));
    }

    /// Get a snapshot of current abnormalities (for UI display)
    pub fn get_abnormalities_snapshot(&self) -> Vec<Abnormality> {
        // For now, we'll collect all abnormalities which clears them
        // In a production system, you'd want a better strategy
        self.abnormalities.collect_all()
    }
}

// Re-export needed types from the original state module
pub use crate::state::{ReaderState, WriterState};
