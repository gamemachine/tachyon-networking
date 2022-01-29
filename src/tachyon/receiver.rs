
use std::collections::VecDeque;

use super::{nack::Nack, sequence::*, sequence_buffer::SequenceBuffer, channel::RECEIVE_WINDOW_SIZE_DEFAULT, byte_buffer_pool::{ByteBuffer, ByteBufferPool}};

const RECEIVE_BUFFER_SIZE: u16 = 1024;


pub struct Receiver {
    pub is_ordered: bool,
    pub receive_window_size: u32,
    pub last_sequence: u16,
    pub current_sequence: u16,
    pub buffered: SequenceBuffer<ByteBuffer>,
    pub published: VecDeque<ByteBuffer>,
    pub received: SequenceBuffer<bool>,
    pub resend_list: Vec<u16>,
    pub nack_list: Vec<Nack>,
    pub nack_queue: VecDeque<Nack>,
    pub skipped_sequences: u64,
    pub buffer_pool: ByteBufferPool
}

impl Receiver {
    pub fn create(is_ordered: bool, receive_window_size: u32) -> Self {
        let mut buffered: SequenceBuffer<ByteBuffer> = SequenceBuffer {
            values: Vec::new(),
            partition_by: RECEIVE_BUFFER_SIZE,
        };
        for i in 0..RECEIVE_BUFFER_SIZE {
            buffered.values.insert(i as usize, None);
        }

        let received: SequenceBuffer<bool> = SequenceBuffer {
            values: vec![None; RECEIVE_BUFFER_SIZE as usize],
            partition_by: RECEIVE_BUFFER_SIZE,
        };

        let receiver = Receiver {
            is_ordered,
            receive_window_size,
            last_sequence: 0,
            current_sequence: 0,
            buffered,
            published: VecDeque::new(),
            received,
            resend_list: Vec::new(),
            nack_list: Vec::new(),
            skipped_sequences: 0,
            nack_queue: VecDeque::new(),
            buffer_pool: ByteBufferPool::default()
        };

        return receiver;
    }

    pub fn default(is_ordered: bool) -> Self {
        return Receiver::create(is_ordered, RECEIVE_WINDOW_SIZE_DEFAULT);
    }

    pub fn calculate_current_in_window(current: u16, last: u16) -> u16 {
        if current == last {
            return current;
        }

        let mut start: i32 = (last as i32 - RECEIVE_WINDOW_SIZE_DEFAULT as i32) as i32;
        if start < 0 {
            start = std::u16::MAX as i32 + start;
        }

        if Sequence::is_greater_then(start as u16, current) {
            return start as u16;
        } else {
            return current;
        }
    }
    pub fn should_increment_current(current: u16, last: u16, receive_window_size: u32) -> bool {
        if current == last {
            return false;
        }

        let mut start: i32 = (last as i32 - receive_window_size as i32) as i32;
        if start < 0 {
            start = std::u16::MAX as i32 + start;
        }

        if Sequence::is_greater_then(start as u16, current) {
            return true;
        } else {
            return false;
        }
    }

    pub fn return_buffer(&mut self, byte_buffer: ByteBuffer) {
        self.buffer_pool.return_buffer(byte_buffer);
    }
    
    pub fn take_published(&mut self) -> Option<ByteBuffer> {
        return self.published.pop_front();
    }

    fn is_buffered(&self, sequence: u16) -> bool {
        return self.buffered.is_some(sequence);
    }

    pub fn is_received(&self, sequence: u16) -> bool {
        return self.received.is_some(sequence);
    }

    fn set_received(&mut self, sequence: u16) {
        self.received.insert(sequence, true);
    }

    fn set_buffered(&mut self, sequence: u16, data: &[u8], length: usize) {
        let mut byte_buffer = self.buffer_pool.get_buffer(length);
        byte_buffer.get_mut()[0..length].copy_from_slice(&data[0..length]);
        self.buffered.insert(sequence, byte_buffer);
    }

    // Note:  we use current sequence increments to mark previous as not received.
    // This forces current to only ever increment by 1.  Ie we can't just adjust our window forward
    // in big steps for example or we would leave a bunch of entries < current still marked as received.

