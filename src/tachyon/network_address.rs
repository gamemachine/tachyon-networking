use std::{
    hash::{Hash, Hasher},
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

#[derive(Eq, Default, Clone, Copy)]
#[repr(C)]
pub struct NetworkAddress {
    pub a: u16,
    pub b: u16,
    pub c: u16,
    pub d: u16,
    pub port: u32,
}

impl std::fmt::Display for NetworkAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{0}.{1}.{2}.{3}:{4}\n",
            self.a, self.b, self.c, self.d, self.port
        )
    }
}

impl Hash for NetworkAddress {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        let hash = NetworkAddress::get_hash(&self);
        hasher.write_u32(hash);
        hasher.finish();
    }
}

impl PartialEq for NetworkAddress {
    fn eq(&self, other: &Self) -> bool {
        self.a == other.a
            && self.b == other.b
            && self.c == other.c
            && self.d == other.d
            && self.port == other.port
    }
}

impl NetworkAddress {
    pub fn test_address() -> Self {
        let address = NetworkAddress {
            a: 127,
            b: 0,
            c: 0,
            d: 1,
            port: 8265,
        };
        return address;
    }

    pub fn broadcast(channel: u8) -> Self {
        NetworkAddress {
            a: 255,
            b: 255,
            c: 255,
            d: 255,
            port: channel as u32,
        }
    }

    pub fn localhost(port: u32) -> Self {
        let address = NetworkAddress {
            a: 127,
            b: 0,
            c: 0,
            d: 1,
            port: port,
        };
        return address;
    }

    pub fn mock_client_address() -> Self {
        let address = NetworkAddress {
            a: 127,
            b: 0,
            c: 0,
            d: 1,
            port: 4598,
        };
        return address;
    }

    pub fn from_socket_addr(address: SocketAddr) -> NetworkAddress {
        if let IpAddr::V4(ipv4) = address.ip() {
            let parts = ipv4.octets();
            let result = NetworkAddress {
                a: parts[0] as u16,
                b: parts[1] as u16,
                c: parts[2] as u16,
                d: parts[3] as u16,
                port: address.port() as u32,
            };
            return result;
        } else {
            return NetworkAddress::default();
        }
    }

    pub fn to_socket_addr(&self) -> SocketAddr {
        let ip = Ipv4Addr::new(self.a as u8, self.b as u8, self.c as u8, self.d as u8);
        return SocketAddr::new(IpAddr::V4(ip), self.port as u16);
    }

    pub fn is_default(&self) -> bool {
        return NetworkAddress::default() == *self;
    }

    pub fn is_broadcast(&self) -> bool {
        self.a == 255 && self.b == 255 && self.c == 255 && self.d == 255
    }

    pub fn copy_from(&mut self, other: NetworkAddress) {
        self.a = other.a;
        self.b = other.b;
        self.c = other.c;
        self.d = other.d;
        self.port = other.port;
    }

    pub fn get_hash(&self) -> u32 {
        let mut hash: u32 = 17;
        hash = hash.wrapping_mul(23).wrapping_add(self.a as u32);
        hash = hash.wrapping_mul(23).wrapping_add(self.b as u32);
        hash = hash.wrapping_mul(23).wrapping_add(self.c as u32);
        hash = hash.wrapping_mul(23).wrapping_add(self.d as u32);
        hash = hash.wrapping_mul(23).wrapping_add(self.port as u32);

        /* hash = hash * 23 + self.a as u32;
        hash = hash * 23 + self.b as u32;
        hash = hash * 23 + self.c as u32;
        hash = hash * 23 + self.d as u32;
        hash = hash * 23 + self.port as u32; */
        return hash;
    }
}
