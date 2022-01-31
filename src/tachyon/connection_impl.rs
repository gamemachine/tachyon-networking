use std::time::Instant;

use super::connection::{Connection, Identity};
use super::header::{
    ConnectionHeader, MESSAGE_TYPE_IDENTITY_LINKED, MESSAGE_TYPE_IDENTITY_UNLINKED,
    MESSAGE_TYPE_LINK_IDENTITY, MESSAGE_TYPE_UNLINK_IDENTITY,
};
use super::network_address::NetworkAddress;
use super::Tachyon;

const IDENTITY_SEND_INTERVAL: u128 = 300;

pub const CONNECTION_ADDED_EVENT: u8 = 1;
pub const CONNECTION_REMOVED_EVENT: u8 = 2;

pub const LINK_IDENTITY_EVENT: u8 = 1;
pub const UNLINK_IDENTITY_EVENT: u8 = 2;
pub const IDENTITY_LINKED_EVENT: u8 = 3;
pub const IDENTITY_UNLINKED_EVENT: u8 = 4;

pub type ConnectionEventCallback = unsafe extern "C" fn(action: u8, connection: Connection);
pub type IdentityEventCallback = unsafe extern "C" fn(action: u8, connection: Connection);

impl Tachyon {
    // setting identity removes any associated connection
    pub fn set_identity(&mut self, id: u32, session_id: u32) {
        self.remove_connection_by_identity(id);

        if session_id == 0 {
            self.identities.remove(&id);
        } else {
            self.identities.insert(id, session_id);
        }
    }

    pub fn create_connection(&mut self, address: NetworkAddress, identity: Identity) {
        let mut conn = Connection::create(address, self.id);
        conn.identity = identity;
        conn.received_at = self.time_since_start();
        self.connections.insert(address, conn);
        self.create_configured_channels(address);
        self.fire_connection_event(CONNECTION_ADDED_EVENT, address);
    }

    fn remove_connection(&mut self, address: NetworkAddress) {
        self.connections.remove(&address);
        self.remove_configured_channels(address);
        self.fire_connection_event(CONNECTION_REMOVED_EVENT, address);
    }

    pub fn get_connection(&self, address: NetworkAddress) -> Option<&Connection> {
        return self.connections.get(&address);
    }

    pub fn get_connection_by_identity(&self, id: u32) -> Option<&Connection> {
        if let Some(address) = self.identity_to_address_map.get(&id) {
            return self.get_connection(*address);
        }
        return None;
    }

    pub fn get_connections(&mut self, max: i32) -> Vec<Connection> {
        let mut list: Vec<Connection> = Vec::new();
        let result_count = std::cmp::min(self.connections.len(), max as usize);

        let mut count = 0;
        let since_start = self.time_since_start();
        for conn in self.connections.values_mut() {
            conn.since_last_received = since_start - conn.received_at;
            list.push(*conn);
            count += 1;
            if count >= result_count {
                break;
            }
        }
        return list;
    }

    pub fn fire_identity_event(&self, event_id: u8, address: NetworkAddress, id: u32, session_id: u32) {
        if let Some(callback) = self.identity_event_callback {
            let mut conn = Connection::create(address, self.id);
            conn.identity = Identity {id, session_id, linked: 0 };
            if event_id == IDENTITY_LINKED_EVENT {
               conn.identity.linked = 1; 
            }
            unsafe {
                callback(event_id, conn);
            }
        }
    }

    pub fn fire_connection_event(&self, event_id: u8, address: NetworkAddress) {
        if let Some(callback) = self.connection_event_callback {
            let conn = Connection::create(address, self.id);
            unsafe {
                callback(event_id, conn);
            }
        }
    }

    // run when use_identity is not set
    pub fn on_receive_connection_update(&mut self, address: NetworkAddress) {
        let since_start = self.time_since_start();
        if let Some(conn) = self.connections.get_mut(&address) {
            conn.received_at = since_start;
        } else {
            self.create_connection(address, Identity::default());
        }
    }

