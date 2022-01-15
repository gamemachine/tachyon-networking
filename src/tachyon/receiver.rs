    use std::{collections::VecDeque};



    use super::{sequence::*, nack::Nack, sequence_buffer::SequenceBuffer};

    const RECEIVE_BUFFER_SIZE: u16 = 1024;
    const RECEIVE_WINDOW_SIZE_DEFAULT: u16 = 512;

    pub struct Receiver {
        pub is_ordered: bool,
        pub receive_window_size: u16,
        pub last_sequence: u16,
        pub current_sequence: u16,
        pub buffered: SequenceBuffer<Vec<u8>>,
        pub published: VecDeque<Vec<u8>>,
        pub received: SequenceBuffer<bool>,
        pub resend_list: Vec<u16>,
        pub nack_list: Vec<Nack>,
        pub skipped_sequences: u64
    }

    impl  Receiver {

        pub fn create(is_ordered: bool, receive_window_size: u16) -> Self {

            let buffered: SequenceBuffer<Vec<u8>> = SequenceBuffer {
                values: vec![None; RECEIVE_BUFFER_SIZE as usize],
                partition_by: RECEIVE_BUFFER_SIZE
            };

            let received: SequenceBuffer<bool> = SequenceBuffer {
                values: vec![None; RECEIVE_BUFFER_SIZE as usize],
                partition_by: RECEIVE_BUFFER_SIZE
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
                skipped_sequences: 0
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
        pub fn should_increment_current(current: u16, last: u16, receive_window_size: u16) -> bool {
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


        pub fn take_published(&mut self) -> Option<Vec<u8>> {
            match self.published.pop_front() {
                Some(value) => {
                    return Some(value);
                },
                None => {
                    return None;
                },
            }
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
            let mut buffer: Vec<u8> = vec![0;length];
            buffer[..].copy_from_slice(&data[0..length]);
            self.buffered.insert(sequence, buffer);
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
                self.received.take(last_sequence);
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
                        self.received.take(seq);
                    } else if step_sequence && Sequence::is_greater_then(seq, self.current_sequence) {
                        self.current_sequence = seq;
                        self.received.take(seq);
                    }
                    
                    if self.is_buffered(seq) {
                        let buffer = self.buffered.take(seq);
                        self.published.push_back(buffer.unwrap());
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

        pub fn write_nacks(&mut self, data: &mut [u8], position: u64) -> (u64,u64) {
            self.nack_list.clear();
            self.set_resend_list();
            Nack::create_nacks(&self.resend_list, &mut self.nack_list);
            let nack_len = Nack::write_varint(&self.nack_list, &mut data[..], position);
            return (nack_len,self.resend_list.len() as u64);
        }

    }
  

#[cfg(test)]
mod tests {

    use crate::tachyon::receiver::*;
    use crate::tachyon::sequence::*;

    #[test]
    fn test_skipped() {
        let mut channel = Receiver::default(true);
        let data: Vec<u8> = vec![0;1024];
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
        
        assert_eq!(65530,Receiver::calculate_current_in_window(65530,100));
        assert_eq!(0,Receiver::calculate_current_in_window(0,512));
        assert_eq!(10,Receiver::calculate_current_in_window(0,512 + 10));
        assert_eq!(1,Receiver::calculate_current_in_window(0,513));
        assert_eq!(0,Receiver::calculate_current_in_window(65533,512));
    }

    #[test]
    fn resend_list_creation() {
        let mut channel = Receiver::default(true);
        channel.current_sequence = 1;
        channel.last_sequence = 520;
        channel.set_resend_list();
        assert_eq!(512, channel.resend_list.len());
        assert_eq!(519, channel.resend_list[0]);
        assert_eq!(channel.last_sequence - channel.receive_window_size, channel.resend_list[511]);

    }

    #[test]
    fn wrapping_in_order() {
        let mut channel = Receiver::default(true);
        channel.current_sequence = 65533;
        let data: Vec<u8> = vec![0;1024];

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
        let data: Vec<u8> = vec![0;1024];
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
    fn resend_generation() {
        let mut channel = Receiver::default(true);
        let data: Vec<u8> = vec![0;1024];
        assert_eq!(0, channel.current_sequence);

        let receive_result = channel.receive_packet(5, &data[..], 32);
        channel.set_resend_list();
        assert_eq!(4, channel.resend_list.len());

        let receive_result = channel.receive_packet(4, &data[..], 32);
        channel.set_resend_list();
        assert_eq!(3, channel.resend_list.len());

        let receive_result = channel.receive_packet(3, &data[..], 32);
        channel.set_resend_list();
        assert_eq!(2, channel.resend_list.len());

        let receive_result = channel.receive_packet(2, &data[..], 32);
        assert_eq!(0, channel.current_sequence);
        assert_eq!(5, channel.last_sequence);

        channel.set_resend_list();
        assert_eq!(1, channel.resend_list.len());

        let receive_result = channel.receive_packet(1, &data[..], 32);
        assert_eq!(5, channel.current_sequence);
        assert_eq!(5, channel.last_sequence);
        channel.set_resend_list();
        assert_eq!(0, channel.resend_list.len());

    }


    #[test]
    fn full_wrap() {
        let mut channel = Receiver::default(true);
        let data: Vec<u8> = vec![0;1024];

        let mut sequence = 1;
        for _ in 1..200000 {
            let receive_result = channel.receive_packet(sequence, &data[..], 32);
            if channel.current_sequence != sequence {
                print!("{0} {1} {2}\n", sequence, channel.current_sequence, channel.last_sequence);
                panic!();
            }
            assert!(channel.take_published().is_some());
            // if channel.take_published().is_none() {
            //     print!("{0} {1} {2}\n", sequence, channel.current_sequence, channel.last_sequence);
            //     panic!();
            // }
            sequence = Sequence::next_sequence(sequence);
           
        }
        
    }

    #[test]
    fn publish_consume_publish() {
        let mut channel = Receiver::default(true);
        let data: Vec<u8> = vec![0;1024];
        let receive_result = channel.receive_packet(1, &data[..], 32);
        let receive_result = channel.receive_packet(2, &data[..], 32);
        assert!((channel.take_published().is_some()));
        assert!((channel.take_published().is_some()));
        assert!((channel.take_published().is_none()));

        let receive_result = channel.receive_packet(4, &data[..], 32);
        let receive_result = channel.receive_packet(3, &data[..], 32);
        assert!((channel.take_published().is_some()));
        assert!((channel.take_published().is_some()));
        assert!((channel.take_published().is_none()));

        let receive_result = channel.receive_packet(5, &data[..], 32);
        assert!((channel.take_published().is_some()));
        assert!((channel.take_published().is_none()));

        assert_eq!(0, channel.published.len());

    }

    #[test]
    fn receive_older_fails() {
        let mut channel = Receiver::default(true);
        let data: Vec<u8> = vec![0;1024];
        let receive_result = channel.receive_packet(1, &data[..], 32);
        assert!(receive_result);
        let receive_result = channel.receive_packet(1, &data[..], 32);
        assert!(!receive_result);
        let receive_result = channel.receive_packet(0, &data[..], 32);
        assert!(!receive_result);
    }

    
   

   

    #[test]
    fn ordered_flow_test() {
        let mut channel = Receiver::default(true);
        let data: Vec<u8> = vec![0;1024];
        let receive_result = channel.receive_packet(1, &data[..], 32);
        assert!(receive_result);
        assert_eq!(1, channel.published.len());
        
        let receive_result = channel.receive_packet(5, &data[..], 32);
        assert!(receive_result);
        assert_eq!(1, channel.published.len());
        assert_eq!(1, channel.current_sequence);
        assert_eq!(5, channel.last_sequence);
        channel.set_resend_list();
        assert_eq!(3, channel.resend_list.len());

        let receive_result = channel.receive_packet(3, &data[..], 32);
        assert!(receive_result);
        assert_eq!(1, channel.current_sequence);

        channel.set_resend_list();
        assert_eq!(2, channel.resend_list.len());

        let receive_result = channel.receive_packet(2, &data[..], 32);
        assert_eq!(3, channel.current_sequence);
        assert_eq!(3, channel.published.len());
        
        channel.set_resend_list();
        assert_eq!(1, channel.resend_list.len());
        

        let receive_result = channel.receive_packet(4, &data[..], 32);
        assert_eq!(5, channel.current_sequence);
        assert_eq!(5, channel.last_sequence);
        channel.set_resend_list();
        assert_eq!(0, channel.resend_list.len());

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
    fn unordered_flow_test() {
        let mut channel = Receiver::default(false);
        let data: Vec<u8> = vec![0;1024];
        let receive_result = channel.receive_packet(1, &data[..], 32);
        assert_eq!(1, channel.published.len());
        let receive_result = channel.receive_packet(5, &data[..], 32);
        assert_eq!(2, channel.published.len());
        assert_eq!(1, channel.current_sequence);
        assert_eq!(5, channel.last_sequence);
        channel.set_resend_list();
        assert_eq!(3, channel.resend_list.len());

        let receive_result = channel.receive_packet(3, &data[..], 32);
        assert_eq!(1, channel.current_sequence);

        channel.set_resend_list();
        assert_eq!(2, channel.resend_list.len());

        let receive_result = channel.receive_packet(2, &data[..], 32);
        assert_eq!(3, channel.current_sequence);
        assert_eq!(4, channel.published.len());
        
        channel.set_resend_list();
        assert_eq!(1, channel.resend_list.len());
        

        let receive_result = channel.receive_packet(4, &data[..], 32);
        assert_eq!(5, channel.current_sequence);
        assert_eq!(5, channel.last_sequence);
        channel.set_resend_list();
        assert_eq!(0, channel.resend_list.len());

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