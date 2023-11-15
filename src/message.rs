//! Messages exchanged within the program.

use etherparse::{Ethernet2Header, Ipv4Header, UdpHeader, VlanHeader};
use rustdds::{
    dds::DiscoveredTopicData,
    discovery::data_types::{
        spdp_participant_data::SpdpDiscoveredParticipantData,
        topic_data::{DiscoveredReaderData, DiscoveredWriterData},
    },
    structure::{
        guid::GuidPrefix,
        locator::Locator,
        sequence_number::{FragmentNumber, SequenceNumberSet},
    },
    SequenceNumber, Timestamp, GUID,
};
use std::time::Instant;

/// The message that is sent to the updater.
#[derive(Debug, Clone)]
pub enum UpdateEvent {
    RtpsMsg(RtpsMsgEvent),
    RtpsSubmsg(RtpsSubmsgEvent),
    ParticipantInfo(ParticipantInfo),
    Tick(TickEvent),
    ToggleLogging,
}

impl From<TickEvent> for UpdateEvent {
    fn from(v: TickEvent) -> Self {
        Self::Tick(v)
    }
}

impl From<ParticipantInfo> for UpdateEvent {
    fn from(v: ParticipantInfo) -> Self {
        Self::ParticipantInfo(v)
    }
}

impl From<RtpsSubmsgEvent> for UpdateEvent {
    fn from(v: RtpsSubmsgEvent) -> Self {
        Self::RtpsSubmsg(v)
    }
}

/// The message bursts every a fixed period of time.
#[derive(Debug, Clone)]
pub struct TickEvent {
    pub when: Instant,
    pub recv_time: chrono::Duration,
}

/// The event records a receipt of a RTPS submessage.
#[derive(Debug, Clone)]
pub struct RtpsSubmsgEvent {
    pub recv_time: chrono::Duration,
    pub rtps_time: Timestamp,
    pub kind: RtpsSubmsgEventKind,
}

/// Variants of RTPS submessages.
#[derive(Debug, Clone)]
pub enum RtpsSubmsgEventKind {
    Data(Box<DataEvent>),
    DataFrag(Box<DataFragEvent>),
    Gap(Box<GapEvent>),
    AckNack(AckNackEvent),
    NackFrag(NackFragEvent),
    Heartbeat(HeartbeatEvent),
    HeartbeatFrag(HeartbeatFragEvent),
}

impl From<NackFragEvent> for RtpsSubmsgEventKind {
    fn from(v: NackFragEvent) -> Self {
        Self::NackFrag(v)
    }
}

impl From<DataFragEvent> for RtpsSubmsgEventKind {
    fn from(v: DataFragEvent) -> Self {
        Self::DataFrag(Box::new(v))
    }
}

impl From<DataEvent> for RtpsSubmsgEventKind {
    fn from(v: DataEvent) -> Self {
        Self::Data(Box::new(v))
    }
}

impl From<GapEvent> for RtpsSubmsgEventKind {
    fn from(v: GapEvent) -> Self {
        Self::Gap(Box::new(v))
    }
}

impl From<HeartbeatEvent> for RtpsSubmsgEventKind {
    fn from(v: HeartbeatEvent) -> Self {
        Self::Heartbeat(v)
    }
}

impl From<HeartbeatFragEvent> for RtpsSubmsgEventKind {
    fn from(v: HeartbeatFragEvent) -> Self {
        Self::HeartbeatFrag(v)
    }
}

impl From<AckNackEvent> for RtpsSubmsgEventKind {
    fn from(v: AckNackEvent) -> Self {
        Self::AckNack(v)
    }
}

/// The event records the receipt of a RTPS packet.
#[derive(Debug, Clone)]
pub struct RtpsMsgEvent {
    pub headers: RtpsPacketHeaders,
}

/// The dissected headers from a RTPS packet.
#[derive(Debug, Clone)]
pub struct RtpsPacketHeaders {
    pub pcap_header: pcap::PacketHeader,
    pub link: Option<Ethernet2Header>,
    pub vlan: Option<VlanHeader>,
    pub ipv4: Ipv4Header,
    pub udp: UdpHeader,
    pub ts: chrono::Duration,
}

