use std::io::Cursor;

use varuint::{WriteVarint, ReadVarint};

use super::int_buffer::IntBuffer;



#[derive(Clone, Copy)]
#[repr(C)]
#[derive(Default)]
pub struct Nack {
    pub start_sequence: u16,
    pub flags : u32
}

impl  Nack {

    pub fn write(nacks: &[Nack], data: &mut [u8], position: u64) -> u64 {
        if nacks.len() == 0 {
            return 0;
        }
        let mut buffer = IntBuffer {index: position as usize};
        buffer.write_u8(nacks.len() as u8, data);
        for nack in nacks {
            buffer.write_u16(nack.start_sequence, data);
            buffer.write_u32(nack.flags, data);
        }
        return buffer.index as u64;
    }

    pub fn write_varint(nacks: &[Nack], data: &mut [u8], position: u64) -> u64 {
        if nacks.len() == 0 {
            return 0;
        }

        let mut cursor = Cursor::new(data);
        cursor.set_position(position);
        let _ = cursor.write_varint(nacks.len() as u16).unwrap();
        for nack in nacks {
            cursor.write_varint(nack.start_sequence).unwrap();
            cursor.write_varint(nack.flags).unwrap();
        }
        return cursor.position();
    }

    pub fn read_varint(sequences: &mut Vec<u16>, data: &[u8], position: u64) {
        let mut cursor = Cursor::new(data);
        cursor.set_position(position);

        let count = ReadVarint::<u32>::read_varint(&mut cursor).unwrap();
        for _ in 0..count {
            let mut nack = Nack::default();
            nack.start_sequence = ReadVarint::<u16>::read_varint(&mut cursor).unwrap();
            nack.flags = ReadVarint::<u32>::read_varint(&mut cursor).unwrap();
            nack.get_nacked(sequences);
        }
    }

    pub fn read(sequences: &mut Vec<u16>, data: &[u8], position: u64) {
        let mut buffer = IntBuffer {index: position as usize};
        let count = buffer.read_u8(data);
        for _ in 0..count {
            let mut nack = Nack::default();
            nack.start_sequence = buffer.read_u16(data);
            nack.flags = buffer.read_u32(data);
            nack.get_nacked(sequences);
        }
    }

    pub fn create_nacks(sequences: &[u16], nacks: &mut Vec<Nack>) {
        let mut current = Nack::default();

        let mut index = 0;
        for seq in sequences {
            if index == 0 {
                current.start_sequence = *seq;
            }
            
            if index > 0 {
                current.set_flagged(*seq);
            }
            
            index += 1;
            if index == 32 {
                index = 0;
                nacks.push(current);
                current = Nack::default();
            }
        }
        nacks.push(current);
    }

    
    pub fn get_nacked(&self, sequences: &mut Vec<u16>) {
        sequences.push(self.start_sequence);
        self.get_flagged(sequences);
    }

    pub fn get_flagged(&self, sequences: &mut Vec<u16>) {
        for i in 0i32..32 {
            let mut index: i32 = self.start_sequence as i32 - i;
            if index < 0 {
                index = std::u16::MAX as i32 + index;
            }
            if self.get_bits(i) {
                sequences.push(index as u16);
            }
        }
    }

    pub fn is_nacked(&self, sequence: u16) -> bool {
        if self.start_sequence == sequence {
            return true;
        }
        return self.is_flagged(sequence);
    }

    pub fn is_flagged(&self, sequence: u16) -> bool {
        
        for i in 0i32..32 {
            let mut index: i32 = self.start_sequence as i32 - i;
            if index < 0 {
                index = std::u16::MAX as i32 + index;
            }
            if index == sequence as i32 {
                return self.get_bits(i);
            }
        }
        return false;
    }

    pub fn set_flagged(&mut self, sequence: u16) {
        for i in 0i32..32 {
            let mut index: i32 = self.start_sequence as i32 - i;
            if index < 0 {
                index = std::u16::MAX as i32 + index;
            }
            if index == sequence as i32 {
                self.set_bits(i, true);
            }
        }
    }

    pub fn get_bits(&self, index: i32) -> bool {
        let mask = 1 << index;
        return (self.flags & mask) == mask;
    }

    pub fn set_bits(&mut self, index: i32, value: bool) {
        let mask = 1 << index;
        if value {
            self.flags |= mask;
        } else {
            self.flags &= !mask;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tachyon::sequence::Sequence;

    use super::Nack;

    pub fn verify_nacks(nacks: &[Nack], sequences: &[u16]) -> i32 {
        for seq in sequences {
            let mut found = false;
            for nack in nacks {
                if nack.start_sequence == *seq || nack.is_flagged(*seq) {
                    found = true;
                    //print!("{0}\n", *seq);
                    break;
                }
            }
            if !found {
                return *seq as i32;
            }
        }

        return -1;
    }

    #[test]
    fn test_create_nacks() {
        let mut sequences: Vec<u16> = Vec::new();

        let mut seq = 65500;
        for i in 0..1000 {
            if i % 2 == 0 {
                continue;
            }
            sequences.push(seq);
            seq = Sequence::next_sequence(seq);
        }

        sequences.reverse();

        let mut data:Vec<u8> = vec![0;1024];
        let mut sequences_out: Vec<u16> = Vec::new();
        let mut nacks: Vec<Nack> = Vec::new();
        
        Nack::create_nacks(&sequences, &mut nacks);
        Nack::write_varint(&nacks, &mut data[..], 0);
        Nack::read_varint(&mut sequences_out, &data[..], 0);

        assert_eq!(sequences.len(),sequences_out.len());
        assert_eq!(-1, verify_nacks(&nacks, &sequences));
        assert_eq!(-1, verify_nacks(&nacks, &sequences_out));
        print!("count {0}\n", sequences.len());
    }

    #[test]
    fn test_ack_bits() {
       let mut nack = Nack::default();
       nack.start_sequence = 1;
       nack.set_flagged(65534);
       nack.set_flagged(65530);
       nack.set_flagged(0);
       nack.set_flagged(4);

       assert!(nack.is_nacked(1));
       assert!(nack.is_nacked(65534));
       assert!(nack.is_nacked(65530));
       assert!(nack.is_nacked(0));
       assert!(!nack.is_nacked(65535));
       assert!(!nack.is_nacked(4));

       let mut sequences: Vec<u16> = Vec::new();
       nack.get_nacked(&mut sequences);
       assert_eq!(4, sequences.len());

    }

}