use std::time::Instant;

use super::{sequence::Sequence, sequence_buffer::SequenceBuffer};

pub const SEND_BUFFER_SIZE: u16 = 1024;
const EXPIRE: u128 = 5000;

pub struct SendBuffer {
    pub sequence: u16,
    pub buffer: Vec<u8>,
    pub created_at: Instant,
}
pub struct SendBufferManager {
    pub current_sequence: u16,
    pub buffers: SequenceBuffer<SendBuffer>,
}

impl SendBufferManager {
    pub fn default() -> Self {
        let mut buffers: SequenceBuffer<SendBuffer> = SequenceBuffer {
            values: Vec::new(),
            partition_by: SEND_BUFFER_SIZE,
        };
        for _ in 0..SEND_BUFFER_SIZE {
            buffers.values.push(None);
        }

        let sender = SendBufferManager {
            current_sequence: 0,
            buffers,
        };
        return sender;
    }

    pub fn ceil_pow2(value: i32) -> i32 {
        let mut i = value;
        i -= 1;
        i |= i >> 1;
        i |= i >> 2;
        i |= i >> 4;
        i |= i >> 8;
        i |= i >> 16;
        return i + 1;
    }

    pub fn get_send_buffer(&mut self, sequence: u16) -> Option<&mut SendBuffer> {
        match self.buffers.get_mut(sequence) {
            Some(send_buffer) => {
                return Some(send_buffer);
            }
            None => {
                return None;
            }
        }
    }

    pub fn expire(&mut self) {
        let mut expired: Vec<u16> = Vec::new();

        for value in &self.buffers.values {
            if let Some(buffer) = value {
                if buffer.created_at.elapsed().as_millis() > EXPIRE {
                    expired.push(buffer.sequence);
                }
            }
        }
        for sequence in expired {
            self.buffers.remove(sequence);
        }
    }

    pub fn create_send_buffer(&mut self, length: usize) -> Option<&mut SendBuffer> {
        self.current_sequence = Sequence::next_sequence(self.current_sequence);

        let data = vec![0; length as usize];
        let buffer = SendBuffer {
            sequence: self.current_sequence,
            buffer: data,
            created_at: Instant::now(),
        };

        self.buffers.insert(self.current_sequence, buffer);

        match self.buffers.get_mut(self.current_sequence) {
            Some(buffer) => {
                return Some(buffer);
            }
            None => {
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::SendBufferManager;

    #[test]
    fn test_expire() {
        let mut buffers = SendBufferManager::default();
        let buffer = buffers.create_send_buffer(32);
        let buffer = buffer.unwrap();
        let sequence = buffer.sequence;
        let now = Instant::now() - Duration::new(6, 0);
        buffer.created_at = now;

        assert!(buffers.buffers.is_some(sequence));
        buffers.expire();
        assert!(!buffers.buffers.is_some(sequence));
    }
}