    pub fn validate_and_update_linked_connection(&mut self, address: NetworkAddress) -> bool {
        let since_start = self.time_since_start();
        if let Some(conn) = self.connections.get_mut(&address) {
            if conn.identity.id == 0 {
                return false;
            }
            conn.received_at = since_start;
            return true;
        }
        return false;
    }

    pub fn get_connection_identity(&self, address: NetworkAddress) -> Identity {
        if let Some(conn) = self.connections.get(&address) {
            return conn.identity;
        } else {
            return Identity::default();
        }
    }

    

    pub fn remove_connection_by_identity(&mut self, id: u32) {
        let mut addresses: Vec<NetworkAddress> = Vec::new();

        for conn in self.connections.values_mut() {
            if conn.identity.id == id {
                addresses.push(conn.address);
            }
        }
        for addr in addresses {
            self.remove_connection(addr);
        }
    }

    

    pub fn try_link_identity(&mut self, address: NetworkAddress, id: u32, session_id: u32) -> bool {
        if let Some(current_session_id) = self.identities.get(&id) {
            if session_id != *current_session_id {
                return false;
            }

            let identity = self.get_connection_identity(address);
            if identity.id == id && identity.session_id == *current_session_id {
                return true;
            }

            self.remove_connection_by_identity(id);
            let identity = Identity {
                id: id,
                session_id: session_id,
                linked: 0,
            };
            self.create_connection(address, identity);
            self.identity_to_address_map.insert(id, address);
            self.send_identity_linked(address);
            return true;
        }
        return false;
    }

    pub fn try_unlink_identity(&mut self, address: NetworkAddress, id: u32, session_id: u32) -> bool {
        if let Some(current_session_id) = self.identities.get(&id) {
            if session_id != *current_session_id {
                return false;
            }

            self.remove_connection_by_identity(id);
            self.identity_to_address_map.remove(&id);
            self.send_identity_unlinked(address);
            return true;
        }
        self.send_identity_unlinked(address);
        return false;
    }

    pub fn client_identity_update(&mut self) {
        if self.config.use_identity == 0 {
            return;
        }

        if self.socket.socket.is_none() {
            return;
        }

        if self.socket.is_server {
            return;
        }

        if !self.identity.is_valid() {
            return;
        }

        if self.identity.is_linked() {
            return;
        }

        let since_last = Instant::now() - self.last_identity_link_request;
        if since_last.as_millis() > IDENTITY_SEND_INTERVAL {
            self.last_identity_link_request = Instant::now();
            self.send_link_identity(self.identity.id, self.identity.session_id);
        }
    }

    pub fn can_send(&self) -> bool {
        if self.socket.is_server {
            return true;
        } else {
            if self.config.use_identity == 1 {
                return self.identity.linked == 1;
            } else {
                return true;
            }
        }
    }

    pub fn send_link_identity(&self, id: u32, session_id: u32) {
        self.send_identity_message(MESSAGE_TYPE_LINK_IDENTITY, id, session_id,  NetworkAddress::default());
    }

    pub fn send_unlink_identity(&self, id: u32, session_id: u32) {
        self.send_identity_message(MESSAGE_TYPE_UNLINK_IDENTITY, id, session_id, NetworkAddress::default());
    }

    pub fn send_identity_linked(&self, address: NetworkAddress) {
        self.send_identity_message(MESSAGE_TYPE_IDENTITY_LINKED, 0, 0, address);
    }

    pub fn send_identity_unlinked(&self, address: NetworkAddress) {
        self.send_identity_message(MESSAGE_TYPE_IDENTITY_UNLINKED, 0, 0, address);
    }

    fn send_identity_message(&self, message_type: u8, id: u32, session_id: u32, address: NetworkAddress) {
        let mut header = ConnectionHeader::default();
        header.message_type = message_type;
        header.id = id;
        header.session_id = session_id;
        let mut send_buffer: Vec<u8> = vec![0; 12];
        header.write(&mut send_buffer);
        self.socket.send_to(address, &send_buffer, send_buffer.len());
    }
}

