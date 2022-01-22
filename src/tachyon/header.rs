use super::int_buffer::IntBuffer;

pub const MESSAGE_TYPE_UNRELIABLE: u8 = 0;
pub const MESSAGE_TYPE_RELIABLE: u8 = 1;
pub const MESSAGE_TYPE_FRAGMENT: u8 = 2;
pub const MESSAGE_TYPE_NONE: u8 = 3;
pub const MESSAGE_TYPE_NACK: u8 = 4;
pub const MESSAGE_TYPE_RELIABLE_WITH_NACK: u8 = 5;

pub const MESSAGE_TYPE_LINK_IDENTITY: u8 = 6;
pub const MESSAGE_TYPE_UNLINK_IDENTITY: u8 = 7;

pub const MESSAGE_TYPE_IDENTITY_LINKED: u8 = 8;
pub const MESSAGE_TYPE_IDENTITY_UNLINKED: u8 = 9;

pub const TACHYON_HEADER_SIZE: usize = 4;
pub const TACHYON_NACKED_HEADER_SIZE: usize = 10;
pub const TACHYON_FRAGMENTED_HEADER_SIZE: usize = 10;

#[derive(Clone, Copy)]
#[repr(C)]
#[derive(Default)]
pub struct ConnectionHeader {
    pub message_type: u8,
    pub id: u32,
    pub session_id: u32,
}

impl ConnectionHeader {
    pub fn read(buffer: &[u8]) -> Self {
        let mut header = ConnectionHeader::default();
        let mut reader = IntBuffer { index: 0 };

        header.message_type = reader.read_u8(buffer);
        header.id = reader.read_u32(buffer);
        header.session_id = reader.read_u32(buffer);

        return header;
    }

    pub fn write(&self, buffer: &mut [u8]) {
        let mut writer = IntBuffer { index: 0 };

        writer.write_u8(self.message_type as u8, buffer);
        writer.write_u32(self.id, buffer);
        writer.write_u32(self.session_id, buffer);
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
#[derive(Default)]
pub struct Header {
    pub message_type: u8,
    pub channel: u8,
    pub sequence: u16,

    // fragment - optional
    pub fragment_group: u16,
    pub fragment_start_sequence: u16,
    pub fragment_count: u16,

    // nacked - optional
    pub start_sequence: u16,
    pub flags: u32
}

impl Header {
   
    pub fn write_unreliable(&self, buffer: &mut [u8]) {
        let mut writer = IntBuffer { index: 0 };

        writer.write_u8(self.message_type as u8, buffer);
    }

    pub fn write_nacked(&self, buffer: &mut [u8]) {
        let mut writer = IntBuffer { index: 0 };

        writer.write_u8(self.message_type, buffer);
        writer.write_u8(self.channel, buffer);
        writer.write_u16(self.sequence, buffer);

        writer.write_u16(self.start_sequence, buffer);
        writer.write_u32(self.flags, buffer);
    }

    pub fn write(&self, buffer: &mut [u8]) {
        let mut writer = IntBuffer { index: 0 };

        writer.write_u8(self.message_type, buffer);
        writer.write_u8(self.channel, buffer);
        writer.write_u16(self.sequence, buffer);
    }
  
    pub fn read(buffer: &[u8]) -> Self {
        let mut header = Header::default();
        let mut reader = IntBuffer { index: 0 };

        header.message_type = reader.read_u8(buffer);
        header.channel = reader.read_u8(buffer);
        header.sequence = reader.read_u16(buffer);

        return header;
    }

    // fragmented
    pub fn write_fragmented(&self, buffer: &mut [u8]) {
        let mut writer = IntBuffer { index: 0 };

        writer.write_u8(self.message_type, buffer);
        writer.write_u8(self.channel, buffer);
        writer.write_u16(self.sequence, buffer);

        writer.write_u16(self.fragment_group, buffer);
        writer.write_u16(self.fragment_start_sequence, buffer);
        writer.write_u16(self.fragment_count, buffer);
    }

    pub fn read_fragmented(buffer: &[u8]) -> Self {
        let mut header = Header::default();
        let mut reader = IntBuffer { index: 0 };

        header.message_type = reader.read_u8(buffer);
        header.channel = reader.read_u8(buffer);
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
        header.fragment_start_sequence = start;
        header.fragment_count = count;
        return header;
    }
}
