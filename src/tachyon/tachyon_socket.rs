use std::{net::{UdpSocket, SocketAddrV4, Ipv4Addr}, io, time::Instant};

use rand::Rng;
use rand::prelude::ThreadRng;
use socket2::{Socket, Domain, Type};

use super::{network_address::NetworkAddress, SocketTimeCounters, int_buffer::IntBuffer, header::MESSAGE_TYPE_RELIABLE};


pub enum CreateConnectResult {
    Success,
    Error
}

pub enum SocketReceiveResult {
    Success {bytes_received: usize, network_address: NetworkAddress},
    Empty,
    Error,
    Dropped
    
}
pub struct TachyonSocket {
    pub address: NetworkAddress,
    pub is_server: bool,
    pub socket: Option<UdpSocket>,
    pub rng: ThreadRng
}

impl  TachyonSocket {

    pub fn create() -> Self {
        let socket = TachyonSocket {
            address: NetworkAddress::default(),
            is_server: false,
            socket: None,
            rng: rand::thread_rng()
        };
        return socket;
    }

    pub fn bind_socket(&mut self, naddress: NetworkAddress) -> CreateConnectResult {
        if self.socket.is_some() {
            return CreateConnectResult::Error;
        }

        let address = naddress.to_socket_addr();
        self.address = naddress;

        let socket = Socket::new(Domain::IPV4, Type::DGRAM, None).unwrap();
        match socket.bind(&address.into()) {
            Ok(()) => {
                socket.set_recv_buffer_size(8192 * 256).unwrap();
                socket.set_nonblocking(true).unwrap();
                self.socket = Some(socket.into());
                self.is_server = true;

                return CreateConnectResult::Success;
            },
            Err(_) => {
                return CreateConnectResult::Error;
            },
        }
    }

    pub fn connect_socket(&mut self, naddress: NetworkAddress) -> CreateConnectResult {
        if self.socket.is_some() {
            return CreateConnectResult::Error;
        }
        
        self.address = NetworkAddress::default();
        let sock_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, None).unwrap();

        match socket.bind(&sock_addr.into()) {
            Ok(()) =>  {
                socket.set_recv_buffer_size(8192 * 256).unwrap();
                socket.set_nonblocking(true).unwrap();
                let address = naddress.to_socket_addr();
                let udp_socket: UdpSocket = socket.into();

                match udp_socket.connect(&address) {
                    Ok(()) => {
                        self.socket = Some(udp_socket);
                        return CreateConnectResult::Success;
                    },
                    Err(_) => {
                        return CreateConnectResult::Error;
                    },
                }
            },
            Err(_) =>  {
                return CreateConnectResult::Error;
            },
        }
    }

    fn should_drop(&mut self,data: &mut[u8], drop_chance: u64, drop_reliable_only: bool) -> bool {
        if drop_chance > 0 {
            let r = self.rng.gen_range(1..100);
            if r <= drop_chance {
                let mut can_drop = true;
                if drop_reliable_only {
                    let mut reader = IntBuffer {index: 0};
                    let message_type = reader.read_u8(data);
                    if message_type != MESSAGE_TYPE_RELIABLE {
                        can_drop = false;
                    }
                }
                
                if can_drop {
                    return true;
                }
            }
        }
        return false;
    }

    pub fn receive(&mut self, data:  &mut[u8], drop_chance: u64, drop_reliable_only: bool) -> SocketReceiveResult {
        let socket = match &self.socket {
            Some(v) =>{v},
            None => {
                return SocketReceiveResult::Error;
            }
        };
        
        if self.is_server {
            match  socket.recv_from(data) {
                Ok((bytes_received, src_addr)) => {
                    if self.should_drop(data, drop_chance, drop_reliable_only) {
                        return SocketReceiveResult::Dropped;
                    }
                    let address = NetworkAddress::from_socket_addr(src_addr);
                    return SocketReceiveResult::Success { bytes_received, network_address: address };
                },
                Err(_) => {
                    return SocketReceiveResult::Empty;
                },
            }
        } else {
            match  socket.recv(data) {
                Ok(size) => {
                    if self.should_drop(data, drop_chance, drop_reliable_only) {
                        return SocketReceiveResult::Dropped;
                    }
                    return SocketReceiveResult::Success { bytes_received: size, network_address: NetworkAddress::default() };
                },
                Err(_) => {
                    return SocketReceiveResult::Empty;
                },
            }
        }
    }

    pub fn send_to_timed(counters: &mut SocketTimeCounters, socket: &TachyonSocket, address: NetworkAddress, data:  &[u8], length: usize) -> usize {
        let now = Instant::now();
        let sent_len = socket.send_to(address, data, length);
        let elapsed = now.elapsed().as_micros();
        counters.socket_send_time  += elapsed;
        return sent_len;
    }
    
    pub fn send_to(&self,address: NetworkAddress, data:  &[u8], length: usize) -> usize {
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