    pub fn receive_packet(&mut self, sequence: u16, data: &[u8], length: usize) -> bool {
        // if the difference between current/last is greater then the window, increment current.
        if Receiver::should_increment_current(self.current_sequence, self.last_sequence, self.receive_window_size) {
            self.received.take(self.current_sequence);
            self.current_sequence = Sequence::next_sequence(self.current_sequence);
            self.skipped_sequences += 1;
        }

        if !Sequence::is_greater_then(sequence, self.current_sequence) {
            return false;
        }

        if Sequence::is_greater_then(sequence, self.last_sequence) {
            self.last_sequence = sequence;
        }

        let next = Sequence::next_sequence(self.current_sequence);
        if sequence == next {
            let last_sequence = self.current_sequence;
            self.current_sequence = sequence;
            self.received.remove(last_sequence);
        }

        // resends can be higher then current and already received.
        if self.is_received(sequence) {
            return false;
        } else {
            self.set_buffered(sequence, data, length);
            self.set_received(sequence);
        }

        self.publish();

        return true;
    }

    pub fn publish(&mut self) {
        // walk from current to last and move buffered into published
        // increment current sequence until we hit a missing sequence.
        // on missing, ordered channel breaks out it's done.
        // unordered channel keep moving buffered to published

        let start = self.current_sequence;
        let end = Sequence::next_sequence(self.last_sequence);
        let mut step_sequence = true;
        let mut seq = start;

        for _ in 0..self.receive_window_size {
            if self.is_received(seq) {
                if self.current_sequence == seq {
                    self.received.remove(seq);
                } else if step_sequence && Sequence::is_greater_then(seq, self.current_sequence) {
                    self.current_sequence = seq;
                    self.received.remove(seq);
                }

                if self.is_buffered(seq) {
                    match self.buffered.take(seq) {
                        Some(byte_buffer) => {
                            self.published.push_back(byte_buffer);
                        }
                        None => {}
                    }
                }
            } else {
                if self.is_ordered {
                    break;
                } else {
                    step_sequence = false;
                }
            }
            seq = Sequence::next_sequence(seq);
            if seq == end {
                break;
            }
        }
    }

    pub fn set_resend_list(&mut self) {
        self.resend_list.clear();

        if self.current_sequence == self.last_sequence {
            return;
        }

        let start = Sequence::previous_sequence(self.last_sequence);
        let end = self.current_sequence;

        let mut seq = start;

        for _ in 0..self.receive_window_size {
            if !self.is_received(seq) {
                self.resend_list.push(seq);
            }

            seq = Sequence::previous_sequence(seq);
            if seq == end {
                break;
            }
        }
    }

    pub fn create_nacks(&mut self) -> u32 {
        self.nack_list.clear();
        self.nack_queue.clear();

        let mut nacked_count = 0;
        let mut seq = Sequence::previous_sequence(self.last_sequence);
        if Sequence::is_equal_to_or_less_than(seq, self.current_sequence) {
            return nacked_count;
        }
     
        let count = self.receive_window_size / 32;

        for _ in 0..count {

            if Sequence::is_equal_to_or_less_than(seq, self.current_sequence) {
                return nacked_count;
            }

            if self.is_received(seq) {
                seq = Sequence::previous_sequence(seq);
                if Sequence::is_equal_to_or_less_than(seq, self.current_sequence) {
                    return nacked_count;
                }
                continue;
            }

            let mut current = Nack::default();
            current.start_sequence = seq;
            nacked_count += 1;
            current.nacked_count = nacked_count;

            for i in 0..32 {
                seq = Sequence::previous_sequence(seq);

                if Sequence::is_equal_to_or_less_than(seq, self.current_sequence) {
                    self.nack_list.push(current);
                    self.nack_queue.push_back(current);
                    return nacked_count;
                }
    
                if !self.is_received(seq) {
                    current.set_bits(i, true);
                    nacked_count += 1;
                    current.nacked_count = nacked_count;
                }
            }
            self.nack_list.push(current);
            self.nack_queue.push_back(current);

            seq = Sequence::previous_sequence(seq);
            
        }
        return nacked_count;
    }

}

#[cfg(test)]
mod tests {

    use crate::tachyon::{receiver::*};

    pub fn is_nacked(receiver: &Receiver, sequence: u16) -> bool {
        for nack in &receiver.nack_list {
            if nack.is_nacked(sequence) {
                return true;
            }
        }
        return false;
    }

    fn assert_nack(receiver: &mut Receiver, sequence: u16) {
        if receiver.is_received(sequence) || sequence >= receiver.last_sequence || sequence <= receiver.current_sequence {
            if is_nacked(receiver, sequence) {
                panic!("{0} is nacked", sequence);
            } else {
                //println!("{0} not nacked", sequence);
            }
        } else {
            if !is_nacked(receiver,sequence) {
                panic!("{0} not nacked", sequence);
            } else {
                //println!("{0} nacked", sequence);
            }
        }
    }

