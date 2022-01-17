    
pub mod ffi;
pub mod fragmentation;
pub mod tachyon_socket;
pub mod connection;
pub mod channel;
pub mod header;
pub mod sequence;
pub mod receiver;
pub mod nack;
pub mod send_buffer_manager;
pub mod network_address;
pub mod sequence_buffer;
pub mod int_buffer;
pub mod receive_result;
pub mod pool;
pub mod unreliable_sender;

// additional stress/scale testing
#[cfg(test)]
pub mod tachyon_test;

use std::collections::VecDeque;

use rustc_hash::FxHashMap;

use self::network_address::NetworkAddress;
use self::nack::Nack;
use self::receive_result::RECEIVE_ERROR_CHANNEL;
use self::receive_result::RECEIVE_ERROR_UNKNOWN;
use self::receive_result::ReceiveResult;
use self::receive_result::TachyonReceiveResult;
use self::receiver::RECEIVE_WINDOW_SIZE_DEFAULT;
use self::tachyon_socket::*;
use self::header::*;
use self::channel::*;
use self::connection::*;
use self::fragmentation::*;
use self::unreliable_sender::UnreliableSender;



pub const SEND_ERROR_CHANNEL: u32 = 2;
pub const SEND_ERROR_SOCKET: u32 = 1;
pub const SEND_ERROR_FRAGMENT: u32 = 3;
pub const SEND_ERROR_UNKNOWN: u32 = 4;
pub const SEND_ERROR_LENGTH: u32 = 5;

pub static mut NONE_SEND_DATA: &'static mut [u8] = &mut [0; TACHYON_HEADER_SIZE];

#[derive(Clone, Copy)]
#[repr(C)]
#[derive(Default)]
#[derive(Debug)]
pub struct TachyonStats {
    pub channel_stats: ChannelStats,
    pub packets_dropped: u64,
    pub unreliable_sent: u64,
    pub unreliable_received: u64,
    pub code_time: u64
}

impl std::fmt::Display for TachyonStats {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f,"channel_stats:{0} packets_dropped:{1} unreliable_sent:{2} unreliable_received:{3} code_time:{4}\n",
         self.channel_stats, self.packets_dropped, self.unreliable_sent, self.unreliable_received, self.code_time)
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct TachyonConfig {
    pub drop_packet_chance: u64,
    pub drop_reliable_only: u8,
    pub receive_window_size: u16
}

