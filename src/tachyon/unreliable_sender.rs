use std::{net::UdpSocket, io};

use super::{network_address::NetworkAddress, TachyonSendResult, SEND_ERROR_LENGTH, SEND_ERROR_CHANNEL, header::{MESSAGE_TYPE_UNRELIABLE, Header}};

// this is created with a cloned UdpSocket which can then be used from another thread.
// this is purely a rust thing sockets are atomic at the OS level.
pub struct UnreliableSender {
    pub socket: Option<UdpSocket>
}

impl UnreliableSender {
    pub fn send_unreliable(&self, address: NetworkAddress, data:  &mut [u8], length: usize) -> TachyonSendResult {
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

    fn send_to(&self,address: NetworkAddress, data:  &[u8], length: usize) -> usize {
        match &self.socket {
            Some(socket) => {
                let slice = &data[0..length];
                let socket_result: io::Result<usize>;
                
                if address.port == 0 {
                    socket_result = socket.send(slice);
                } else {
                    socket_result = socket.send_to(slice, address.to_socket_addr());
                }
                
                match socket_result  {
                    Ok(size) => {
                        return size;
                    },
                    Err(_) => {
                        return 0;
                    },
                }
            },
            None => {
                return 0;
            }
        }
    }

}