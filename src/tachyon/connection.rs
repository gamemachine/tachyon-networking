

use super::{network_address::NetworkAddress};


pub struct Connection {
    pub address: NetworkAddress
}

impl Connection {
    pub fn create(address: NetworkAddress) -> Self {
        let conn = Connection {
            address: address
        };
        return conn;
    }

}
