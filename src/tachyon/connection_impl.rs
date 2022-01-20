

use std::time::Instant;

use super::Tachyon;
use super::network_address::NetworkAddress;
use super::connection::Connection;
use super::header::{MESSAGE_TYPE_IDENTITY_LINKED, ConnectionHeader, MESSAGE_TYPE_IDENTITY_UNLINKED, MESSAGE_TYPE_LINK_IDENTITY, MESSAGE_TYPE_UNLINK_IDENTITY};

const IDENTITY_SEND_INTERVAL: u128 = 300;

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
    
    pub fn try_create_connection(&mut self, address: NetworkAddress) -> bool {
        if !self.connections.contains_key(&address) {
            let conn = Connection::create(address, self.id);
            self.connections.insert(address, conn);
            return true;
        } else {
            return false;
        }
    }

    pub fn get_connections(&mut self, max: u16) -> Vec<Connection> {
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

     // run when use_identity is not set
    pub fn on_receive_connection_update(&mut self, address: NetworkAddress) {
        let since_start = self.time_since_start();
        if let Some(conn) = self.connections.get_mut(&address) {
            conn.received_at = since_start;
        } else {
            let mut conn = Connection::create(address, self.id);
            conn.received_at = since_start;
            self.connections.insert(address, conn);
            self.create_configured_channels(address);
        }
    }

    pub fn validate_and_update_linked_connection(&mut self, address: NetworkAddress) -> bool {
        let since_start = self.time_since_start();
        if let Some(conn) = self.connections.get_mut(&address) {
            if conn.id == 0 {
                return false;
            }
            conn.received_at = since_start;
            return true;
        }
        return false;
    }

    pub fn remove_connection_by_identity(&mut self, id: u32) {
        let mut addresses: Vec<NetworkAddress> = Vec::new();

        for conn in self.connections.values_mut() {
            if conn.id == id {
                addresses.push(conn.address);
            }
        }
        for addr in addresses {
            self.connections.remove(&addr);
            self.remove_configured_channels(addr);
        }
    }

    pub fn try_link_identity(&mut self, address: NetworkAddress, id: u32, session_id: u32) -> bool {
        if let Some(current_session_id) = self.identities.get(&id) {
            if session_id != *current_session_id {
                return false;
            }

            self.remove_connection_by_identity(id);
            let mut conn = Connection::create(address, self.id);
            conn.id = id;
            self.connections.insert(address, conn);
            self.create_configured_channels(address);
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

    pub fn send_link_identity(&self,id: u32, session_id: u32) {
        self.send_identity_message(MESSAGE_TYPE_LINK_IDENTITY,id, session_id, NetworkAddress::default());
    }

    pub fn send_unlink_identity(&self,id: u32, session_id: u32) {
        self.send_identity_message(MESSAGE_TYPE_UNLINK_IDENTITY,id, session_id, NetworkAddress::default());
    }

    pub fn send_identity_linked(&self, address: NetworkAddress) {
        self.send_identity_message(MESSAGE_TYPE_IDENTITY_LINKED,0,0, address);
    }

    pub fn send_identity_unlinked(&self, address: NetworkAddress) {
        self.send_identity_message(MESSAGE_TYPE_IDENTITY_UNLINKED,0,0, address);
    }

    fn send_identity_message(&self, message_type: u8 ,id: u32, session_id: u32, address: NetworkAddress) {
        let mut header = ConnectionHeader::default();
        header.message_type = message_type;
        header.id = id;
        header.session_id = session_id;
        let mut send_buffer: Vec<u8> = vec![0;12];
        header.write(&mut send_buffer);
        self.socket.send_to(address, &send_buffer, send_buffer.len());
    }
}



#[cfg(test)]
mod tests {
    use std::time::{Instant, Duration};

    use crate::tachyon::{tachyon_test::TachyonTest, TachyonConfig, Tachyon, network_address::NetworkAddress, connection::Identity};


    #[test]
    fn test_connect() {
        let address = NetworkAddress::localhost(100);
        let changed_address = NetworkAddress::localhost(200);

        let config = TachyonConfig::default();
        let mut server = Tachyon::create(config);
        server.set_identity(1, 10);

        assert!(!server.try_link_identity(address, 1, 11));

        assert!(server.try_link_identity(address, 1,10));
        assert!(server.connections.contains_key(&address));
        assert_eq!(2, server.get_channel_count(address));

        // connect when connected is valid
        assert!(server.try_link_identity(address, 1,10));
        assert!(server.connections.contains_key(&address));
        assert_eq!(2, server.get_channel_count(address));

        // connect with new address wipes out old connection
        assert!(server.try_link_identity(changed_address, 1,10));
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
        server.try_link_identity(address, 1,10);
        
        assert!(!server.try_unlink_identity(address, 1,11));

        assert!(server.try_unlink_identity(address, 1,10));
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

        server.try_link_identity(address, 1,10);
        assert!(server.validate_and_update_linked_connection(address));
    }

    #[test]
    fn test_can_send() {
        let address = NetworkAddress::localhost(100);

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
    fn test_link_flow() {
        let mut test = TachyonTest::default();
        test.client.config.use_identity = 1;
        test.client.identity = Identity {
            id: 1,
            session_id: 11,
            linked: 0
        };

        test.server.config.use_identity = 1;
        test.server.set_identity(1, 10);

        test.connect();


        // link fails = bad session id
        test.client.last_identity_link_request = Instant::now() - Duration::new(10, 0);
        test.client.update();
        test.server_receive();
        test.client_receive();
        assert!(!test.client.identity.is_linked());

        // linked
        test.server.set_identity(1, 11);
        test.client.last_identity_link_request = Instant::now() - Duration::new(10, 0);
        test.client.update();
        test.server_receive();
        test.client_receive();
        assert!(test.client.identity.is_linked());

        // unlinked
        test.client.send_unlink_identity(test.client.identity.id, test.client.identity.session_id);
        test.server_receive();
        test.client_receive();
        assert!(!test.client.identity.is_linked());
    }

}