

use std::time::Instant;

use super::{fragmentation::Fragmentation, send_buffer_manager::SendBufferManager, receiver::Receiver, network_address::NetworkAddress, header::{TACHYON_HEADER_SIZE, MESSAGE_TYPE_NONE, TACHYON_FRAGMENTED_HEADER_SIZE, MESSAGE_TYPE_FRAGMENT, Header}, int_buffer::IntBuffer};

#[derive(Clone, Copy)]
#[repr(C)]
#[derive(Default)]
#[derive(Debug)]
pub struct ChannelStats {
    pub sent: u64,
    pub received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub fragments_sent: u64,
    pub fragments_received: u64,
    pub fragments_assembled: u64,
    pub published: u64,
    pub published_consumed: u64,
    pub nacks_sent: u64,
    pub nacks_received: u64,
    pub resent: u64,
    pub nones_sent: u64,
    pub nones_received: u64,
    pub nones_accepted: u64,
    pub skipped_sequences: u64
}

impl ChannelStats {
    pub fn add_from(&mut self, other: &ChannelStats) {
        self.sent += other.sent;
        self.received += other.received;
        self.bytes_sent += other.bytes_sent;
        self.bytes_received += other.bytes_received;
        self.fragments_sent += other.fragments_sent;
        self.fragments_received += other.fragments_received;
        self.fragments_assembled += other.fragments_assembled;
        self.published += other.published;
        self.published_consumed += other.published_consumed;
        self.nacks_sent += other.nacks_sent;
        self.nacks_received += other.nacks_received;
        self.resent += other.resent;
        self.nones_sent += other.nones_sent;
        self.nones_received += other.nones_received;
        self.nones_accepted += other.nones_accepted;
        self.skipped_sequences += other.skipped_sequences;
    }
}

impl std::fmt::Display for ChannelStats {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f,
"sent:{} received:{},kb_sent:{} kb_received:{}
fragments_sent:{} fragments_received:{} fragments_assembled:{},
published: {} published_consumed:{} nacks_sent:{} nacks_received:{} resent:{}
nones_sent:{} nones_received:{} nones_accepted:{} skipped_sequences:{}\n\n",
        self.sent, self.received, self.bytes_sent / 1024, self.bytes_received / 1024,
        self.fragments_sent, self.fragments_received, self.fragments_assembled,
        self.published, self.published_consumed, self.nacks_sent, self.nacks_received, self.resent,
        self.nones_sent, self.nones_received, self.nones_accepted, self.skipped_sequences)
    }
}

pub struct Channel {
    pub id: u8,
    pub address: NetworkAddress,
    pub frag: Fragmentation,
    pub send_buffers: SendBufferManager,
    pub receiver: Receiver,
    pub stats: ChannelStats
}

impl Channel {
    pub fn create(id: u8, ordered: bool, address: NetworkAddress, receive_window_size: u16) -> Self {
        let channel = Channel {
            id,
            address,
            frag: Fragmentation::default(),
            send_buffers: SendBufferManager::default(),
            receiver: Receiver::create(ordered, receive_window_size),
            stats: ChannelStats::default()
        };
        return channel;
    }

    pub fn is_ordered(&self) -> bool {
        return self.receiver.is_ordered;
    }

    pub fn update_stats(&mut self) {
        self.stats.skipped_sequences = self.receiver.skipped_sequences;
    }

    pub fn receive_published(&mut self, receive_buffer:  &mut[u8]) -> (u32, NetworkAddress) {
        match self.receiver.take_published() {
            Some(buffer) => {

                let buffer_len = buffer.len();

                // msg length of TACHYON_HEADER_SIZE should be MESSAGE_TYPE_NONE.
                // if this is a fragment we just orphaned the fragment group.  these will be expired in update()
                if buffer_len == TACHYON_HEADER_SIZE {
                    let mut reader = IntBuffer {index: 0 };
                    let message_type = reader.read_u8(&buffer);
                    if message_type == MESSAGE_TYPE_NONE {
                        return (0, NetworkAddress::default());
                    }
                }

                // could be a fragment, or just a tiny message
                if buffer_len == TACHYON_FRAGMENTED_HEADER_SIZE {
                    let header = Header::read_fragmented(&buffer);
                    if header.message_type == MESSAGE_TYPE_FRAGMENT {
                        match self.frag.assemble(header) {
                            Ok(res) => {
                                let assembled_len = res.len();
                                receive_buffer[0..assembled_len].copy_from_slice(&res[..]);
                                self.stats.received += 1;
                                self.stats.fragments_assembled += header.fragment_count as u64;
                                self.stats.published_consumed += 1;
                                return (assembled_len as u32, self.address);
                            },
                            Err(_) => {
                                return (0, self.address);
                            },
                        }
                    }
                }
                
                self.stats.published_consumed += 1;
                receive_buffer[0..buffer_len-TACHYON_HEADER_SIZE].copy_from_slice(&buffer[TACHYON_HEADER_SIZE..buffer_len]);
                return ((buffer_len - TACHYON_HEADER_SIZE) as u32, self.address);
            },
            None => {},
        }
        return (0, NetworkAddress::default());
    }
}