impl  TachyonConfig {
    pub fn default() -> Self {
        let default = TachyonConfig {
            drop_packet_chance: 0,
            drop_reliable_only: 0,
            receive_window_size: RECEIVE_WINDOW_SIZE_DEFAULT
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
    pub header: Header
}

pub struct Tachyon {
    pub id: u16,
    pub socket: TachyonSocket,
    pub unreliable_sender: Option<UnreliableSender>,
    pub connections: FxHashMap<NetworkAddress, Connection>,
    pub channels: FxHashMap<(NetworkAddress,u8),Channel>,
    pub channel_config: FxHashMap<u8, bool>,
    pub config: TachyonConfig,
    pub nack_send_data: Vec<u8>,
    pub resend_sequences: Vec<u16>,
    pub stats: TachyonStats
}

impl Tachyon {
    
    pub fn create(config: TachyonConfig) -> Self {
        return Tachyon::create_with_id(config, 0);
    }

    pub fn create_with_id(config: TachyonConfig, id: u16) -> Self {
        let socket = TachyonSocket::create();
      
        let mut tachyon = Tachyon {
            id,
            connections: FxHashMap::default(),
            channels: FxHashMap::default(),
            channel_config: FxHashMap::default(),
            socket: socket,
            unreliable_sender: None,
            config,
            nack_send_data: vec![0;4096],
            resend_sequences: Vec::new(),
            stats: TachyonStats::default()
        };
        tachyon.channel_config.insert(1, true);
        tachyon.channel_config.insert(2, false);

        return tachyon;
    }

    fn create_none(sequence: u16, channel_id: u8) {
        let mut header = Header::default();
        header.message_type = MESSAGE_TYPE_NONE;
        header.sequence = sequence;
        header.channel = channel_id;
        header.write(unsafe {&mut NONE_SEND_DATA});
    }

    pub fn bind(&mut self, address: NetworkAddress) -> bool {
        match self.socket.bind_socket(address) {
            CreateConnectResult::Success => {
                self.unreliable_sender = self.create_unreliable_sender();
                return true;
            },
            CreateConnectResult::Error => {
                return false;
            },
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
            },
            CreateConnectResult::Error => {
                return false;
            },
        }
    }

    pub fn create_unreliable_sender(&self) -> Option<UnreliableSender> {
        let socket = self.socket.clone_socket();
        if !socket.is_some() {
            return None;
        }
        let sender = UnreliableSender {
            socket: socket
        };
        return Some(sender);
    }

    fn try_create_connection(&mut self, address: NetworkAddress) -> bool {
        if !self.connections.contains_key(&address) {
            let conn = Connection::create(address, self.id);
            self.connections.insert(address, conn);
            return true;
        } else {
            return false;
        }
    }

    pub fn get_channel(&mut self, address: NetworkAddress, channel_id: u8) -> Option<&mut Channel> {
        match self.channels.get_mut(&(address, channel_id)) {
            Some(channel) => {
                return Some(channel);
            },
            None => { return None;},
        }
    }

    pub fn create_configured_channels(&mut self, address: NetworkAddress) {
        for config in &self.channel_config {
            let channel_id = *config.0;
            let ordered = *config.1;
            match self.channels.get_mut(&(address, channel_id)) {
                Some(_) => {
                },
                None => {
                    let channel = Channel::create(channel_id, ordered, address, self.config.get_receive_window_size());
                    self.channels.insert((address, channel_id), channel);
                },
            }
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

    pub fn get_connections(&self, max: u16) -> Vec<Connection> {
        let mut list: Vec<Connection> = Vec::new();
        let result_count = std::cmp::min(self.connections.len(), max as usize);

        let mut count = 0;
    
        for (address, conn) in &self.connections {
            list.push(*conn);
            count += 1;
            if count >= result_count {
                break;
            }
        }
        return list;
    }

    pub fn update(&mut self) {
        for channel in self.channels.values_mut() {
            channel.frag.expire_groups();
            channel.receiver.publish();
            let nack_res = channel.receiver.write_nacks(&mut self.nack_send_data, TACHYON_HEADER_SIZE as u64);
            let nack_len = nack_res.0;
            let nacks_sent = nack_res.1;
            if nacks_sent > 0 && nack_len > 0 {
                let mut header = Header::default();
                header.message_type = MESSAGE_TYPE_RESEND;
                header.channel = channel.id;
                header.write(&mut self.nack_send_data);
                let send_len = nack_len as usize + TACHYON_HEADER_SIZE;
                self.socket.send_to(channel.address, &self.nack_send_data, send_len);

                channel.stats.nacks_sent +=nacks_sent;
            }
        }
    }

    fn receive_published_channel_id(&mut self, receive_buffer:  &mut[u8], address: NetworkAddress, channel_id: u8) -> u32 {
        match self.channels.get_mut(&(address,channel_id)) {
            Some(channel) => {
                let res = channel.receive_published(receive_buffer);
                return res.0;
            },
            None => {return 0;},
        }
    }

    fn receive_published_all_channels(&mut self, receive_buffer:  &mut[u8]) -> (u32, NetworkAddress) {
        for channel in self.channels.values_mut() {
            let res = channel.receive_published(receive_buffer);
            if res.0 > 0 {
                return res;
            }
        }
        return (0, NetworkAddress::default());
    }


    pub fn receive_loop(&mut self, receive_buffer:  &mut[u8]) -> TachyonReceiveResult {
        
        let mut result = TachyonReceiveResult::default();
        
        for _ in 0..100 {
            let receive_result = self.receive_from_socket(receive_buffer);
            match receive_result {
                ReceiveResult::Reliable {network_address: socket_addr, channel_id } => {
                    let published = self.receive_published_channel_id(receive_buffer, socket_addr, channel_id);
                    if published > 0 {
                        result.length = published;
                        result.address = socket_addr;
                        return result;
                    }
                },
                ReceiveResult::UnReliable { received_len ,network_address: socket_addr} => {
                    result.length = received_len as u32;
                    result.address = socket_addr;
                    return result;
                },
                ReceiveResult::Empty => {break;},
                ReceiveResult::Retry => {},
                ReceiveResult::Error => {result.error = RECEIVE_ERROR_UNKNOWN; return result;},
                ReceiveResult::ChannelError => {result.error = RECEIVE_ERROR_CHANNEL;return result;}
            }
        }
        
        let published = self.receive_published_all_channels(receive_buffer);
        if published.0 > 0 {
            result.length = published.0;
            result.address = published.1;
            return result;
        }
        

        return result;
    }

    
    fn receive_from_socket(&mut self, receive_buffer:  &mut[u8]) -> ReceiveResult {
        
        let address: NetworkAddress;
        let received_len: usize;
        
        let socket_result = self.socket.receive(receive_buffer, self.config.drop_packet_chance, self.config.drop_reliable_only == 1);
        match socket_result {
            SocketReceiveResult::Success { bytes_received, network_address } => {
                received_len = bytes_received;
                address = network_address;

                if self.socket.is_server {
                    if self.try_create_connection(address) {
                        self.create_configured_channels(address);
                    }
                }
            },
            SocketReceiveResult::Empty => {
                return ReceiveResult::Empty;
            },
            SocketReceiveResult::Error => {
                return ReceiveResult::Error;
            },
            SocketReceiveResult::Dropped => {
                self.stats.packets_dropped += 1;
                return ReceiveResult::Retry;
            },
        }
        
        let header = Header::read(receive_buffer);
       
        if header.message_type == MESSAGE_TYPE_UNRELIABLE {
            self.stats.unreliable_received += 1;
            return ReceiveResult::UnReliable {received_len: received_len, network_address: address};
        }
        
        let channel = match self.channels.get_mut(&(address, header.channel)) {
            Some(c) =>{c},        
            None => {
                return ReceiveResult::ChannelError;
            }
        };
        
        channel.stats.bytes_received += received_len as u64;


        if header.message_type == MESSAGE_TYPE_NONE {
            channel.stats.nones_received += 1;
            if channel.receiver.receive_packet(header.sequence, receive_buffer, received_len) {
                channel.stats.nones_accepted += 1;
            }
            return ReceiveResult::Retry;
        }

        if header.message_type == MESSAGE_TYPE_RESEND {
            self.resend_sequences.clear();
            Nack::read_varint(&mut self.resend_sequences, &receive_buffer[..], TACHYON_HEADER_SIZE as u64);
            for seq in &self.resend_sequences {
                channel.stats.nacks_received += 1;
                match channel.send_buffers.get_send_buffer(*seq) {
                    Some(send_buffer) => {
                        self.socket.send_to(address, &send_buffer.buffer, send_buffer.buffer.len());
                        channel.stats.resent += 1;
                    },
                    None => {
                        Tachyon::create_none(*seq,channel.id);
                        let _sent_len = self.socket.send_to(address, unsafe {&NONE_SEND_DATA}, TACHYON_HEADER_SIZE);
                        channel.stats.nones_sent += 1;
                    },
                }
            }
            
            return ReceiveResult::Retry;
        }
        
        if header.message_type == MESSAGE_TYPE_FRAGMENT {
            let received_frag_res = channel.frag.receive_fragment(receive_buffer, received_len);
            if received_frag_res.0 {
                let received = channel.receiver.receive_packet(header.sequence, receive_buffer, TACHYON_FRAGMENTED_HEADER_SIZE);
                if received {
                    channel.stats.fragments_received += 1;
                }
            }
            return ReceiveResult::Retry;
        }
        
        
        if header.message_type == MESSAGE_TYPE_RELIABLE {
            let received = channel.receiver.receive_packet(header.sequence, receive_buffer, received_len);
            if received {
                channel.stats.received += 1;
                return ReceiveResult::Reliable {network_address: address, channel_id: header.channel};
            }
            return ReceiveResult::Retry;
        }

        return ReceiveResult::Error;
    }


    pub fn send_unreliable(&mut self, address: NetworkAddress, data:  &mut [u8], length: usize) -> TachyonSendResult {

        match &self.unreliable_sender {
            Some(sender) => {
                let result = sender.send_unreliable(address, data, length);
                if result.error == 0 {
                    self.stats.unreliable_sent += 1;
                }
                return result;
            },
            None => {
                let mut result = TachyonSendResult::default();
                result.error = SEND_ERROR_UNKNOWN;
                return result;
            },
        }
    }

    
    
    pub fn send_reliable(&mut self, channel_id: u8, address: NetworkAddress, data:  &mut [u8], length: usize) -> TachyonSendResult {
        
        let message_type = MESSAGE_TYPE_RELIABLE;
        let mut result = TachyonSendResult::default();

        if length == 0 {
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
            Some(c) =>{c},        
            None => {
                result.error = SEND_ERROR_CHANNEL;
                return result;
            }
        };

        if Fragmentation::should_fragment(length) {
            let mut fragment_bytes_sent = 0;
            let frag_sequences = channel.frag.create_fragments(&mut channel.send_buffers, channel.id, data, length);
            if frag_sequences.len() == 0 {
                result.error = SEND_ERROR_FRAGMENT;
                return result;
            }
            
            for seq in frag_sequences {
                match channel.send_buffers.get_send_buffer(seq) {
                    Some(fragment) => {
                        let sent = self.socket.send_to(address, &fragment.buffer, fragment.buffer.len());
                        fragment_bytes_sent += sent;

                        channel.stats.bytes_sent += sent as u64;
                        channel.stats.fragments_sent += 1;
                    },
                    None => {
                        result.error = SEND_ERROR_FRAGMENT;
                        return result;
                    },
                }
            }

            result.header.message_type = MESSAGE_TYPE_FRAGMENT;
            result.sent_len = fragment_bytes_sent as u32;

            channel.stats.sent += 1;

            return result;
        }

        let send_buffer_len = length + TACHYON_HEADER_SIZE;
        match channel.send_buffers.create_send_buffer(send_buffer_len) {
            Some(send_buffer) => {
                let sequence = send_buffer.sequence;
                let buffer = &mut send_buffer.buffer;
                buffer[TACHYON_HEADER_SIZE..length + TACHYON_HEADER_SIZE].copy_from_slice(&data[0..length]);

                let mut header = Header::default();
                header.message_type = message_type;
                header.channel = channel.id;
                header.sequence = sequence;
                header.write(buffer);
                
                let sent_len = self.socket.send_to(address, &buffer, send_buffer_len);
                result.sent_len = sent_len as u32;
                result.header = header;

                channel.stats.bytes_sent += sent_len as u64;
                channel.stats.sent += 1;

                return result;
                
            },
            None => {
                result.error = SEND_ERROR_UNKNOWN;
                return result;
            },
        }
    }
}


#[cfg(test)]
mod tests {

   
    use serial_test::serial;

    use crate::tachyon::tachyon_test::TachyonTest;

    use super::*;
    
    
    
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
        let sent = test.client_send_unreliable(2);
        assert_eq!(0, sent.error);
        assert_eq!(2, sent.sent_len as usize);

        let res = test.server_receive();
        assert_eq!(2, res.length);
        assert_eq!(0, test.receive_buffer[0]);
        assert_eq!(4, test.receive_buffer[1]);
    }

   

}

