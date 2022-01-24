use std::time::Instant;

use super::header::*;
use super::send_buffer_manager::*;
use super::sequence::*;
use rustc_hash::FxHashMap;

const GROUP_EXPIRE: u128 = 5000;
const FRAG_SIZE: usize = 1200;
pub struct Fragmentation {
    pub next_group: u16,
    pub received: FxHashMap<u16, FxHashMap<u16, Vec<u8>>>,
    pub received_at: FxHashMap<u16, Instant>,
}

impl Fragmentation {
    pub fn default() -> Self {
        let default = Fragmentation {
            next_group: 1,
            received: FxHashMap::default(),
            received_at: FxHashMap::default(),
        };
        return default;
    }

    pub fn expire_groups(&mut self) {
        let mut expired: Vec<u16> = Vec::new();
        for (group, time) in &self.received_at {
            if time.elapsed().as_millis() > GROUP_EXPIRE {
                self.received.remove(group);
                expired.push(*group);
            }
        }
        for group in expired {
            self.received_at.remove(&group);
        }
    }

    pub fn should_fragment(length: usize) -> bool {
        return length >= FRAG_SIZE;
    }

    fn get_next_group(&mut self) -> u16 {
        self.next_group += 1;
        if self.next_group >= std::u16::MAX - 1 {
            self.next_group = 1;
        }
        return self.next_group;
    }

    fn get_group_length(map: &FxHashMap<u16, Vec<u8>>) -> usize {
        let mut length = 0;
        for (_, value) in map {
            length += value.len() - TACHYON_FRAGMENTED_HEADER_SIZE;
        }
        return length;
    }

    pub fn assemble(&mut self, header: Header) -> Result<Vec<u8>, ()> {
        let map = match self.received.get_mut(&header.fragment_group) {
            Some(v) => v,
            None => {
                return Err(());
            }
        };
        if map.len() != header.fragment_count as usize {
            return Err(());
        }

        let body_length = Fragmentation::get_group_length(map);

        let mut buffer: Vec<u8> = vec![0; body_length];
        let mut offset = 0;

        let mut seq = header.fragment_start_sequence;
        for _ in 0..header.fragment_count {
            match map.get_mut(&seq) {
                Some(fragment) => {
                    let frag_body_len = fragment.len() - TACHYON_FRAGMENTED_HEADER_SIZE;
                    let src = &fragment[TACHYON_FRAGMENTED_HEADER_SIZE..fragment.len()];
                    let dest = &mut buffer[offset..offset + frag_body_len];
                    dest.copy_from_slice(src);

                    offset += frag_body_len;
                }
                None => {
                    self.received.remove(&header.fragment_group);
                    return Err(());
                }
            }
            seq = Sequence::next_sequence(seq);
        }

        self.received.remove(&header.fragment_group);
        return Ok(buffer);
    }

    pub fn receive_fragment(&mut self, data: &[u8], length: usize) -> (bool, bool) {
        let header = Header::read_fragmented(data);
        if !self.received.contains_key(&header.fragment_group) {
            self.received.insert(header.fragment_group, FxHashMap::default());
            self.received_at.insert(header.fragment_group, Instant::now());
        }
        if let Some(map) = self.received.get_mut(&header.fragment_group) {
            let slice = &data[0..length as usize];

            let mut fragment: Vec<u8> = vec![0; length];
            fragment[..].copy_from_slice(slice);
            if !map.contains_key(&header.sequence) {
                map.insert(header.sequence, fragment);
            }

            return (true, map.len() == header.fragment_count as usize);
        }

        return (false, false);
    }

