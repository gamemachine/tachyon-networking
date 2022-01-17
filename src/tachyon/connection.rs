

use super::{network_address::NetworkAddress};

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Connection {
    pub address: NetworkAddress,
    pub tachyon_id: u16
}

impl Connection {
    pub fn create(address: NetworkAddress, tachyon_id: u16) -> Self {
        let conn = Connection {
            address: address,
            tachyon_id
        };
        return conn;
    }

}
