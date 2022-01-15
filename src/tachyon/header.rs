use super::int_buffer::IntBuffer;


pub const MESSAGE_TYPE_UNRELIABLE: u8 = 0;
pub const MESSAGE_TYPE_RELIABLE: u8 = 1;
pub const MESSAGE_TYPE_FRAGMENT: u8 = 2;
pub const MESSAGE_TYPE_NONE: u8 = 3;
pub const MESSAGE_TYPE_RESEND: u8 = 4;

pub const TACHYON_HEADER_SIZE: usize = 4;
pub const TACHYON_FRAGMENTED_HEADER_SIZE: usize = 10;

#[derive(Clone, Copy)]
#[repr(C)]
#[derive(Default)]
pub struct Header {
    pub message_type: u8,
    pub sequence: u16,
    pub channel: u8,

    pub fragment_group: u16,
    pub fragment_start_sequence: u16,
    pub fragment_count: u16,
}

impl Header {
    
    pub fn write_non_fragmented(buffer: &mut [u8], fragmented: Header) {
        let mut writer = IntBuffer {index: 0};
        
        writer.write_u8(fragmented.message_type as u8, buffer);
        writer.write_u8(fragmented.channel as u8, buffer);
        writer.write_u16(fragmented.sequence, buffer);
        
    }

    pub fn write_unreliable(&self, buffer: &mut [u8]) {
        let mut writer = IntBuffer {index: 0};
        
        writer.write_u8(self.message_type as u8, buffer);
    }

    pub fn write(&self, buffer: &mut [u8]) {
        let mut writer = IntBuffer {index: 0};
        
        writer.write_u8(self.message_type as u8, buffer);
        writer.write_u8(self.channel as u8, buffer);
        writer.write_u16(self.sequence, buffer);
    }

    pub fn read_raw(buffer: *const u8) -> Self {
        let slice = unsafe {std::slice::from_raw_parts_mut(buffer as *mut u8, TACHYON_HEADER_SIZE)};
        return Header::read(slice);
    }

    pub fn read(buffer: &[u8]) -> Self {
        let mut header = Header::default();
        let mut reader = IntBuffer {index: 0};
        
        header.message_type = reader.read_u8(buffer);
        header.channel =reader.read_u8(buffer);
        header.sequence = reader.read_u16(buffer);
        

        return header;
    }

    // fragmented
    pub fn write_fragmented(&self, buffer: &mut [u8]) {
        let mut writer = IntBuffer {index: 0};
        
        writer.write_u8(self.message_type as u8, buffer);
        writer.write_u8(self.channel as u8, buffer);
        writer.write_u16(self.sequence, buffer);
        

        writer.write_u16(self.fragment_group, buffer);
        writer.write_u16(self.fragment_start_sequence, buffer);
        writer.write_u16(self.fragment_count, buffer);
    }

    pub fn read_fragmented(buffer: &[u8]) -> Self {
        let mut header = Header::default();
        let mut reader = IntBuffer {index: 0};
        
        header.message_type = reader.read_u8(buffer);
        header.channel =reader.read_u8(buffer);
        header.sequence = reader.read_u16(buffer);
        

        header.fragment_group = reader.read_u16(buffer);
        header.fragment_start_sequence = reader.read_u16(buffer);
        header.fragment_count = reader.read_u16(buffer);

        return header;
    }

    pub fn create_fragmented(sequence: u16, channel: u8, group: u16, start: u16, count: u16) -> Self {
        let mut header = Header::default();
        header.message_type = MESSAGE_TYPE_FRAGMENT;
        header.sequence = sequence;
        header.channel = channel;

        header.fragment_group = group;
        header.fragment_start_sequence =start;
        header.fragment_count = count;
        return header;
    }

}
