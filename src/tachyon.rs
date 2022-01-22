pub mod channel;
pub mod connection;
pub mod ffi;
pub mod fragmentation;
pub mod header;
pub mod int_buffer;
pub mod nack;
pub mod network_address;
pub mod pool;
pub mod receive_result;
pub mod receiver;
pub mod send_buffer_manager;
pub mod sequence;
pub mod sequence_buffer;
pub mod tachyon_socket;
pub mod unreliable_sender;

mod connection_impl;

// additional stress/scale testing
#[cfg(test)]
pub mod tachyon_test;

use std::time::Duration;
use std::time::Instant;

use rustc_hash::FxHashMap;

use self::channel::*;
use self::connection::*;
use self::fragmentation::*;
use self::header::*;
use self::network_address::NetworkAddress;
use self::receive_result::ReceiveResult;
use self::receive_result::TachyonReceiveResult;
use self::receive_result::RECEIVE_ERROR_CHANNEL;
use self::receive_result::RECEIVE_ERROR_UNKNOWN;
use self::receiver::RECEIVE_WINDOW_SIZE_DEFAULT;
use self::tachyon_socket::*;
use self::unreliable_sender::UnreliableSender;

pub const SEND_ERROR_CHANNEL: u32 = 2;
pub const SEND_ERROR_SOCKET: u32 = 1;
pub const SEND_ERROR_FRAGMENT: u32 = 3;
pub const SEND_ERROR_UNKNOWN: u32 = 4;
pub const SEND_ERROR_LENGTH: u32 = 5;
pub const SEND_ERROR_IDENTITY: u32 = 6;

const NACK_REDUNDANCY_DEFAULT: u32 = 1;

pub type OnConnectedCallback = unsafe extern "C" fn();

#[derive(Clone, Copy)]
#[repr(C)]
#[derive(Default, Debug)]
pub struct TachyonStats {
    pub channel_stats: ChannelStats,
    pub packets_dropped: u64,
    pub unreliable_sent: u64,
    pub unreliable_received: u64,
}

impl std::fmt::Display for TachyonStats {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "channel_stats:{0} packets_dropped:{1} unreliable_sent:{2} unreliable_received:{3}\n",
            self.channel_stats,
            self.packets_dropped,
            self.unreliable_sent,
            self.unreliable_received
        )
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct TachyonConfig {
    pub use_identity: u32,
    pub drop_packet_chance: u64,
    pub drop_reliable_only: u32,
    pub receive_window_size: u16,
    pub nack_redundancy: u32
}

impl TachyonConfig {
    pub fn default() -> Self {
        let default = TachyonConfig {
            use_identity: 0,
            drop_packet_chance: 0,
            drop_reliable_only: 0,
            receive_window_size: RECEIVE_WINDOW_SIZE_DEFAULT,
            nack_redundancy: NACK_REDUNDANCY_DEFAULT
        };
        return default;
    }

