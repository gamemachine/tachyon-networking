use std::{
    collections::VecDeque,
    sync::{Arc}
};

use crossbeam::queue::ArrayQueue;
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};
use rustc_hash::FxHashMap;
use synchronoise::CountdownEvent;

use super::{network_address::NetworkAddress, Tachyon, TachyonConfig, int_buffer::LengthPrefixed, connection::Connection, TachyonSendResult};


#[derive(Clone, Copy)]
#[repr(C)]
#[derive(Default)]
pub struct SendTarget {
    pub identity_id: u32,
    pub address: NetworkAddress
}

#[derive(Default, Clone, Copy)]
#[repr(C)]
pub struct PoolServerRef {
    pub address: NetworkAddress,
    pub id: u16
}

#[derive(Default, Clone, Copy)]
#[repr(C)]
pub struct OutBufferCounts {
    pub bytes_written: u32,
    pub count: u32
}

pub struct OutBuffer {
    pub data: Vec<u8>,
    pub bytes_written: u32,
    pub count: u32
}

pub struct Pool {
    pub next_id: u16,
    pub max_servers: u8,
    pub receive_buffer_len: u32,
    pub servers: FxHashMap<u16, Tachyon>,
    pub receive_queue: Arc<ArrayQueue<VecDeque<Vec<u8>>>>,
    pub receive_buffers: Arc<ArrayQueue<Vec<u8>>>,
    pub out_buffers: Arc<ArrayQueue<OutBuffer>>,
    pub published: VecDeque<Vec<u8>>,
    pub servers_in_use: Arc<ArrayQueue<Tachyon>>,
    pub counter: Option<Arc<CountdownEvent>>,
    pub connections_by_identity: FxHashMap<u32, Connection>,
    pub connections_by_address: FxHashMap<NetworkAddress, Connection>
    
}

impl Pool {
    pub fn create(max_servers: u8, receive_buffer_len: u32, out_buffer_len: u32) -> Self {

        let receive_buffers: ArrayQueue<Vec<u8>> = ArrayQueue::new(max_servers as usize);
        let out_buffers: ArrayQueue<OutBuffer> = ArrayQueue::new(max_servers as usize);
        let queue: ArrayQueue<VecDeque<Vec<u8>>> = ArrayQueue::new(max_servers as usize);

        for _ in 0..max_servers {
            queue.push(VecDeque::new()).unwrap_or(());
            receive_buffers.push(vec![0; receive_buffer_len as usize]).unwrap_or(());

            let buffer = OutBuffer {
                data: vec![0; out_buffer_len as usize],
                bytes_written: 0,
                count: 0
            };
            out_buffers.push(buffer).unwrap_or(());
        }
        
        let in_use: ArrayQueue<Tachyon> = ArrayQueue::new(max_servers as usize);

        let pool = Pool {
            next_id: 0,
            max_servers,
            receive_buffer_len,
            servers: FxHashMap::default(),
            receive_queue: Arc::new(queue),
            receive_buffers: Arc::new(receive_buffers),
            out_buffers: Arc::new(out_buffers),
            published: VecDeque::new(),
            servers_in_use: Arc::new(in_use),
            counter: None,
            connections_by_identity: FxHashMap::default(),
            connections_by_address: FxHashMap::default()
        };
        return pool;
    }

    pub fn create_server(&mut self, config: TachyonConfig, address: NetworkAddress, id: u16) -> bool {

        if self.servers.len() > self.max_servers.into() {
            return false;
        }
        if self.servers.contains_key(&id) {
            return false;
        }

        let mut tachyon = Tachyon::create(config);
        match tachyon.bind(address) {
            true => {
                tachyon.id = id;
                self.servers.insert(id, tachyon);

                return true;
            }
            false => {
                return false;
            }
        }
    }

    pub fn set_identity(&mut self, server_id: u16, id: u32, session_id: u32, on_self: u32) {
        if let Some(tachyon) = self.get_server(server_id) {
            if on_self == 1 {
                tachyon.identity.id = id;
                tachyon.identity.session_id = session_id;
            } else {
                tachyon.set_identity(id, session_id);
            }
        }
    }

    pub fn build_connection_maps(&mut self) {
        self.connections_by_address.clear();
        self.connections_by_identity.clear();

        for server in self.servers.values_mut() {
            for conn in server.connections.values() {
                self.connections_by_address.insert(conn.address, *conn);
                if conn.identity.id > 0 {
                    self.connections_by_identity.insert(conn.identity.id, *conn);
                }
            }
        }
    }

