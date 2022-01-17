
use super::{sequence::Sequence, sequence_buffer::SequenceBuffer};

pub const SEND_BUFFER_SIZE: u16 = 1024;

pub struct SendBuffer {
    pub sequence: u16,
    pub buffer: Vec<u8>
}
pub struct SendBufferManager {
    pub current_sequence: u16,
    pub buffers: SequenceBuffer<SendBuffer>
}

impl  SendBufferManager {
    pub fn default() -> Self {

        let mut buffers: SequenceBuffer<SendBuffer> = SequenceBuffer {
            values: Vec::new(),
            partition_by: SEND_BUFFER_SIZE
        };
        for _ in 0..SEND_BUFFER_SIZE {
            buffers.values.push(None);
        }
        
        let sender = SendBufferManager {
            current_sequence: 0,
            buffers
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

    pub fn get_send_buffer(&mut self, sequence: u16)-> Option<&mut SendBuffer> {
        match self.buffers.get_mut(sequence) {
            Some(send_buffer) => {
                return Some(send_buffer);
            },
            None => {
                return None;
            },
        }
    }

    pub fn create_send_buffer(&mut self, length: usize) -> Option<&mut SendBuffer> {
        self.current_sequence = Sequence::next_sequence(self.current_sequence);

        let data = vec![0;length as usize];
        let buffer = SendBuffer {
            sequence: self.current_sequence,
            buffer: data
        };

        self.buffers.insert(self.current_sequence, buffer);
        
        match self.buffers.get_mut(self.current_sequence) {
            Some(buffer) => {
                return Some(buffer);
            },
            None => {
                return None;
            },
        }
    }

}

#[cfg(test)]
mod tests {
    
}