    pub fn get_receive_window_size(&self) -> u16 {
        if self.receive_window_size > 0 {
            return self.receive_window_size;
        } else {
            return RECEIVE_WINDOW_SIZE_DEFAULT;
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
#[derive(Default)]
pub struct TachyonSendResult {
    pub sent_len: u32,
    pub error: u32,
    pub header: Header,
}

pub struct Tachyon {
    pub id: u16,
    pub socket: TachyonSocket,
    pub unreliable_sender: Option<UnreliableSender>,
    pub identities: FxHashMap<u32, u32>,
    pub connections: FxHashMap<NetworkAddress, Connection>,
    pub channels: FxHashMap<(NetworkAddress, u8), Channel>,
    pub channel_config: FxHashMap<u8, bool>,
    pub config: TachyonConfig,
    pub nack_send_data: Vec<u8>,
    pub stats: TachyonStats,
    pub start_time: Instant,
    pub last_identity_link_request: Instant,
    pub identity: Identity,
    pub on_connected_callback: Option<OnConnectedCallback>,
}

impl Tachyon {
    pub fn create(config: TachyonConfig) -> Self {
        return Tachyon::create_with_id(config, 0);
    }

    pub fn create_with_id(config: TachyonConfig, id: u16) -> Self {
        let socket = TachyonSocket::create();

        let mut tachyon = Tachyon {
            id,
            identities: FxHashMap::default(),
            connections: FxHashMap::default(),
            channels: FxHashMap::default(),
            channel_config: FxHashMap::default(),
            socket: socket,
            unreliable_sender: None,
            config,
            nack_send_data: vec![0; 4096],
            stats: TachyonStats::default(),
            start_time: Instant::now(),
            last_identity_link_request: Instant::now() - Duration::new(100, 0),
            identity: Identity::default(),
            on_connected_callback: None,
        };
        tachyon.channel_config.insert(1, true);
        tachyon.channel_config.insert(2, false);

        return tachyon;
    }


    pub fn time_since_start(&self) -> u64 {
        return Instant::now().duration_since(self.start_time).as_millis() as u64;
    }

    pub fn bind(&mut self, address: NetworkAddress) -> bool {
        match self.socket.bind_socket(address) {
            CreateConnectResult::Success => {
                self.unreliable_sender = self.create_unreliable_sender();
                return true;
            }
            CreateConnectResult::Error => {
                return false;
            }
        }
    }

    pub fn connect(&mut self, address: NetworkAddress) -> bool {
        match self.socket.connect_socket(address) {
            CreateConnectResult::Success => {
                let local_address = NetworkAddress::default();
                self.try_create_connection(local_address);
                self.create_configured_channels(local_address);
                self.unreliable_sender = self.create_unreliable_sender();
                return true;
            }
            CreateConnectResult::Error => {
                return false;
            }
        }
    }

    pub fn create_unreliable_sender(&self) -> Option<UnreliableSender> {
        let socket = self.socket.clone_socket();
        if !socket.is_some() {
            return None;
        }
        let sender = UnreliableSender { socket: socket };
        return Some(sender);
    }

    pub fn get_channel(&mut self, address: NetworkAddress, channel_id: u8) -> Option<&mut Channel> {
        match self.channels.get_mut(&(address, channel_id)) {
            Some(channel) => {
                return Some(channel);
            }
            None => {
                return None;
            }
        }
    }

    fn create_configured_channels(&mut self, address: NetworkAddress) {
        for config in &self.channel_config {
            let channel_id = *config.0;
            let ordered = *config.1;
            match self.channels.get_mut(&(address, channel_id)) {
                Some(_) => {}
                None => {
                    let channel = Channel::create(channel_id, ordered, address, self.config.get_receive_window_size(), self.config.nack_redundancy);
                    self.channels.insert((address, channel_id), channel);
                }
            }
        }
    }

    pub fn get_channel_count(&mut self, address: NetworkAddress) -> u32 {
        let mut count = 0;
        for config in &self.channel_config {
            let channel_id = *config.0;
            if self.channels.contains_key(&(address, channel_id)) {
                count += 1;
            }
        }
        return count;
    }

    fn remove_configured_channels(&mut self, address: NetworkAddress) {
        for config in &self.channel_config {
            let channel_id = *config.0;
            self.channels.remove(&(address, channel_id));
        }
    }

    pub fn configure_channel(&mut self, channel_id: u8, ordered: bool) -> bool {
        if channel_id < 3 {
            return false;
        }
        self.channel_config.insert(channel_id, ordered);
        return true;
    }

    pub fn get_combined_stats(&mut self) -> TachyonStats {
        let mut channel_stats = ChannelStats::default();
        for channel in self.channels.values_mut() {
            channel.update_stats();
            channel_stats.add_from(&channel.stats);
        }
        let mut stats = self.stats.clone();
        stats.channel_stats = channel_stats;
        return stats;
    }

    pub fn update(&mut self) {
        self.client_identity_update();

        for channel in self.channels.values_mut() {
            channel.update(&self.socket);
        }
    }

    fn receive_published_channel_id(&mut self,  receive_buffer: &mut [u8], address: NetworkAddress, channel_id: u8) -> u32 {
        match self.channels.get_mut(&(address, channel_id)) {
            Some(channel) => {
                let res = channel.receive_published(receive_buffer);
                return res.0;
            }
            None => {
                return 0;
            }
        }
    }

    fn receive_published_all_channels(&mut self, receive_buffer: &mut [u8]) -> TachyonReceiveResult {
        let mut result = TachyonReceiveResult::default();

        for channel in self.channels.values_mut() {
            let res = channel.receive_published(receive_buffer);
            if res.0 > 0 {
                result.length = res.0;
                result.address = res.1;
                result.channel = channel.id as u16;
                return result;
            }
        }
        return result;
    }

    pub fn receive_loop(&mut self, receive_buffer: &mut [u8]) -> TachyonReceiveResult {
        let mut result = TachyonReceiveResult::default();

        for _ in 0..100 {
            let receive_result = self.receive_from_socket(receive_buffer);
            match receive_result {
                ReceiveResult::Reliable {
                    network_address: socket_addr,
                    channel_id,
                } => {
                    let published =
                        self.receive_published_channel_id(receive_buffer, socket_addr, channel_id);
                    if published > 0 {
                        result.channel = channel_id as u16;
                        result.length = published;
                        result.address = socket_addr;
                        return result;
                    }
                }
                ReceiveResult::UnReliable {
                    received_len,
                    network_address: socket_addr,
                } => {
                    result.length = received_len as u32;
                    result.address = socket_addr;
                    return result;
                }
                ReceiveResult::Empty => {
                    break;
                }
                ReceiveResult::Retry => {}
                ReceiveResult::Error => {
                    result.error = RECEIVE_ERROR_UNKNOWN;
                    return result;
                }
                ReceiveResult::ChannelError => {
                    result.error = RECEIVE_ERROR_CHANNEL;
                    return result;
                }
            }
        }

        return self.receive_published_all_channels(receive_buffer);
    }

    fn receive_from_socket(&mut self, receive_buffer: &mut [u8]) -> ReceiveResult {
        let address: NetworkAddress;
        let received_len: usize;
        let header: Header;

        let socket_result = self.socket.receive(receive_buffer,self.config.drop_packet_chance,self.config.drop_reliable_only == 1);
        match socket_result {
            SocketReceiveResult::Success {bytes_received, network_address} => {
                received_len = bytes_received;
                address = network_address;

                header = Header::read(receive_buffer);

                if self.socket.is_server {
                    if self.config.use_identity == 1 {
                        let connection_header: ConnectionHeader;

                        if header.message_type == MESSAGE_TYPE_LINK_IDENTITY {
                            connection_header = ConnectionHeader::read(receive_buffer);
                            self.try_link_identity(address, connection_header.id, connection_header.session_id);
                            return ReceiveResult::Empty;
                        } else if header.message_type == MESSAGE_TYPE_UNLINK_IDENTITY {
                            connection_header = ConnectionHeader::read(receive_buffer);
                            self.try_unlink_identity(address, connection_header.id, connection_header.session_id);
                            return ReceiveResult::Empty;
                        } else {
                            if !self.validate_and_update_linked_connection(address) {
                                return ReceiveResult::Empty;
                            }
                        }
                    } else {
                        self.on_receive_connection_update(address);
                    }
                } else {
                    if self.config.use_identity == 1 {
                        if header.message_type == MESSAGE_TYPE_IDENTITY_LINKED {
                            self.identity.set_linked(1);
                            if let Some(callback) = self.on_connected_callback {
                                unsafe {
                                    callback();
                                }
                            }
                            return ReceiveResult::Empty;
                        } else if header.message_type == MESSAGE_TYPE_IDENTITY_UNLINKED {
                            self.identity.set_linked(0);
                            return ReceiveResult::Empty;
                        }

                        if !self.identity.is_linked() {
                            return ReceiveResult::Empty;
                        }
                    }
                }
            }
            SocketReceiveResult::Empty => {
                return ReceiveResult::Empty;
            }
            SocketReceiveResult::Error => {
                return ReceiveResult::Error;
            }
            SocketReceiveResult::Dropped => {
                self.stats.packets_dropped += 1;
                return ReceiveResult::Retry;
            }
        }

        if header.message_type == MESSAGE_TYPE_UNRELIABLE {
            self.stats.unreliable_received += 1;
            return ReceiveResult::UnReliable {
                received_len: received_len,
                network_address: address,
            };
        }

        let channel = match self.channels.get_mut(&(address, header.channel)) {
            Some(c) => c,
            None => {
                return ReceiveResult::ChannelError;
            }
        };

        channel.stats.bytes_received += received_len as u64;

        if header.message_type == MESSAGE_TYPE_NONE {
            channel.stats.nones_received += 1;
            if channel.receiver.receive_packet(header.sequence, receive_buffer, received_len)
            {
                channel.stats.nones_accepted += 1;
            }
            return ReceiveResult::Retry;
        }

        if header.message_type == MESSAGE_TYPE_NACK {
            channel.process_nack_message(address, receive_buffer);
            return ReceiveResult::Retry;
        }

        if header.message_type == MESSAGE_TYPE_FRAGMENT {
            channel.process_fragment_message(header.sequence, receive_buffer, received_len);
            return ReceiveResult::Retry;
        }

        

        if header.message_type == MESSAGE_TYPE_RELIABLE || header.message_type == MESSAGE_TYPE_RELIABLE_WITH_NACK {

            if header.message_type == MESSAGE_TYPE_RELIABLE_WITH_NACK {
                channel.process_single_nack(address, receive_buffer);
            }

            if channel.receiver.receive_packet(header.sequence, receive_buffer, received_len) {
                channel.stats.received += 1;
                return ReceiveResult::Reliable {
                    network_address: address,
                    channel_id: header.channel,
                };
            } else {
                return ReceiveResult::Retry;
            }
        }

        return ReceiveResult::Error;
    }

    pub fn send_unreliable(&mut self, address: NetworkAddress, data: &mut [u8], length: usize) -> TachyonSendResult {
        if !self.can_send() {
            let mut result = TachyonSendResult::default();
            result.error = SEND_ERROR_IDENTITY;
            return result;
        }

        match &self.unreliable_sender {
            Some(sender) => {
                let result = sender.send_unreliable(address, data, length);
                if result.error == 0 {
                    self.stats.unreliable_sent += 1;
                }
                return result;
            }
            None => {
                let mut result = TachyonSendResult::default();
                result.error = SEND_ERROR_UNKNOWN;
                return result;
            }
        }
    }

    pub fn send_reliable(&mut self, channel_id: u8, address: NetworkAddress, data: &mut [u8], body_len: usize) -> TachyonSendResult {
        let mut result = TachyonSendResult::default();

        if !self.can_send() {
            result.error = SEND_ERROR_IDENTITY;
            return result;
        }

        if body_len == 0 {
            result.error = SEND_ERROR_LENGTH;
            return result;
        }

        if channel_id == 0 {
            result.error = SEND_ERROR_CHANNEL;
            return result;
        }

        if !self.socket.socket.is_some() {
            result.error = SEND_ERROR_SOCKET;
            return result;
        }

        let channel = match self.channels.get_mut(&(address, channel_id)) {
            Some(c) => c,
            None => {
                result.error = SEND_ERROR_CHANNEL;
                return result;
            }
        };

        if Fragmentation::should_fragment(body_len) {
            let mut fragment_bytes_sent = 0;
            let frag_sequences = channel.frag.create_fragments(&mut channel.send_buffers, channel.id, data, body_len);
            if frag_sequences.len() == 0 {
                result.error = SEND_ERROR_FRAGMENT;
                return result;
            }

            for seq in frag_sequences {
                match channel.send_buffers.get_send_buffer(seq) {
                    Some(fragment) => {
                        let sent =self.socket.send_to(address, &fragment.buffer, fragment.buffer.len());
                        fragment_bytes_sent += sent;

                        channel.stats.bytes_sent += sent as u64;
                        channel.stats.fragments_sent += 1;
                    }
                    None => {
                        result.error = SEND_ERROR_FRAGMENT;
                        return result;
                    }
                }
            }

            result.header.message_type = MESSAGE_TYPE_FRAGMENT;
            result.sent_len = fragment_bytes_sent as u32;

            channel.stats.sent += 1;

            return result;
        }

        
        result = channel.send_reliable(address, data, body_len, &self.socket);
        return result;

        // let nack_option = channel.receiver.nack_queue.pop_front();
        // let header_size: usize;

        // if nack_option.is_some() {
        //     header_size = TACHYON_NACKED_HEADER_SIZE;
            
        // } else {
        //     header_size = TACHYON_HEADER_SIZE;
        // }
        // let send_buffer_len = length + header_size;

        // match channel.send_buffers.create_send_buffer(send_buffer_len) {
        //     Some(send_buffer) => {
        //         let sequence = send_buffer.sequence;
        //         let buffer = &mut send_buffer.buffer;
        //         buffer[header_size..length + header_size].copy_from_slice(&data[0..length]);

        //         let mut header = Header::default();
        //         header.message_type = message_type;
        //         header.channel = channel.id;
        //         header.sequence = sequence;

        //         if let Some(nack) = nack_option {
        //             header.start_sequence = nack.start_sequence;
        //             header.flags = nack.flags;
        //             channel.receiver.nack_queue.push_back(nack);
        //         }
                
        //         header.write(buffer);

        //         let sent_len = self.socket.send_to(address, &buffer, send_buffer_len);
        //         result.sent_len = sent_len as u32;
        //         result.header = header;

        //         channel.stats.bytes_sent += sent_len as u64;
        //         channel.stats.sent += 1;

        //         return result;
        //     }
        //     None => {
        //         result.error = SEND_ERROR_UNKNOWN;
        //         return result;
        //     }
        // }
    }
}

#[cfg(test)]
mod tests {

    use serial_test::serial;

    use crate::tachyon::tachyon_test::TachyonTest;

    use super::*;

    #[test]
    fn test_nack_rotation() {
        println!("{0}", 4 % 5);
    }

    #[test]
    #[serial]
    fn test_reliable() {
        // reliable messages just work with message bodies, headers are all internal

        let mut test = TachyonTest::default();
        test.connect();

        test.send_buffer[0] = 4;
        let sent = test.client_send_reliable(1, 2);
        // sent_len reports total including header.
        assert_eq!(2 + TACHYON_HEADER_SIZE, sent.sent_len as usize);

        let res = test.server_receive();
        assert_eq!(2, res.length);
        assert_eq!(4, test.receive_buffer[0]);

        test.client_send_reliable(2, 33);
        let res = test.server_receive();
        assert_eq!(33, res.length);

        // fragmented
        test.client_send_reliable(2, 3497);
        let res = test.server_receive();
        assert_eq!(3497, res.length);
    }

    #[test]
    #[serial]
    fn test_unconfigured_channel_fails() {
        let mut test = TachyonTest::default();
        test.client.configure_channel(3, true);
        test.connect();

        let sent = test.client_send_reliable(3, 2);
        assert_eq!(2 + TACHYON_HEADER_SIZE, sent.sent_len as usize);
        assert_eq!(0, sent.error);

        let res = test.server_receive();
        assert_eq!(0, res.length);
        assert_eq!(RECEIVE_ERROR_CHANNEL, res.error);
    }

    #[test]
    #[serial]
    fn test_configured_channel() {
        let mut test = TachyonTest::default();
        test.client.configure_channel(3, true);
        test.server.configure_channel(3, true);
        test.connect();

        let sent = test.client_send_reliable(3, 2);
        assert_eq!(2 + TACHYON_HEADER_SIZE, sent.sent_len as usize);
        assert_eq!(0, sent.error);

        let res = test.server_receive();
        assert_eq!(2, res.length);
        assert_eq!(0, res.error);
    }

    #[test]
    #[serial]
    fn test_unreliable() {
        let mut test = TachyonTest::default();
        test.connect();

        // unreliable messages need to be body length + 1;
        // send length error
        let send = test.client_send_unreliable(0);
        assert_eq!(SEND_ERROR_LENGTH, send.error);

        let res = test.server_receive();
        assert_eq!(0, res.length);

        test.receive_buffer[0] = 1;
        test.send_buffer[1] = 4;
        test.send_buffer[2] = 5;
        test.send_buffer[3] = 6;
        let sent = test.client_send_unreliable(4);
        assert_eq!(0, sent.error);
        assert_eq!(4, sent.sent_len as usize);

        let res = test.server_receive();
        assert_eq!(4, res.length);
        assert_eq!(0, test.receive_buffer[0]);
        assert_eq!(4, test.receive_buffer[1]);
        assert_eq!(5, test.receive_buffer[2]);
        assert_eq!(6, test.receive_buffer[3]);
    }
}