    pub fn get_server_having_connection(&self, address: NetworkAddress) -> u16 {
        if let Some(conn) = self.connections_by_address.get(&address) {
            return conn.tachyon_id;
        } else {
            return 0;
        }
    }

    pub fn get_server_having_identity(&self, identity_id: u32) -> u16 {
        if let Some(conn) = self.connections_by_identity.get(&identity_id) {
            return conn.tachyon_id;
        } else {
            return 0;
        }
    }

    pub fn get_available_server(&self) -> Option<PoolServerRef> {
        let mut best: Option<PoolServerRef> = None;
        let mut low = 10000;
        for (_id,server) in &self.servers {
            let conn_count = server.connections.len();
            if conn_count < low && server.socket.socket.is_some() {
               low = conn_count;
               best = Some(PoolServerRef {address: server.socket.address, id: server.id});
            }
        }

        return best;
    }
    
    pub fn get_server(&mut self, id: u16) -> Option<&mut Tachyon> {
        return self.servers.get_mut(&id);
    }

    pub fn send_to_target(&mut self,channel_id: u8, target: SendTarget, data: &mut [u8], length: i32) -> TachyonSendResult {
        if target.identity_id > 0 {
            return self.send_to_identity(channel_id,target.identity_id, data, length);
        } else {
            return self.send_to_address(channel_id,target.address, data, length);
        }
    }

    fn send_to_identity(&mut self, channel_id: u8, id: u32, data: &mut [u8], length: i32) -> TachyonSendResult {
        if let Some(conn) = self.connections_by_identity.get(&id) {
            if let Some(server) = self.servers.get_mut(&conn.tachyon_id) {
                if channel_id == 0 {
                    return server.send_unreliable(conn.address, data, length as usize);
                } else {
                    return server.send_reliable(channel_id,conn.address, data, length as usize);
                }
            }
        }
        return TachyonSendResult::default();
    }

    fn send_to_address(&mut self,channel_id: u8, address: NetworkAddress, data: &mut [u8], length: i32) -> TachyonSendResult {
        if let Some(conn) = self.connections_by_address.get(&address) {
            if let Some(sender) = self.servers.get_mut(&conn.tachyon_id) {
                if channel_id == 0 {
                    return sender.send_unreliable(address, data, length as usize);
                } else {
                    return sender.send_reliable(channel_id, address, data, length as usize);
                }
            }
        }
        return TachyonSendResult::default();
    }

    pub fn take_published(&mut self) -> Option<Vec<u8>> {
        return self.published.pop_front();
    }

    fn move_received_to_published(&mut self) -> i32 {
        let mut count = 0;
        for _ in 0..self.receive_queue.len() {
            if let Some(mut receive_queue) = self.receive_queue.pop() {
                for value in receive_queue.drain(..) {
                    self.published.push_back(value);
                    count += 1;
                }
                self.receive_queue.push(receive_queue).unwrap_or_default();
            }
        }
        return count;
    }

    fn receive_server(server: &mut Tachyon, receive_queue: &mut VecDeque<Vec<u8>>, receive_buffer: &mut Vec<u8>) {
        for _ in 0..100000 {
            let res = server.receive_loop(receive_buffer);
            if res.length == 0 || res.error > 0 {
                break;
            } else {
                let mut message: Vec<u8> = vec![0; res.length as usize];
                message.copy_from_slice(&receive_buffer[0..res.length as usize]);
                receive_queue.push_back(message);

            }
        }
    }

    // receive and finish_receive go together, this heap allocates and puts messages into a queue
    pub fn receive(&mut self) -> bool {
        let server_count = self.servers.len();
        if server_count == 0 {
            return false;
        }

        let counter = Arc::new(CountdownEvent::new(server_count));

        let in_use = self.servers_in_use.clone();
        for s in self.servers.drain() {
            let server = s.1;
            in_use.push(server).unwrap_or(());
        }

        for _ in 0..server_count {
            let in_use = self.servers_in_use.clone();
            let receive_queue_clone = self.receive_queue.clone();
            let receive_buffers_clone = self.receive_buffers.clone();
            let signal = counter.clone();

            rayon::spawn(move || {
                match in_use.pop() {
                    Some(mut server) => {
                        if let Some(mut receive_queue) = receive_queue_clone.pop() {
                            if let Some(mut receive_buffer) = receive_buffers_clone.pop() {
                                Pool::receive_server(&mut server, &mut receive_queue, &mut receive_buffer);
                                receive_buffers_clone.push(receive_buffer).unwrap_or_default();
                            }
                            receive_queue_clone.push(receive_queue).unwrap_or_default();
                        }
                        in_use.push(server).unwrap_or(());
                    }
                    None => {}
                }
                signal.decrement().unwrap();
            });
        }
        self.counter = Some(counter);
        return true;
    }

