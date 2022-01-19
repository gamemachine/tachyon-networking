


use super::{network_address::NetworkAddress};

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Connection {
    pub address: NetworkAddress,
    pub tachyon_id: u16,
    pub received_at: u64,
    pub since_last_received: u64
}

impl Connection {
    pub fn create(address: NetworkAddress, tachyon_id: u16) -> Self {
        let conn = Connection {
            address: address,
            tachyon_id,
            received_at: 0,
            since_last_received: 0
        };
        return conn;
    }

}