#[cfg(test)]
mod tests {

    use serial_test::serial;

    use crate::tachyon::{
        connection::Identity, network_address::NetworkAddress, tachyon_test::TachyonTest, Tachyon,
        TachyonConfig,
    };

    #[test]
    fn test_connect() {
        let address = NetworkAddress::localhost(100);
        let changed_address = NetworkAddress::localhost(200);

        let config = TachyonConfig::default();
        let mut server = Tachyon::create(config);
        server.set_identity(1, 10);

        assert!(!server.try_link_identity(address, 1, 11));

        assert!(server.try_link_identity(address, 1, 10));
        assert!(server.connections.contains_key(&address));
        assert_eq!(2, server.get_channel_count(address));

        // connect when connected is valid
        assert!(server.try_link_identity(address, 1, 10));
        assert!(server.connections.contains_key(&address));
        assert_eq!(2, server.get_channel_count(address));

        // connect with new address wipes out old connection
        assert!(server.try_link_identity(changed_address, 1, 10));
        assert!(server.connections.contains_key(&changed_address));
        assert_eq!(2, server.get_channel_count(changed_address));

        assert!(!server.connections.contains_key(&address));
        assert_eq!(0, server.get_channel_count(address));
    }

    #[test]
    fn test_disconnect() {
        let address = NetworkAddress::localhost(100);

        let config = TachyonConfig::default();
        let mut server = Tachyon::create(config);
        server.set_identity(1, 10);
        server.try_link_identity(address, 1, 10);

        assert!(!server.try_unlink_identity(address, 1, 11));

        assert!(server.try_unlink_identity(address, 1, 10));
        assert!(!server.connections.contains_key(&address));
        assert_eq!(0, server.get_channel_count(address));
    }

    #[test]
    fn test_validate_and_update_connection() {
        let address = NetworkAddress::localhost(100);

        let config = TachyonConfig::default();
        let mut server = Tachyon::create(config);
        server.set_identity(1, 10);

        assert!(!server.validate_and_update_linked_connection(address));

        server.try_link_identity(address, 1, 10);
        assert!(server.validate_and_update_linked_connection(address));
    }

    #[test]
    fn test_can_send() {

        let mut config = TachyonConfig::default();
        config.use_identity = 1;
        let mut tach = Tachyon::create(config);
        tach.socket.is_server = true;
        assert!(tach.can_send());

        tach.socket.is_server = false;
        assert!(!tach.can_send());
        tach.identity.linked = 1;
        assert!(tach.can_send());
    }

    #[test]
    #[serial]
    fn test_link_flow() {
        let mut test = TachyonTest::default();
        test.client.config.use_identity = 1;
        test.client.identity = Identity {
            id: 1,
            session_id: 11,
            linked: 0,
        };

        test.server.config.use_identity = 1;
        test.server.set_identity(1, 10);

        test.connect();

        // linked
        test.server.set_identity(1, 11);
        test.client.update();
        test.server_receive();
        test.client_receive();
        assert!(test.client.identity.is_linked());

        // unlinked
        test.client
            .send_unlink_identity(test.client.identity.id, test.client.identity.session_id);
        test.server_receive();
        test.client_receive();
        assert!(!test.client.identity.is_linked());
    }

    #[test]
    #[serial]
    fn test_link_fail_flow() {
        let mut test = TachyonTest::default();
        test.client.config.use_identity = 1;
        test.client.identity = Identity {
            id: 1,
            session_id: 11,
            linked: 0,
        };

        test.server.config.use_identity = 1;
        test.server.set_identity(1, 10);

        test.connect();

        // link fails = bad session id
        test.client.update();
        test.server_receive();
        test.client_receive();
        assert!(!test.client.identity.is_linked());
    }
}
