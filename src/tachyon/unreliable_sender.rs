use std::{io, net::UdpSocket};

use super::{
    header::{Header, MESSAGE_TYPE_UNRELIABLE},
    network_address::NetworkAddress,
    TachyonSendResult, SEND_ERROR_CHANNEL, SEND_ERROR_LENGTH,
};

// this is created with a cloned UdpSocket which can then be used from another thread.
// this is purely a rust thing sockets are atomic at the OS level.
pub struct UnreliableSender {
    pub socket: Option<UdpSocket>,
    pub send_buffer: Vec<u8>
}

impl UnreliableSender {
    pub fn send_unreliable(&mut self, address: NetworkAddress, data: &mut [u8], body_len: usize) -> TachyonSendResult {
        let mut result = TachyonSendResult::default();
        
        if body_len < 1 {
            result.error = SEND_ERROR_LENGTH;
            return result;
        }

        if !self.socket.is_some() {
            result.error = SEND_ERROR_CHANNEL;
            return result;
        }

        // copy to send buffer at +1 offset for message_type
        self.send_buffer[1..body_len+1].copy_from_slice(&data[0..body_len]);
        let length = body_len + 1;

        let mut header = Header::default();
        header.message_type = MESSAGE_TYPE_UNRELIABLE;
        header.write_unreliable(&mut self.send_buffer);

        let sent_len = self.send_to(address, length);
        result.sent_len = sent_len as u32;
        result.header = header;

        return result;
    }

    fn send_to(&self, address: NetworkAddress, length: usize) -> usize {
        match &self.socket {
            Some(socket) => {
                let slice = &self.send_buffer[0..length];
                let socket_result: io::Result<usize>;

                if address.port == 0 {
                    socket_result = socket.send(slice);
                } else {
                    socket_result = socket.send_to(slice, address.to_socket_addr());
                }

                match socket_result {
                    Ok(size) => {
                        return size;
                    }
                    Err(_) => {
                        return 0;
                    }
                }
            }
            None => {
                return 0;
            }
        }
    }
}
