use std::{io, net::UdpSocket};


use super::{
    header::{Header, MESSAGE_TYPE_UNRELIABLE},
    network_address::NetworkAddress,
    TachyonSendResult, SEND_ERROR_CHANNEL, SEND_ERROR_LENGTH,
};


// this is created with a cloned UdpSocket which can then be used from another thread.
pub struct UnreliableSender {
    pub socket: Option<UdpSocket>
}

impl UnreliableSender {
    pub fn create(socket: Option<UdpSocket>) -> Self {
        UnreliableSender {
            socket
        }
    }

    pub fn send(&self, address: NetworkAddress, data: &mut [u8], length: usize) -> TachyonSendResult {
        let mut result = TachyonSendResult::default();
        
        if length < 1 {
            result.error = SEND_ERROR_LENGTH;
            return result;
        }

        if !self.socket.is_some() {
            result.error = SEND_ERROR_CHANNEL;
            return result;
        }

        let mut header = Header::default();
        header.message_type = MESSAGE_TYPE_UNRELIABLE;
        header.write_unreliable(data);

        let sent_len = self.send_to(address, data, length);
        result.sent_len = sent_len as u32;
        result.header = header;

        return result;
    }

    fn send_to(&self, address: NetworkAddress, data: &[u8], length: usize) -> usize {
        match &self.socket {
            Some(socket) => {
                let slice = &data[0..length];
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
