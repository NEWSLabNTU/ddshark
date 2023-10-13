use etherparse::{Ethernet2Header, SingleVlanHeader};
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
    SequenceNumber, GUID,
};
use smoltcp::wire::{Ipv4Repr, UdpRepr};

#[derive(Debug, Clone)]
pub enum UpdateEvent {
    RtpsMsg(RtpsMsgEvent),
    RtpsSubmsg(RtpsSubmsgEvent),
    ParticipantInfo(ParticipantInfo),
    Tick,
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

#[derive(Debug, Clone)]
pub enum RtpsSubmsgEvent {
    Data(Box<DataEvent>),
    DataFrag(Box<DataFragEvent>),
    Gap(Box<GapEvent>),
    AckNack(AckNackEvent),
    NackFrag(NackFragEvent),
    Heartbeat(HeartbeatEvent),
    HeartbeatFrag(HeartbeatFragEvent),
}

impl From<NackFragEvent> for RtpsSubmsgEvent {
    fn from(v: NackFragEvent) -> Self {
        Self::NackFrag(v)
    }
}

impl From<DataFragEvent> for RtpsSubmsgEvent {
    fn from(v: DataFragEvent) -> Self {
        Self::DataFrag(Box::new(v))
    }
}

impl From<DataEvent> for RtpsSubmsgEvent {
    fn from(v: DataEvent) -> Self {
        Self::Data(Box::new(v))
    }
}

impl From<GapEvent> for RtpsSubmsgEvent {
    fn from(v: GapEvent) -> Self {
        Self::Gap(Box::new(v))
    }
}

impl From<HeartbeatEvent> for RtpsSubmsgEvent {
    fn from(v: HeartbeatEvent) -> Self {
        Self::Heartbeat(v)
    }
}

impl From<HeartbeatFragEvent> for RtpsSubmsgEvent {
    fn from(v: HeartbeatFragEvent) -> Self {
        Self::HeartbeatFrag(v)
    }
}

impl From<AckNackEvent> for RtpsSubmsgEvent {
    fn from(v: AckNackEvent) -> Self {
        Self::AckNack(v)
    }
}

#[derive(Debug, Clone)]
pub struct RtpsMsgEvent {
    pub headers: PacketHeaders,
}

#[derive(Debug, Clone)]
pub struct PacketHeaders {
    pub pcap_header: pcap::PacketHeader,
    pub eth_header: Ethernet2Header,
    pub vlan_header: Option<SingleVlanHeader>,
    pub ipv4_header: Ipv4Repr,
    // pub udp_header: UdpRepr,
    pub ts: chrono::Duration,
}

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

#[derive(Debug, Clone)]
pub struct DiscoveredTopicEvent {
    pub data: DiscoveredTopicData,
}

#[derive(Debug, Clone)]
pub struct DiscoveredWriterEvent {
    pub data: DiscoveredWriterData,
}

#[derive(Debug, Clone)]
pub struct DiscoveredReaderEvent {
    pub data: DiscoveredReaderData,
}

#[derive(Debug, Clone)]
pub struct HeartbeatEvent {
    pub writer_guid: GUID,
    pub first_sn: SequenceNumber,
    pub last_sn: SequenceNumber,
    pub count: i32,
}

#[derive(Debug, Clone)]
pub struct AckNackEvent {
    pub writer_guid: GUID,
    pub reader_guid: GUID,
    pub count: i32,
    pub base_sn: i64,
    pub missing_sn: Vec<i64>,
}

#[derive(Debug, Clone)]
pub struct HeartbeatFragEvent {
    pub writer_guid: GUID,
    pub writer_sn: SequenceNumber,
    pub last_fragment_num: FragmentNumber,
    pub count: i32,
}

#[derive(Debug, Clone)]
pub struct DataEvent {
    pub writer_guid: GUID,
    pub writer_sn: SequenceNumber,
    pub payload_size: usize,
    pub payload: Option<DataPayload>,
}

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

#[derive(Debug, Clone)]
pub struct GapEvent {
    pub writer_guid: GUID,
    pub reader_guid: GUID,
    pub gap_start: SequenceNumber,
    pub gap_list: SequenceNumberSet,
}

#[derive(Debug, Clone)]
pub struct NackFragEvent {
    pub writer_guid: GUID,
    pub reader_guid: GUID,
    pub writer_sn: SequenceNumber,
    pub count: i32,
}

#[derive(Debug, Clone)]
pub struct ParticipantInfo {
    pub guid_prefix: GuidPrefix,
    pub unicast_locator_list: Vec<Locator>,
    pub multicast_locator_list: Option<Vec<Locator>>,
}
