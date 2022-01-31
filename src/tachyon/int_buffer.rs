use std::ops::Range;

use super::network_address::NetworkAddress;

pub struct IntBuffer {
    pub index: usize,
}

impl IntBuffer {

    pub fn write_address(&mut self, address: NetworkAddress, data: &mut [u8]) {
        self.write_u16(address.a, data);
        self.write_u16(address.b, data);
        self.write_u16(address.c, data);
        self.write_u16(address.d, data);
        self.write_u32(address.port, data);
    }

    pub fn read_address(&mut self, data: &[u8]) -> NetworkAddress {
        let mut address = NetworkAddress::default();
        address.a = self.read_u16(data);
        address.b = self.read_u16(data);
        address.c = self.read_u16(data);
        address.d = self.read_u16(data);
        address.port = self.read_u32(data);
        return address;
    }

    pub fn write_u32(&mut self, v: u32, data: &mut [u8]) {
        data[self.index] = v as u8;
        self.index += 1;
        data[self.index] = (v >> 8) as u8;
        self.index += 1;
        data[self.index] = (v >> 16) as u8;
        self.index += 1;
        data[self.index] = (v >> 24) as u8;
        self.index += 1;
    }

    pub fn read_u32(&mut self, data: &[u8]) -> u32 {
        let value = (data[self.index] as u32)
            | (data[self.index + 1] as u32) << 8
            | (data[self.index + 2] as u32) << 16
            | (data[self.index + 3] as u32) << 24;
        self.index += 4;
        return value;
    }

    pub fn write_u16(&mut self, v: u16, data: &mut [u8]) {
        data[self.index] = v as u8;
        self.index += 1;
        data[self.index] = (v >> 8) as u8;
        self.index += 1;
    }

    pub fn read_u16(&mut self, data: &[u8]) -> u16 {
        let value = (data[self.index] as u16) | (data[self.index + 1] as u16) << 8;
        self.index += 2;
        return value;
    }

    pub fn write_u8(&mut self, v: u8, data: &mut [u8]) {
        data[self.index] = v;
        self.index += 1;
    }

    pub fn read_u8(&mut self, data: &[u8]) -> u8 {
        let value = data[self.index];
        self.index += 1;
        return value;
    }

    pub fn u4_to_u8(v1: u8, v2: u8) -> u8 {
        return v1 | v2 << 4;
    }

    pub fn u8_to_u4(byte: u8) -> (u8,u8) {
        return (byte & 0x0F,  byte >> 4);
    }
}

pub struct LengthPrefixed {
    pub reader: IntBuffer,
    pub writer: IntBuffer
}

impl LengthPrefixed {
    pub fn default() -> Self {
        return LengthPrefixed {
            reader: IntBuffer {index: 0},
            writer: IntBuffer {index: 0}
        }
    }

    pub fn write(&mut self, channel: u16, src_address: NetworkAddress, src: &[u8], dst: &mut [u8]) {
        self.writer.write_u32(src.len() as u32, dst);
        self.writer.write_u16(channel, dst);
        self.writer.write_address(src_address, dst);
        dst[self.writer.index..self.writer.index+src.len()].copy_from_slice(&src);
        self.writer.index += src.len();
    }

    pub fn read(&mut self, data: &[u8]) -> (u16,NetworkAddress,Range<usize>) {
        let len = self.reader.read_u32(&data) as usize;
        let channel = self.reader.read_u16(&data);
        let address = self.reader.read_address(&data);
        let range = self.reader.index..self.reader.index+len;
        self.reader.index += len;
        return (channel, address,range);
    }
}


#[cfg(test)]
mod tests {
    use crate::tachyon::int_buffer::IntBuffer;

    #[test]
    fn test_4bit() {
        let mut low = 3;
        let mut high = 15;
        let byte = IntBuffer::u4_to_u8(low, high);
        let res = IntBuffer::u8_to_u4(byte);
        low = res.0;
        high = res.1;

        println!("{0}", byte);
        println!("{0} {1}", low, high);
    }

    #[test]
    fn test_header_readwrite() {
        let mut bytes: Vec<u8> = vec![0; 128];

        let mut buffer = IntBuffer { index: 0 };
        buffer.write_u32(234, &mut bytes);
        buffer.write_u16(44, &mut bytes);
        buffer.write_u8(99, &mut bytes);
        buffer.write_u32(1, &mut bytes);
        buffer.index = 0;
        assert_eq!(234, buffer.read_u32(&bytes));
        assert_eq!(44, buffer.read_u16(&bytes));
        assert_eq!(99, buffer.read_u8(&bytes));
        assert_eq!(1, buffer.read_u32(&bytes));
        return;
    }
}
