pub struct IntBuffer {
    pub index: usize
}

impl  IntBuffer {
    pub fn read_u32(&mut self, data: &[u8]) -> u32 {
        let value = (data[self.index] as u32)
            | (data[self.index + 1] as u32) <<8
            | (data[self.index + 2] as u32) <<16
            | (data[self.index + 3] as u32) <<24;
            self.index += 4;
            return value;
    }

    pub fn read_u16(&mut self, data: &[u8]) -> u16 {
        let value = (data[self.index] as u16)
            | (data[self.index + 1] as u16) <<8;
            self.index += 2;
            return value;
    }

    pub fn read_u8(&mut self, data: &[u8]) -> u8 {
        let value = data[self.index];
        self.index += 1;
        return value;
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

    pub fn write_u16(&mut self, v: u16, data: &mut [u8]) {
        data[self.index] = v as u8;
        self.index += 1;
        data[self.index] = (v >> 8) as u8;
        self.index += 1;
        
    }

    pub fn write_u8(&mut self, v: u8, data: &mut [u8]) {
        data[self.index] = v;
        self.index += 1;
    }

}


#[cfg(test)]
mod tests {
    use crate::tachyon::int_buffer::IntBuffer;



    #[test]
    fn test_header_readwrite() {
        let mut bytes: Vec<u8> = vec![0;128];

        let mut buffer = IntBuffer {
            index: 0
        };
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