    pub fn finish_receive(&mut self) -> (u32, i32) {
        let mut server_count = 0;
        let mut message_count = 0;

        match &self.counter {
            Some(counter) => {
                counter.wait();
                message_count += self.move_received_to_published();

                for _ in 0..self.servers_in_use.len() {
                    if let Some(server) = self.servers_in_use.pop() {
                        self.servers.insert(server.id, server);
                        server_count += 1;
                    }
                }
                self.counter = None;
            }
            None => {}
        }
        return (server_count, message_count);
    }

    // receive blocking, also heap allocates into the queue
    pub fn receive_blocking(&mut self) {
        self.servers.par_iter_mut().for_each(|(_key, server)| {
            let receive_queue_clone = self.receive_queue.clone();
            let receive_buffers_clone = self.receive_buffers.clone();

            if let Some(mut receive_queue) = receive_queue_clone.pop() {
                if let Some(mut receive_buffer) = receive_buffers_clone.pop() {
                    Pool::receive_server(server, &mut receive_queue, &mut receive_buffer);
                    receive_buffers_clone.push(receive_buffer).unwrap_or_default();
                }
                receive_queue_clone.push(receive_queue).unwrap_or_default();
            }
        });
        self.move_received_to_published();
    }


    // blocking receive with more complex api.  messages are copied to a single out buffer with length and ip address prefixed.
    pub fn receive_blocking_out_buffer(&mut self) {
        self.servers.par_iter_mut().for_each(|(_key, server)| {
            let receive_buffers_clone = self.receive_buffers.clone();
            let out_buffers_clone = self.out_buffers.clone();

            if let Some(mut out_buffer) = out_buffers_clone.pop() {
                out_buffer.bytes_written = 0;
                out_buffer.count = 0;

                if let Some(mut receive_buffer) = receive_buffers_clone.pop() {
                    Pool::receive_server_into_out_buffer(server, &mut out_buffer, &mut receive_buffer);
                    receive_buffers_clone.push(receive_buffer).unwrap_or_default();
                }
                out_buffers_clone.push(out_buffer).unwrap_or_default();
            }
        });
    }

    fn receive_server_into_out_buffer(server: &mut Tachyon, out_buffer: &mut OutBuffer, receive_buffer: &mut Vec<u8>) {
        let mut writer = LengthPrefixed::default();
        for _ in 0..100000 {
            let res = server.receive_loop(receive_buffer);
            if res.length == 0 || res.error > 0 {
                out_buffer.bytes_written = writer.writer.index as u32;
                break;
            } else {
                writer.write(res.channel,res.address,&receive_buffer[0..res.length as usize], &mut out_buffer.data);
                out_buffer.count += 1;
            }
        }
    }

