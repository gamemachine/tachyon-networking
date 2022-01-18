
use std::{sync::{Arc, Mutex}, time::Instant, collections::VecDeque, panic};

use crossbeam::queue::ArrayQueue;
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};
use rustc_hash::FxHashMap;
use synchronoise::CountdownEvent;

use super::{Tachyon, TachyonConfig, network_address::NetworkAddress};

const MAX_SERVERS: usize = 3;

pub struct Pool {
    pub next_id: u16,
    pub servers: FxHashMap<u16, Tachyon>,
    pub receive_queue: Arc<ArrayQueue<VecDeque<Vec<u8>>>>,
    pub receive_buffers: Arc<ArrayQueue<Vec<u8>>>,
    pub published: VecDeque<Vec<u8>>,
    pub servers_in_use: Arc<ArrayQueue<Tachyon>>,
    pub counter: Option<Arc<CountdownEvent>>
}

impl Pool {
    pub fn create() -> Self {
        let receive_buffers: ArrayQueue<Vec<u8>> = ArrayQueue::new(MAX_SERVERS);
        let queue: ArrayQueue<VecDeque<Vec<u8>>> = ArrayQueue::new(MAX_SERVERS);
        for _ in 0..MAX_SERVERS {
            queue.push(VecDeque::new());
            receive_buffers.push(vec![0;1024 * 1024]);
        }
        let in_use:ArrayQueue<Tachyon> = ArrayQueue::new(MAX_SERVERS);

        let pool = Pool {
            next_id: 0,
            servers: FxHashMap::default(),
            receive_queue: Arc::new(queue),
            receive_buffers: Arc::new(receive_buffers),
            published: VecDeque::new(),
            servers_in_use: Arc::new(in_use),
            counter: None
        };
        return pool;
    }

    pub fn create_server(&mut self, config: TachyonConfig, address: NetworkAddress) -> Option<&mut Tachyon> {
        let mut tachyon = Tachyon::create(config);
        match tachyon.bind(address) {
            true => {
                self.next_id += 1;
                let id = self.next_id;
                tachyon.id = id;
                self.servers.insert(id, tachyon);
                
                return self.servers.get_mut(&id);
            },
            false => {
                return None;
            },
        }
    }
   
    pub fn get_server(&mut self, id: u16) -> Option<&mut Tachyon> {
        return self.servers.get_mut(&id);
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
        let mut count = 0;
        let now = Instant::now();
        for _ in 0..100000 {
            let res = server.receive_loop(receive_buffer);
            if res.length == 0 || res.error > 0 {
                let elapsed = now.elapsed().as_millis();
                println!("received {0} Elapsed: {1}", count, elapsed);
                break;
            } else {
                let mut message: Vec<u8> = vec![0;res.length as usize];
                message.copy_from_slice(&receive_buffer[0..res.length as usize]);
                receive_queue.push_back(message);

                count += 1;
            }
        }
    }

    pub fn receive_blocking(&mut self) {
        self.servers.par_iter_mut().for_each(|(key,server)|{
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
            },
            None => {},
        }
        return (server_count, message_count);
    }
    
    pub fn receive(&mut self) -> bool {

        let server_count = self.servers.len();
        if server_count == 0 {
            return false;
        }

        let counter = Arc::new(CountdownEvent::new(server_count));
        
        let in_use = self.servers_in_use.clone();
        for s in self.servers.drain() {
            let server = s.1;
            in_use.push(server);
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
                        in_use.push(server);
                    },
                    None => {
    
                    },
                }
                signal.decrement().unwrap();
             });
        }
        self.counter = Some(counter);
        return true;
    }

}


#[cfg(test)]
mod tests {
    use core::time;
    use std::{time::Instant, thread, sync::{Mutex, Arc}};
    use crate::tachyon::{tachyon_test::{TachyonTest, TachyonTestClient}, network_address::NetworkAddress, Tachyon, TachyonConfig};
    use crossbeam::queue::ArrayQueue;
    use rayon::prelude::*;

    use super::Pool;
    
    #[test]
    fn test_receive() {
        let mut pool = Pool::create();
        let config = TachyonConfig::default();
        pool.create_server(config, NetworkAddress::localhost(8001));
        pool.create_server(config, NetworkAddress::localhost(8002));
        pool.create_server(config, NetworkAddress::localhost(8003));

        let mut client1 = TachyonTestClient::create(NetworkAddress::localhost(8001));
        let mut client2 = TachyonTestClient::create(NetworkAddress::localhost(8002));
        let mut client3 = TachyonTestClient::create(NetworkAddress::localhost(8003));
        client1.connect();
        client2.connect();
        client3.connect();

        let count = 20000;
        let msg_len = 64;

        for _ in 0..count {
            client1.client_send_reliable(1, msg_len);
            client2.client_send_reliable(1, msg_len);
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

    #[test]
    fn test_blocking_receive() {
        let mut pool = Pool::create();
        let config = TachyonConfig::default();
        pool.create_server(config, NetworkAddress::localhost(8001));
        pool.create_server(config, NetworkAddress::localhost(8002));
        pool.create_server(config, NetworkAddress::localhost(8003));

        let mut client1 = TachyonTestClient::create(NetworkAddress::localhost(8001));
        let mut client2 = TachyonTestClient::create(NetworkAddress::localhost(8002));
        let mut client3 = TachyonTestClient::create(NetworkAddress::localhost(8003));
        client1.connect();
        client2.connect();
        client3.connect();

        let count = 20000;
        let msg_len = 64;

        for _ in 0..count {
            client1.client_send_reliable(1, msg_len);
            client2.client_send_reliable(1, msg_len);
            client3.client_send_reliable(1, msg_len);
        }

        let now = Instant::now();
        pool.receive_blocking();

        let elapsed = now.elapsed();
        println!("Elapsed: {:.2?}", elapsed);
        
    }

   

    #[test]
    fn test_receive_in_thread() {
        
        let mut test = TachyonTest::create(NetworkAddress::localhost(8001));
        test.connect();

        let shared = Arc::new(Mutex::new(0));
        
        let mut server = test.server;
        let lock = shared.clone();
        rayon::spawn(move || {
            match lock.try_lock() {
                Ok(_res) => {
                    println!("thread acquired");
                    let mut temp:Vec<u8> = vec![0;4096];
                    server.receive_from_socket(&mut temp);
                    let ms = time::Duration::from_millis(500);
                    thread::sleep(ms);
                },
                Err(_err) => {
                    println!("thread failed");
                },
            }
         });

        let ms = time::Duration::from_millis(10);
        thread::sleep(ms);
         match shared.clone().try_lock() {
            Ok(ok) => {
                println!("main acquired");
            },
            Err(err) => {
                println!("main failed");
            },
        }
    }


}