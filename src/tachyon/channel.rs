

use super::{fragmentation::Fragmentation, send_buffer_manager::SendBufferManager, receiver::Receiver, network_address::NetworkAddress};

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
}