    #[test]
    fn test_all_nacked() {
        let mut channel = Receiver::default(true);
        channel.current_sequence = 0;
        channel.last_sequence = 512;
        

        let nack_count = channel.create_nacks();
        assert_eq!(16, channel.nack_list.len());
        assert_eq!(511, nack_count);

        for i in 0..512 {
            assert_nack(&mut channel, i);
        }
    }

    #[test]
    fn test_some_nacked() {
        let mut channel = Receiver::default(true);
        channel.current_sequence = 0;
        channel.last_sequence = 64;
        
        channel.set_received(63);
        channel.set_received(63 - 32);
        channel.set_received(63 - 33);
        channel.set_received(1);
        let nacked_count = channel.create_nacks();

        assert_eq!(2, channel.nack_list.len());
        assert_eq!(63 - 4, nacked_count);

        for i in 0..66 {
            assert_nack(&mut channel,i);
            
        }
    }

    #[test]
    fn test_skipped() {
        let mut channel = Receiver::default(true);
        let data: Vec<u8> = vec![0; 1024];
        channel.current_sequence = 0;
        channel.last_sequence = 512 + 10;

        // should skip and take received
        channel.set_received(0);
        assert!(!channel.receive_packet(1, &data[..], 32));
        assert!(!channel.is_received(0));
        assert_eq!(1, channel.current_sequence);

        assert!(!channel.receive_packet(1, &data[..], 32));
        assert_eq!(2, channel.current_sequence);
    }

    #[test]
    fn test_reset_receive_window() {
        assert_eq!(65530, Receiver::calculate_current_in_window(65530, 100));
        assert_eq!(0, Receiver::calculate_current_in_window(0, 512));
        assert_eq!(10, Receiver::calculate_current_in_window(0, 512 + 10));
        assert_eq!(1, Receiver::calculate_current_in_window(0, 513));
        assert_eq!(0, Receiver::calculate_current_in_window(65533, 512));
    }

    #[test]
    fn wrapping_in_order() {
        let mut channel = Receiver::default(true);
        channel.current_sequence = 65533;
        let data: Vec<u8> = vec![0; 1024];

        let receive_result = channel.receive_packet(65534, &data[..], 32);
        assert!(receive_result);

        assert_eq!(65534, channel.current_sequence);
        assert_eq!(1, channel.published.len());

        let receive_result = channel.receive_packet(0, &data[..], 32);
        assert!(receive_result);
        assert_eq!(0, channel.current_sequence);
        assert_eq!(0, channel.last_sequence);
        assert!((channel.take_published().is_some()));

        let receive_result = channel.receive_packet(1, &data[..], 32);
        assert!(receive_result);
        assert_eq!(1, channel.current_sequence);
        assert!((channel.take_published().is_some()));

        let receive_result = channel.receive_packet(2, &data[..], 32);
        assert!(receive_result);
        assert_eq!(2, channel.last_sequence);
        assert_eq!(2, channel.current_sequence);
        assert!((channel.take_published().is_some()));
    }

    #[test]
    fn wrapping_out_of_order() {
        let mut channel = Receiver::default(true);
        channel.current_sequence = 65533;
        let data: Vec<u8> = vec![0; 1024];
        let receive_result = channel.receive_packet(65534, &data[..], 32);
        assert!(receive_result);
        assert_eq!(65534, channel.current_sequence);
        let receive_result = channel.receive_packet(2, &data[..], 32);
        assert!(receive_result);
        assert_eq!(65534, channel.current_sequence);
        assert_eq!(2, channel.last_sequence);
        let receive_result = channel.receive_packet(1, &data[..], 32);
        assert!(receive_result);
        assert_eq!(65534, channel.current_sequence);

        let receive_result = channel.receive_packet(0, &data[..], 32);
        assert!(receive_result);
        assert_eq!(2, channel.last_sequence);
        assert_eq!(2, channel.current_sequence);
    }

  
    #[test]
    fn full_wrap() {
        let mut channel = Receiver::default(true);
        let data: Vec<u8> = vec![0; 1024];

        let mut sequence = 1;
        for _ in 1..200000 {
            let _receive_result = channel.receive_packet(sequence, &data[..], 32);
            if channel.current_sequence != sequence {
                print!(
                    "{0} {1} {2}\n",
                    sequence, channel.current_sequence, channel.last_sequence
                );
                panic!();
            }
            assert!(channel.take_published().is_some());
            
            sequence = Sequence::next_sequence(sequence);
        }
    }