    pub fn create_fragments(&mut self, sender: &mut SendBufferManager, channel: u8, data: &[u8], length: usize) -> Vec<u16> {
        let slice = &data[0..length];

        let chunks = slice.chunks(FRAG_SIZE as usize);
        let fragment_count = chunks.len() as u16;
        let mut fragments: Vec<u16> = Vec::new();
        let group = self.get_next_group();

        let mut start_sequence = 0;
        let mut index = 0;

        for chunk in chunks {
            let chunk_len = chunk.len();
            let fragment_len = chunk_len + TACHYON_FRAGMENTED_HEADER_SIZE;

            match sender.create_send_buffer(fragment_len) {
                Some(send_buffer) => {
                    let sequence = send_buffer.sequence;
                    if index == 0 {
                        start_sequence = sequence;
                    }

                    let fragment_header = Header::create_fragmented(sequence, channel, group, start_sequence, fragment_count);
                    fragment_header.write_fragmented(&mut send_buffer.byte_buffer.get_mut());

                    send_buffer.byte_buffer.get_mut()[TACHYON_FRAGMENTED_HEADER_SIZE..chunk_len + TACHYON_FRAGMENTED_HEADER_SIZE].copy_from_slice(chunk);
                    fragments.push(sequence);

                    index += 1;
                }
                None => {
                    fragments.clear();
                    return fragments;
                }
            }
        }

        return fragments;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::tachyon::fragmentation::*;

    #[test]
    fn test_expire() {
        let mut frag = Fragmentation::default();
        frag.received_at.insert(1, Instant::now());
        frag.expire_groups();
        assert!(frag.received_at.contains_key(&1));
        assert_eq!(1, frag.received_at.len());

        let now = Instant::now() - Duration::new(6, 0);
        frag.received_at.insert(2, now);
        frag.expire_groups();
        assert!(!frag.received_at.contains_key(&2));
        assert_eq!(1, frag.received_at.len());
    }

    #[test]
    fn test_create() {
        let mut frag = Fragmentation::default();
        let mut sender = SendBufferManager::default();

        let data: Vec<u8> = vec![0; 1400];
        assert_eq!(1400, data.len());

        let result = frag.create_fragments(&mut sender, 1, &data[..], data.len());
        assert_eq!(2, result.len());
        let buffer = sender.get_send_buffer(result[0]).unwrap();
        assert_eq!(1210, buffer.byte_buffer.length);
        let header = Header::read_fragmented(&buffer.byte_buffer.get());
        assert_eq!(MESSAGE_TYPE_FRAGMENT, header.message_type);
        assert_eq!(1, header.sequence);
        assert_eq!(1, header.fragment_start_sequence);
        assert_eq!(2, header.fragment_count);

        let buffer = sender.get_send_buffer(result[1]).unwrap();
        assert_eq!(210, buffer.byte_buffer.length);

        let header = Header::read_fragmented(&buffer.byte_buffer.get());
        assert_eq!(MESSAGE_TYPE_FRAGMENT, header.message_type);
        assert_eq!(2, header.sequence);
    }

    #[test]
    fn test_receive() {
        let mut frag = Fragmentation::default();
        let mut sender = SendBufferManager::default();

        let data: Vec<u8> = vec![3; 2500];
        let created = frag.create_fragments(&mut sender, 1, &data[..], data.len());
        let send_buffer = sender.get_send_buffer(created[0]).unwrap();
        let complete = frag.receive_fragment(&send_buffer.byte_buffer.get(), send_buffer.byte_buffer.length);
        assert!(!complete.1);

        let send_buffer = sender.get_send_buffer(created[1]).unwrap();
        let complete = frag.receive_fragment(&send_buffer.byte_buffer.get(), send_buffer.byte_buffer.length);
        assert!(!complete.1);

        let send_buffer = sender.get_send_buffer(created[2]).unwrap();
        let complete = frag.receive_fragment(&send_buffer.byte_buffer.get(), send_buffer.byte_buffer.length);
        assert!(complete.1);

        let header = Header::read_fragmented(&send_buffer.byte_buffer.get());
        let assembled = frag.assemble(header);
        assert!(assembled.is_ok());
        let assembled_data = assembled.unwrap();
        assert_eq!(2500, assembled_data.len());

        for i in 0..assembled_data.len() {
            assert_eq!(3, assembled_data[i]);
        }
    }
}
