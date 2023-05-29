use etherparse::{Ethernet2Header, SingleVlanHeader};
use pcap;
use rustdds::{discovery::data_types::topic_data::DiscoveredWriterData, SequenceNumber, GUID};
use smoltcp::wire::{Ipv4Packet, Ipv4Repr};

#[derive(Debug, Clone)]
pub enum RtpsEvent {
    Data(DataEvent),
    DataFrag(DataFragEvent),
}

impl From<DataFragEvent> for RtpsEvent {
    fn from(v: DataFragEvent) -> Self {
        Self::DataFrag(v)
    }
}

impl From<DataEvent> for RtpsEvent {
    fn from(v: DataEvent) -> Self {
        Self::Data(v)
    }
}

#[derive(Debug, Clone)]
pub struct DataEvent {
    pub writer_id: GUID,
    pub reader_id: GUID,
    pub writer_sn: SequenceNumber,
    pub payload_size: usize,
    pub discovery_data: Option<DiscoveredWriterData>,
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