/// The typed data payload decoded from a RTPS submessage.
#[derive(Debug, Clone)]
pub enum DataPayload {
    Topic(Box<DiscoveredTopicData>),
    Writer(Box<DiscoveredWriterData>),
    Reader(Box<DiscoveredReaderData>),
    Participant(Box<SpdpDiscoveredParticipantData>),
}

impl From<SpdpDiscoveredParticipantData> for DataPayload {
    fn from(v: SpdpDiscoveredParticipantData) -> Self {
        Self::Participant(Box::new(v))
    }
}

impl From<DiscoveredReaderData> for DataPayload {
    fn from(v: DiscoveredReaderData) -> Self {
        Self::Reader(Box::new(v))
    }
}

impl From<DiscoveredWriterData> for DataPayload {
    fn from(v: DiscoveredWriterData) -> Self {
        Self::Writer(Box::new(v))
    }
}

impl From<DiscoveredTopicData> for DataPayload {
    fn from(v: DiscoveredTopicData) -> Self {
        Self::Topic(Box::new(v))
    }
}

/// The events records the receipt of a topic discovery message.
#[derive(Debug, Clone)]
pub struct DiscoveredTopicEvent {
    pub data: DiscoveredTopicData,
}

/// The events records the receipt of a writer discovery message.
#[derive(Debug, Clone)]
pub struct DiscoveredWriterEvent {
    pub data: DiscoveredWriterData,
}

/// The events records the receipt of a reader discovery message.
#[derive(Debug, Clone)]
pub struct DiscoveredReaderEvent {
    pub data: DiscoveredReaderData,
}

/// The events records the receipt of a HEARTBEAT submessage.
#[derive(Debug, Clone)]
pub struct HeartbeatEvent {
    pub writer_guid: GUID,
    pub first_sn: SequenceNumber,
    pub last_sn: SequenceNumber,
    pub count: i32,
}

/// The events records the receipt of a ACK-NACK submessage.
#[derive(Debug, Clone)]
pub struct AckNackEvent {
    pub writer_guid: GUID,
    pub reader_guid: GUID,
    pub count: i32,
    pub base_sn: i64,
    pub missing_sn: Vec<i64>,
}

/// The events records the receipt of a HEARTBEAT-FRAG submessage.
#[derive(Debug, Clone)]
pub struct HeartbeatFragEvent {
    pub writer_guid: GUID,
    pub writer_sn: SequenceNumber,
    pub last_fragment_num: FragmentNumber,
    pub count: i32,
}

/// The events records the receipt of a DATA submessage.
#[derive(Debug, Clone)]
pub struct DataEvent {
    pub writer_guid: GUID,
    pub writer_sn: SequenceNumber,
    pub payload_size: usize,
    pub payload: Option<DataPayload>,
}

/// The events records the receipt of a DATA-FRAG submessage.
#[derive(Debug, Clone)]
pub struct DataFragEvent {
    pub writer_guid: GUID,
    pub writer_sn: SequenceNumber,
    pub fragment_starting_num: u32,
    pub fragments_in_submessage: u16,
    pub data_size: u32,
    pub fragment_size: u16,
    pub payload_size: usize,
    pub payload_hash: u64,
}

/// The events records the receipt of a GAP submessage.
#[derive(Debug, Clone)]
pub struct GapEvent {
    pub writer_guid: GUID,
    pub reader_guid: GUID,
    pub gap_start: SequenceNumber,
    pub gap_list: SequenceNumberSet,
}

/// The events records the receipt of a NACK-FRAG submessage.
#[derive(Debug, Clone)]
pub struct NackFragEvent {
    pub writer_guid: GUID,
    pub reader_guid: GUID,
    pub writer_sn: SequenceNumber,
    pub count: i32,
}

/// Records the GUID prefix and locators of an observed participant.
#[derive(Debug, Clone)]
pub struct ParticipantInfo {
    pub recv_time: chrono::Duration,
    pub guid_prefix: GuidPrefix,
    pub unicast_locator_list: Vec<Locator>,
    pub multicast_locator_list: Option<Vec<Locator>>,
}