    pub fn get_next_out_buffer(&mut self, receive_buffer: &mut [u8]) -> OutBufferCounts {
        let mut result = OutBufferCounts::default();

        for _ in 0..self.out_buffers.len() {
            if let Some(mut out_buffer) = self.out_buffers.pop() {
                if out_buffer.count == 0 {
                    self.out_buffers.push(out_buffer).unwrap_or_default();
                    continue;
                }

                receive_buffer[0..out_buffer.bytes_written as usize].copy_from_slice(&out_buffer.data[0..out_buffer.bytes_written as usize]);

                result.count = out_buffer.count;
                result.bytes_written = out_buffer.bytes_written;

                out_buffer.bytes_written = 0;
                out_buffer.count = 0;

                self.out_buffers.push(out_buffer).unwrap_or_default();

                return result;
            }
        }
        return result;
    }

}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use crate::tachyon::{
        network_address::NetworkAddress,
        tachyon_test::{TachyonTestClient},
        TachyonConfig, int_buffer::{IntBuffer, LengthPrefixed}
    };
    use std::{
        time::Instant,
    };

    use super::Pool;

    #[test]
    #[serial]
    fn test_blocking_receive() {
        let mut pool = Pool::create(40, 1024 * 1024, 1024 * 1024 * 4);
        let config = TachyonConfig::default();
        pool.create_server(config, NetworkAddress::localhost(8001),1);
        pool.create_server(config, NetworkAddress::localhost(8002),2);
        pool.create_server(config, NetworkAddress::localhost(8003),3);

        let mut id = 4;
        for i in 0..20 {
            pool.create_server(config, NetworkAddress::localhost(8004 + i), id);
            id += 1;
        }

        let mut client1 = TachyonTestClient::create(NetworkAddress::localhost(8001));
        let mut client2 = TachyonTestClient::create(NetworkAddress::localhost(8002));
        let mut client3 = TachyonTestClient::create(NetworkAddress::localhost(8003));
        client1.connect();
        client2.connect();
        client3.connect();

        let count: usize = 2000;
        let msg_len = 64;
        let msg_value = 234873;

        for _ in 0..count {
            let mut writer = IntBuffer {index: 0};
            writer.write_u32(msg_value, &mut client1.send_buffer);
            client1.client_send_reliable(1, msg_len);

            let mut writer = IntBuffer {index: 0};
            writer.write_u32(msg_value, &mut client2.send_buffer);
            client2.client_send_reliable(1, msg_len);

            let mut writer = IntBuffer {index: 0};
            writer.write_u32(msg_value, &mut client3.send_buffer);
            client3.client_send_reliable(1, msg_len);
        }

        let now = Instant::now();
        pool.receive_blocking_out_buffer();

        let elapsed = now.elapsed();
        println!("Elapsed: {:.2?}", elapsed);

        // should have 3 out buffers to read
        let mut receive_buffer: Vec<u8> = vec![0;1024 * 1024 * 4];
        let res = pool.get_next_out_buffer(&mut receive_buffer);
        println!("bytes_written:{0} count:{1}", res.bytes_written, res.count);

        

        // length + channel + address + body
        let bytes_written = count * msg_len + count * 4 + count * 14;
        assert_eq!(res.bytes_written, bytes_written as u32);
        assert_eq!(count, res.count as usize);

        let mut reader = LengthPrefixed::default();
        for _ in 0..res.count {
            let (_channel,_address,range) = reader.read(&receive_buffer);
            let len = range.end - range.start;
            //println!("len:{0} address:{1}", len, address);

            assert_eq!(msg_len, len as usize);
            let slice = &receive_buffer[range];

            let mut value_reader = IntBuffer {index: 0};
            let value = value_reader.read_u32(slice);
            assert_eq!(msg_value, value);
        }

        let res = pool.get_next_out_buffer(&mut receive_buffer);
        assert_eq!(res.bytes_written, bytes_written as u32);
        assert_eq!(count, res.count as usize);
        
        let res = pool.get_next_out_buffer(&mut receive_buffer);
        assert_eq!(res.bytes_written, bytes_written as u32);
        assert_eq!(count, res.count as usize);

        let res = pool.get_next_out_buffer(&mut receive_buffer);
        assert_eq!(res.bytes_written, 0);
        assert_eq!(0, res.count as usize);

        // servers and arrays returned
        assert_eq!(pool.max_servers as usize, pool.out_buffers.len());
        assert_eq!(23, pool.servers.len());

    }

    #[test]
    #[serial]
    fn test_receive() {
        let mut pool = Pool::create(4, 1024 * 1024, 1024 * 1024 * 4);
        let config = TachyonConfig::default();
        pool.create_server(config, NetworkAddress::localhost(8001),1);
        pool.create_server(config, NetworkAddress::localhost(8002),2);
        pool.create_server(config, NetworkAddress::localhost(8003),3);

        let mut client1 = TachyonTestClient::create(NetworkAddress::localhost(8001));
        let mut client2 = TachyonTestClient::create(NetworkAddress::localhost(8002));
        let mut client3 = TachyonTestClient::create(NetworkAddress::localhost(8003));
        client1.connect();
        client2.connect();
        client3.connect();

        let count = 200;
        let msg_len = 64;
        let msg_value = 234873;
        for _ in 0..count {
            let mut writer = IntBuffer {index: 0};
            writer.write_u32(msg_value, &mut client1.send_buffer);
            client1.client_send_reliable(1, msg_len);

            let mut writer = IntBuffer {index: 0};
            writer.write_u32(msg_value, &mut client2.send_buffer);
            client2.client_send_reliable(1, msg_len);

            let mut writer = IntBuffer {index: 0};
            writer.write_u32(msg_value, &mut client3.send_buffer);
            client3.client_send_reliable(1, msg_len);
        }

        let now = Instant::now();
        let receiving = pool.receive();
        assert!(receiving);

        // should return false, all servers moved
        let receiving = pool.receive();
        assert!(!receiving);

        let res = pool.finish_receive();
        assert_eq!(3, res.0);
        assert_eq!(count * 3, res.1);
        assert_eq!(count * 3, pool.published.len() as i32);

        // nothing to finish
        let res = pool.finish_receive();
        assert_eq!(0, res.0);
        assert_eq!(0, res.1);

        let elapsed = now.elapsed();
        println!("Elapsed: {:.2?}", elapsed);

    }

    
}
