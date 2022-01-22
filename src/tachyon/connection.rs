use super::network_address::NetworkAddress;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Connection {
    pub address: NetworkAddress,
    pub identity: Identity,
    pub tachyon_id: u16,
    pub received_at: u64,
    pub since_last_received: u64,
}

impl Connection {
    pub fn create(address: NetworkAddress, tachyon_id: u16) -> Self {
        let conn = Connection {
            identity: Identity::default(),
            address: address,
            tachyon_id,
            received_at: 0,
            since_last_received: 0,
        };
        return conn;
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
#[derive(Default)]
pub struct Identity {
    pub id: u32,
    pub session_id: u32,
    pub linked: u32,
}

impl Identity {
    pub fn is_valid(&self) -> bool {
        return self.id > 0 && self.session_id > 0;
    }

    pub fn is_linked(&self) -> bool {
        return self.is_valid() && self.linked == 1;
    }

    pub fn set_linked(&mut self, linked: u32) {
        self.linked = linked;
    }
}
