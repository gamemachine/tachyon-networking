use std::{
    io,
    net::{Ipv4Addr, SocketAddrV4, UdpSocket},
};

use rand::{prelude::StdRng, Rng, SeedableRng};
use socket2::{Domain, Socket, Type};

use super::{
    header::{Header, MESSAGE_TYPE_RELIABLE},
    int_buffer::IntBuffer,
    network_address::NetworkAddress,
    sequence::Sequence,
};

pub enum CreateConnectResult {
    Success,
    Error,
}

pub enum SocketReceiveResult {
    Success {
        bytes_received: usize,
        network_address: NetworkAddress,
    },
    Empty,
    Error,
    Dropped,
}

pub struct TachyonSocket {
    pub address: NetworkAddress,
    pub is_server: bool,
    pub socket: Option<UdpSocket>,
    pub rng: StdRng,
    pub test_mode: bool,
    pub test_sequence: u16,
}

impl TachyonSocket {
    pub fn create() -> Self {
        TachyonSocket {
            address: NetworkAddress::default(),
            is_server: false,
            socket: None,
            rng: SeedableRng::seed_from_u64(32634),
            test_mode: false,
            test_sequence: 0,
        }
    }

    pub fn clone_socket(&self) -> Option<UdpSocket> {
        self.socket.as_ref()?.try_clone().ok()
    }

    pub fn bind_socket(&mut self, net_address: NetworkAddress) -> CreateConnectResult {
        if self.socket.is_some() {
            return CreateConnectResult::Error;
        }

        let address = net_address.to_socket_addr();
        self.address = net_address;

        let socket = Socket::new(Domain::IPV4, Type::DGRAM, None).unwrap();
        match socket.bind(&address.into()) {
            Ok(()) => {
                socket.set_recv_buffer_size(8192 * 256).unwrap();
                socket.set_nonblocking(true).unwrap();
                self.socket = Some(socket.into());
                self.is_server = true;

                CreateConnectResult::Success
            }
            Err(_) => CreateConnectResult::Error,
        }
    }

    pub fn connect_socket(&mut self, net_address: NetworkAddress) -> CreateConnectResult {
        if self.socket.is_some() {
            return CreateConnectResult::Error;
        }

        self.address = NetworkAddress::default();
        let sock_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, None).unwrap();

        if socket.bind(&sock_addr.into()).is_ok() {
            socket.set_recv_buffer_size(8192 * 256).unwrap();
            socket.set_nonblocking(true).unwrap();
            let address = net_address.to_socket_addr();
            let udp_socket: UdpSocket = socket.into();

            if udp_socket.connect(&address).is_ok() {
                self.socket = Some(udp_socket);
                return CreateConnectResult::Success;
            };
        }

        CreateConnectResult::Error
    }

    fn should_drop(&mut self, data: &mut [u8], drop_chance: u64, drop_reliable_only: bool) -> bool {
        let mut can_drop = false;

        if drop_chance > 0 {
            let r = self.rng.gen_range(1..100);
            if r <= drop_chance {
                can_drop = true;
                if drop_reliable_only {
                    let mut reader = IntBuffer { index: 0 };
                    let message_type = reader.read_u8(data);
                    if message_type != MESSAGE_TYPE_RELIABLE {
                        can_drop = false;
                    }
                }
            }
        }

        can_drop
    }

    pub fn receive(
        &mut self,
        data: &mut [u8],
        drop_chance: u64,
        drop_reliable_only: bool,
    ) -> SocketReceiveResult {
        if self.test_mode {
            let mut head = Header::default();
            head.message_type = MESSAGE_TYPE_RELIABLE;
            head.channel = 1;
            head.sequence = self.test_sequence;
            self.test_sequence = Sequence::next_sequence(self.test_sequence);
            head.write(data);

            return SocketReceiveResult::Success {
                bytes_received: 32,
                network_address: NetworkAddress::mock_client_address(),
            };
        }

        let socket = match &self.socket {
            Some(v) => v,
            None => {
                return SocketReceiveResult::Error;
            }
        };

        if self.is_server {
            match socket.recv_from(data) {
                Ok((bytes_received, src_addr)) => {
                    if self.should_drop(data, drop_chance, drop_reliable_only) {
                        return SocketReceiveResult::Dropped;
                    }
                    let address = NetworkAddress::from_socket_addr(src_addr);
                    SocketReceiveResult::Success {
                        bytes_received,
                        network_address: address,
                    }
                }
                _ => SocketReceiveResult::Empty,
            }
        } else {
            match socket.recv(data) {
                Ok(size) => {
                    if self.should_drop(data, drop_chance, drop_reliable_only) {
                        return SocketReceiveResult::Dropped;
                    }
                    SocketReceiveResult::Success {
                        bytes_received: size,
                        network_address: NetworkAddress::default(),
                    }
                }
                _ => SocketReceiveResult::Empty,
            }
        }
    }

    pub fn send_to(&self, address: NetworkAddress, data: &[u8], length: usize) -> usize {
        if self.socket.is_none() {
            return 0;
        };

        let socket = self.socket.as_ref().unwrap();
        let slice = &data[0..length];
        let socket_result: io::Result<usize> = match address.port {
            0 => socket.send(slice),
            _ => socket.send_to(slice, address.to_socket_addr()),
        };

        match socket_result {
            Ok(size) => size,
            _ => 0,
        }
    }
}
