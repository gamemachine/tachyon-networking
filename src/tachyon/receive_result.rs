use super::network_address::NetworkAddress;

pub const RECEIVE_ERROR_UNKNOWN: u32 = 1;
pub const RECEIVE_ERROR_CHANNEL: u32 = 2;

pub enum ReceiveResult {
    Reliable {
        network_address: NetworkAddress,
        channel_id: u8,
    },
    Error,
    Empty,
    Retry,
    ChannelError,
    UnReliable {
        received_len: usize,
        network_address: NetworkAddress,
    },
}

#[repr(C)]
#[derive(Default)]
#[derive(Clone, Copy)]
pub struct TachyonReceiveResult {
    pub channel: u16,
    pub address: NetworkAddress,
    pub length: u32,
    pub error: u32,
}

impl TachyonReceiveResult {
    pub fn default() -> Self {
        let result = TachyonReceiveResult {
            channel: 0,
            address: NetworkAddress::default(),
            length: 0,
            error: 0,
        };
        return result;
    }
}
