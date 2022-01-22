use std::io::Cursor;

use varuint::{ReadVarint, WriteVarint};

use super::{int_buffer::IntBuffer, sequence::Sequence};

#[derive(Clone, Copy)]
#[repr(C)]
#[derive(Default)]
pub struct Nack {
    pub start_sequence: u16,
    pub flags: u32,
    pub nacked_count: u32,
    pub sent_count: u32
}

impl Nack {
    pub fn write(nacks: &[Nack], data: &mut [u8], position: u64) -> u64 {
        if nacks.len() == 0 {
            return 0;
        }
        let mut buffer = IntBuffer {
            index: position as usize,
        };
        buffer.write_u8(nacks.len() as u8, data);
        for nack in nacks {
            buffer.write_u16(nack.start_sequence, data);
            buffer.write_u32(nack.flags, data);
        }
        return buffer.index as u64;
    }

    pub fn read_single(sequences: &mut Vec<u16>, data: &[u8], position: usize) {
        let mut buffer = IntBuffer {
            index: position
        };

        let mut nack = Nack::default();
        nack.start_sequence = buffer.read_u16(data);
        if nack.start_sequence == 0 {
            return;
        }
        
        nack.flags = buffer.read_u32(data);
        nack.get_nacked(sequences);
    }

    pub fn write_single(nack: &Nack, data: &mut [u8], position: usize) -> usize {
        let mut buffer = IntBuffer {
            index: position
        };

        buffer.write_u16(nack.start_sequence, data);
        buffer.write_u32(nack.flags, data);
        return buffer.index;
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

    pub fn read_varint(sequences: &mut Vec<u16>, data: &[u8], position: usize) {
        let mut cursor = Cursor::new(data);
        cursor.set_position(position as u64);

        let count = ReadVarint::<u32>::read_varint(&mut cursor).unwrap();
        for _ in 0..count {
            let mut nack = Nack::default();
            nack.start_sequence = ReadVarint::<u16>::read_varint(&mut cursor).unwrap();
            nack.flags = ReadVarint::<u32>::read_varint(&mut cursor).unwrap();
            nack.get_nacked(sequences);
        }
    }

    pub fn read(sequences: &mut Vec<u16>, data: &[u8], position: u64) {
        let mut buffer = IntBuffer {
            index: position as usize,
        };
        let count = buffer.read_u8(data);
        for _ in 0..count {
            let mut nack = Nack::default();
            nack.start_sequence = buffer.read_u16(data);
            nack.flags = buffer.read_u32(data);
            nack.get_nacked(sequences);
        }
    }


    pub fn get_nacked(&self, sequences: &mut Vec<u16>) {
        sequences.push(self.start_sequence);
        self.get_flagged(sequences);
    }

    pub fn get_flagged(&self, sequences: &mut Vec<u16>) {
        let mut seq = Sequence::previous_sequence(self.start_sequence);
        for i in 0i32..32 {
            if self.get_bits(i) {
                sequences.push(seq);
            }
            seq = Sequence::previous_sequence(seq);
        }
    }

    pub fn is_nacked(&self, sequence: u16) -> bool {
        if self.start_sequence == sequence {
            return true;
        }
        return self.is_flagged(sequence);
    }

    pub fn is_flagged(&self, sequence: u16) -> bool {
        let mut seq = Sequence::previous_sequence(self.start_sequence);
        for i in 0i32..32 {
            if seq == sequence {
                return self.get_bits(i);
            }
            seq = Sequence::previous_sequence(seq);
        }
        return false;
    }

    pub fn set_flagged(&mut self, sequence: u16) {
        let mut seq = Sequence::previous_sequence(self.start_sequence);
        for i in 0i32..32 {
            if seq == sequence {
                self.set_bits(i, true);
                return;
            }
            seq = Sequence::previous_sequence(seq);
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

    #[test]
    fn test_flagged() {
        let mut nack = Nack::default();
        nack.start_sequence = 33;
        nack.set_flagged(32);
        assert!(nack.is_flagged(32));
        assert!(nack.get_bits(0));

        nack.set_flagged(1);
        assert!(nack.get_bits(31));
        assert!(nack.is_flagged(1));

        nack.set_flagged(34);
        assert!(!nack.is_flagged(34));
    }

    #[test]
    fn test_flagged_wrapped() {
        let mut nack = Nack::default();
        nack.start_sequence = 0;
        nack.set_flagged(65534);
        assert!(nack.is_flagged(65534));

        nack.set_flagged(65534 - 31);
        assert!(nack.is_flagged(65534 - 31));

        nack.set_flagged(2);
        assert!(!nack.is_flagged(2));
    }

    fn create_full_nack(start: u16) -> Nack {
        let mut nack = Nack::default();
        nack.start_sequence = start;
        let mut seq = Sequence::previous_sequence(nack.start_sequence);
        for _ in 0..32 {
            nack.set_flagged(seq);
            seq = Sequence::previous_sequence(seq);
        }
        return nack;
    }

    #[test]
    fn test_get_nacked() {
        let mut nack = create_full_nack(1);

        let mut sequences: Vec<u16> = Vec::new();
        nack.get_nacked(&mut sequences);
        assert_eq!(33, sequences.len());

        nack.set_bits(4, false);
        nack.set_bits(31, false);

        let mut sequences: Vec<u16> = Vec::new();
        nack.get_nacked(&mut sequences);
        assert_eq!(31, sequences.len());
    }

    #[test]
    fn test_write_read() {
        let mut data: Vec<u8> = vec![0; 1024];
        let mut sequences_out: Vec<u16> = Vec::new();
        let mut nacks: Vec<Nack> = Vec::new();
        nacks.push(create_full_nack(1));
        nacks.push(create_full_nack(34));

        Nack::write_varint(&nacks, &mut data[..], 0);
        Nack::read_varint(&mut sequences_out, &data[..], 0);
        assert_eq!(66, sequences_out.len());
    }
}