    #[test]
    fn publish_consume_publish() {
        let mut channel = Receiver::default(true);
        let data: Vec<u8> = vec![0; 1024];
        let _receive_result = channel.receive_packet(1, &data[..], 32);
        let _receive_result = channel.receive_packet(2, &data[..], 32);
        assert!((channel.take_published().is_some()));
        assert!((channel.take_published().is_some()));
        assert!((channel.take_published().is_none()));

        let _receive_result = channel.receive_packet(4, &data[..], 32);
        let _receive_result = channel.receive_packet(3, &data[..], 32);
        assert!((channel.take_published().is_some()));
        assert!((channel.take_published().is_some()));
        assert!((channel.take_published().is_none()));

        let _receive_result = channel.receive_packet(5, &data[..], 32);
        assert!((channel.take_published().is_some()));
        assert!((channel.take_published().is_none()));

        assert_eq!(0, channel.published.len());
    }

    #[test]
    fn receive_older_fails() {
        let mut channel = Receiver::default(true);
        let data: Vec<u8> = vec![0; 1024];
        let receive_result = channel.receive_packet(1, &data[..], 32);
        assert!(receive_result);
        let receive_result = channel.receive_packet(1, &data[..], 32);
        assert!(!receive_result);
        let receive_result = channel.receive_packet(0, &data[..], 32);
        assert!(!receive_result);
    }

    #[test]
    #[allow(dead_code)]
    fn ordered_flow_test() {
        let mut channel = Receiver::default(true);
        let data: Vec<u8> = vec![0; 1024];
        let receive_result = channel.receive_packet(1, &data[..], 32);
        assert!(receive_result);
        assert_eq!(1, channel.published.len());

        let receive_result = channel.receive_packet(5, &data[..], 32);
        assert!(receive_result);
        assert_eq!(1, channel.published.len());
        assert_eq!(1, channel.current_sequence);
        assert_eq!(5, channel.last_sequence);
       

        let receive_result = channel.receive_packet(3, &data[..], 32);
        assert!(receive_result);
        assert_eq!(1, channel.current_sequence);

        let _receive_result = channel.receive_packet(2, &data[..], 32);
        assert_eq!(3, channel.current_sequence);
        assert_eq!(3, channel.published.len());

        let _receive_result = channel.receive_packet(4, &data[..], 32);
        assert_eq!(5, channel.current_sequence);
        assert_eq!(5, channel.last_sequence);
       

        assert_eq!(5, channel.published.len());

        assert!(channel.take_published().is_some());
        assert!(channel.take_published().is_some());
        assert!(channel.take_published().is_some());
        assert!(channel.take_published().is_some());
        assert!(channel.take_published().is_some());

        assert!(channel.take_published().is_none());
        assert_eq!(0, channel.published.len());
    }

    #[test]
    #[allow(dead_code)]
    fn unordered_flow_test() {
        let mut channel = Receiver::default(false);
        let data: Vec<u8> = vec![0; 1024];
        let _receive_result = channel.receive_packet(1, &data[..], 32);
        assert_eq!(1, channel.published.len());
        let _receive_result = channel.receive_packet(5, &data[..], 32);
        assert_eq!(2, channel.published.len());
        assert_eq!(1, channel.current_sequence);
        assert_eq!(5, channel.last_sequence);
       

        let _receive_result = channel.receive_packet(3, &data[..], 32);
        assert_eq!(1, channel.current_sequence);


        let _receive_result = channel.receive_packet(2, &data[..], 32);
        assert_eq!(3, channel.current_sequence);
        assert_eq!(4, channel.published.len());


        let _receive_result = channel.receive_packet(4, &data[..], 32);
        assert_eq!(5, channel.current_sequence);
        assert_eq!(5, channel.last_sequence);
       
        assert_eq!(5, channel.published.len());

        assert!(channel.take_published().is_some());
        assert!(channel.take_published().is_some());
        assert!(channel.take_published().is_some());
        assert!(channel.take_published().is_some());
        assert!(channel.take_published().is_some());

        assert!(channel.take_published().is_none());
        assert_eq!(0, channel.published.len());
    }
}
