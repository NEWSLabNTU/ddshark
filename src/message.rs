use etherparse::{Ethernet2Header, SingleVlanHeader};
use rustdds::{
    dds::DiscoveredTopicData,
    discovery::data_types::{
        spdp_participant_data::SpdpDiscoveredParticipantData,
        topic_data::{DiscoveredReaderData, DiscoveredWriterData},
    },
    SequenceNumber, GUID,
};
use smoltcp::wire::Ipv4Repr;

#[derive(Debug, Clone)]
pub enum RtpsEvent {
    Data(Box<DataEvent>),
    DataFrag(Box<DataFragEvent>),
}

#[derive(Debug, Clone)]
pub enum DataPayload {
    DiscoveredTopic(Box<DiscoveredTopicData>),
    DiscoveredWriter(Box<DiscoveredWriterData>),
    DiscoveredReader(Box<DiscoveredReaderData>),
    DiscoveredParticipant(Box<SpdpDiscoveredParticipantData>),
}

impl From<SpdpDiscoveredParticipantData> for DataPayload {
    fn from(v: SpdpDiscoveredParticipantData) -> Self {
        Self::DiscoveredParticipant(Box::new(v))
    }
}

impl From<DiscoveredReaderData> for DataPayload {
    fn from(v: DiscoveredReaderData) -> Self {
        Self::DiscoveredReader(Box::new(v))
    }
}

impl From<DiscoveredWriterData> for DataPayload {
    fn from(v: DiscoveredWriterData) -> Self {
        Self::DiscoveredWriter(Box::new(v))
    }
}

impl From<DiscoveredTopicData> for DataPayload {
    fn from(v: DiscoveredTopicData) -> Self {
        Self::DiscoveredTopic(Box::new(v))
    }
}

impl From<DataFragEvent> for RtpsEvent {
    fn from(v: DataFragEvent) -> Self {
        Self::DataFrag(Box::new(v))
    }
}

impl From<DataEvent> for RtpsEvent {
    fn from(v: DataEvent) -> Self {
        Self::Data(Box::new(v))
    }
}

#[derive(Debug, Clone)]
pub struct DataEvent {
    pub writer_id: GUID,
    pub reader_id: GUID,
    pub writer_sn: SequenceNumber,
    pub payload_size: usize,
    pub payload: Option<DataPayload>,
}

#[derive(Debug, Clone)]
pub struct DataFragEvent {
    pub writer_id: GUID,
    pub reader_id: GUID,
    pub writer_sn: SequenceNumber,
    pub fragment_starting_num: u32,
    pub fragments_in_submessage: u16,
    pub data_size: u32,
    pub fragment_size: u16,
    pub payload_size: usize,
}

#[derive(Debug, Clone)]
pub struct PacketHeaders {
    pub pcap_header: pcap::PacketHeader,
    pub eth_header: Ethernet2Header,
    pub vlan_header: Option<SingleVlanHeader>,
    pub ipv4_header: Ipv4Repr,
}

#[derive(Debug, Clone)]
pub struct RtpsMessage {
    pub headers: PacketHeaders,
    pub event: RtpsEvent,
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
