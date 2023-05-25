use rustdds::{discovery::data_types::topic_data::DiscoveredReaderData, SequenceNumber, GUID};

#[derive(Debug)]
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

#[derive(Debug)]
pub struct DataEvent {
    pub writer_id: GUID,
    pub reader_id: GUID,
    pub writer_sn: SequenceNumber,
    pub payload_size: usize,
    pub discovery_data: Option<DiscoveredReaderData>,
}

#[derive(Debug)]
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
