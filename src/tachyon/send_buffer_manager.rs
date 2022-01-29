use std::time::Instant;

use super::{sequence::Sequence, sequence_buffer::SequenceBuffer, byte_buffer_pool::{ByteBuffer, ByteBufferPool, BYTE_BUFFER_SIZE_DEFAULT}};

const SEND_BUFFER_SIZE: u16 = 1024;
const EXPIRE: u128 = 5000;

pub struct SendBuffer {
    pub sequence: u16,
    pub byte_buffer: ByteBuffer,
    pub created_at: Instant,
}
pub struct SendBufferManager {
    pub current_sequence: u16,
    pub buffers: SequenceBuffer<SendBuffer>,
    pub buffer_pool: ByteBufferPool
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
            buffer_pool: ByteBufferPool::create(BYTE_BUFFER_SIZE_DEFAULT,SEND_BUFFER_SIZE as usize)
        };
        return sender;
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

    pub fn create_send_buffer_old(&mut self, length: usize) -> Option<&mut SendBuffer> {

        self.current_sequence = Sequence::next_sequence(self.current_sequence);

        let byte_buffer = ByteBuffer::create(length);
       
        let buffer = SendBuffer {
            sequence: self.current_sequence,
            byte_buffer,
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

    pub fn create_send_buffer(&mut self, length: usize) -> Option<&mut SendBuffer> {
        self.current_sequence = Sequence::next_sequence(self.current_sequence);

        if let Some(mut send_buffer) = self.buffers.take(self.current_sequence) {
            if send_buffer.byte_buffer.pooled && length <= self.buffer_pool.buffer_size {
                send_buffer.byte_buffer.length = length;
            } else {
                self.buffer_pool.return_buffer(send_buffer.byte_buffer);
                send_buffer.byte_buffer = self.buffer_pool.get_buffer(length);
            }
            send_buffer.sequence = self.current_sequence;
            send_buffer.created_at = Instant::now();
            return self.buffers.insert(self.current_sequence, send_buffer);
        }

        let byte_buffer = self.buffer_pool.get_buffer(length);
        let send_buffer = SendBuffer {
            sequence: self.current_sequence,
            byte_buffer,
            created_at: Instant::now(),
        };
        return self.buffers.insert(self.current_sequence, send_buffer);
        
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};


    use crate::tachyon::byte_buffer_pool::BYTE_BUFFER_SIZE_DEFAULT;

    use super::SendBufferManager;

    #[test]
    fn test_create_buffer() {
        let mut manager = SendBufferManager::default();
        let buffer = manager.create_send_buffer(BYTE_BUFFER_SIZE_DEFAULT).unwrap();
        assert!(buffer.byte_buffer.pooled);

        let buffer = manager.create_send_buffer(BYTE_BUFFER_SIZE_DEFAULT + 1).unwrap();
        assert!(!buffer.byte_buffer.pooled);
    }

    #[test]
    fn test_reused_buffer() {
        let mut manager = SendBufferManager::default();
        manager.current_sequence = 10;
        let buffer = manager.create_send_buffer(BYTE_BUFFER_SIZE_DEFAULT).unwrap();
        assert_eq!(11, buffer.sequence);
        assert_eq!(0, buffer.byte_buffer.version);

        // should get back same byte buffer with same version and new length
        manager.current_sequence = 10;
        let buffer = manager.create_send_buffer(BYTE_BUFFER_SIZE_DEFAULT - 1).unwrap();
        assert_eq!(11, buffer.sequence);
        assert_eq!(0, buffer.byte_buffer.version);
        assert!(buffer.byte_buffer.pooled);
        assert_eq!(BYTE_BUFFER_SIZE_DEFAULT - 1, buffer.byte_buffer.length);

        // should get new byte buffer
        manager.current_sequence = 10;
        let buffer = manager.create_send_buffer(BYTE_BUFFER_SIZE_DEFAULT + 10).unwrap();
        assert_eq!(11, buffer.sequence);
        assert_eq!(0, buffer.byte_buffer.version);
        assert!(!buffer.byte_buffer.pooled);
        assert_eq!(BYTE_BUFFER_SIZE_DEFAULT + 10, buffer.byte_buffer.length);
    }

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
