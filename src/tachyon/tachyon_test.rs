use core::time;
use std::thread;
use std::time::Instant;

use serial_test::serial;

use crate::tachyon::header::*;
use crate::tachyon::receiver::*;
use crate::tachyon::*;

pub struct TachyonTestClient {
    pub client_address: NetworkAddress,
    pub client: Tachyon,
    pub address: NetworkAddress,
    pub config: TachyonConfig,
    pub receive_buffer: Vec<u8>,
    pub send_buffer: Vec<u8>,
}

impl TachyonTestClient {
    pub fn create(address: NetworkAddress) -> Self {
        let config = TachyonConfig::default();
        let client = Tachyon::create(config);
        let default = TachyonTestClient {
            client_address: NetworkAddress::default(),
            address: address,
            config: config,
            client: client,
            receive_buffer: vec![0; 4096],
            send_buffer: vec![0; 4096],
        };
        return default;
    }

    pub fn connect(&mut self) {
        assert!(self.client.connect(self.address), "connect failed");
    }

    pub fn client_send_reliable(&mut self, channel_id: u8, length: usize) -> TachyonSendResult {
        let target = SendTarget {address: self.client_address, identity_id: 0};
        return self.client.send_to_target(channel_id, target,&mut self.send_buffer, length);
    }

    pub fn client_send_unreliable(&mut self, length: usize) -> TachyonSendResult {
        let target = SendTarget {address: self.client_address, identity_id: 0};
        return self.client.send_to_target(0,target, &mut self.send_buffer, length);
    }

    pub fn client_receive(&mut self) -> TachyonReceiveResult {
        return self.client.receive_loop(&mut self.receive_buffer);
    }
}

pub struct TachyonTest {
    pub client_address: NetworkAddress,
    pub client: Tachyon,
    pub server: Tachyon,
    pub address: NetworkAddress,
    pub config: TachyonConfig,
    pub receive_buffer: Vec<u8>,
    pub send_buffer: Vec<u8>,
}

impl TachyonTest {
    pub fn default() -> Self {
        let address = NetworkAddress::test_address();
        return TachyonTest::create(address);
    }

    pub fn create(address: NetworkAddress) -> Self {
        let config = TachyonConfig::default();
        let server = Tachyon::create(config);
        let client = Tachyon::create(config);

        let default = TachyonTest {
            client_address: NetworkAddress::default(),
            address: address,
            config: config,
            client: client,
            server: server,
            receive_buffer: vec![0; 4096],
            send_buffer: vec![0; 4096],
        };
        return default;
    }

    pub fn connect(&mut self) {
        assert!(self.server.bind(self.address), "bind failed");
        assert!(self.client.connect(self.address), "connect failed");
    }

    pub fn remote_client(&mut self) -> NetworkAddress {
        let list = self.server.get_connections(100);
        if list.len() > 0 {
            return list[0].address;
        } else {
            return NetworkAddress::default();
        }
    }

    pub fn server_send_reliable(&mut self, channel_id: u8, length: usize) -> TachyonSendResult {
        let address = self.remote_client();
        if address.is_default() {
            return TachyonSendResult::default();
        }
        let target = SendTarget {address: address, identity_id: 0};
        return self.server.send_to_target(channel_id, target, &mut self.send_buffer, length);
    }

    pub fn server_send_unreliable(&mut self, length: usize) -> TachyonSendResult {
        let address = self.remote_client();
        if address.is_default() {
            return TachyonSendResult::default();
        }
        let target = SendTarget {address: address, identity_id: 0};
        return self.server.send_to_target(0,target, &mut self.send_buffer, length);
    }

    pub fn client_send_reliable(&mut self, channel_id: u8, length: usize) -> TachyonSendResult {
        let target = SendTarget {address: self.client_address, identity_id: 0};
        return self.client.send_to_target(channel_id,  target,&mut self.send_buffer,length);
    }

    pub fn client_send_unreliable(&mut self, length: usize) -> TachyonSendResult {
        let target = SendTarget {address: self.client_address, identity_id: 0};
        return self.client.send_to_target(0,target, &mut self.send_buffer, length);
    }

    pub fn server_receive(&mut self) -> TachyonReceiveResult {
        return self.server.receive_loop(&mut self.receive_buffer);
    }

    pub fn client_receive(&mut self) -> TachyonReceiveResult {
        return self.client.receive_loop(&mut self.receive_buffer);
    }
}

pub fn show_channel_debug(channel: &mut Receiver) {
    print!(
        "current:{0} last:{1}\n",
        channel.current_sequence, channel.last_sequence
    );
    channel.set_resend_list();
    for seq in &channel.resend_list {
        print!("not received:{0} \n", seq);
    }
}

fn send_receive(update: bool,send: bool,channel_id: u8, message_type: u8, client: &mut Tachyon, server: &mut Tachyon,  send_buffer: &mut [u8], receive_buffer: &mut [u8],
     send_message_size: usize, client_remote: &mut NetworkAddress) {
    let client_address = NetworkAddress::default();
    //let mut client_remote = NetworkAddress::default();

    if send {
        for _ in 0..4 {
            let send_result: TachyonSendResult;
            let target = SendTarget {address: client_address, identity_id: 0};
            if message_type == MESSAGE_TYPE_RELIABLE {
                send_result = client.send_to_target(channel_id, target, send_buffer, send_message_size);
            } else {
                send_result = client.send_to_target(0,target, send_buffer, send_message_size);
            }
            assert_eq!(0, send_result.error);
            assert!(send_result.sent_len > 0);
        }
        
    }

    let receive_result = server.receive_loop(receive_buffer);
    assert_eq!(0, receive_result.error);

    if send {
        if receive_result.length > 0 {
            assert!(receive_result.address.port > 0);
            client_remote.copy_from(receive_result.address);
            if client_remote.port > 0 {
                let target = SendTarget {address: *client_remote, identity_id: 0};
                if message_type == MESSAGE_TYPE_RELIABLE {
                    server.send_to_target(channel_id,target,send_buffer,send_message_size);
                } else {
                    server.send_to_target(0,target, send_buffer, send_message_size);
                }
            }
        }
    }

    for _ in 0..100 {
        let receive_result = server.receive_loop(receive_buffer);
        if receive_result.length == 0 {
            break;
        }
    }

    for _ in 0..100 {
        let receive_result = client.receive_loop(receive_buffer);
        if receive_result.length == 0 {
            break;
        }
    }

    if update {
        client.update();
        server.update();
    }
}


#[test]
#[serial]
fn general_stress() {
    let address = NetworkAddress::test_address();
    let client_address = NetworkAddress::default();

    let sleep_in_loop = false;
    let buffer_size =  64 * 1024;
    let drop_reliable_only = 0;
    let client_drop_chance = 0;
    let server_drop_chance = 0;
    let loop_count = 2000; // inner loop sends 4 messages
    let channel_id = 1;
    let message_type = MESSAGE_TYPE_RELIABLE;
    //let message_type = MESSAGE_TYPE_UNRELIABLE;
    let send_message_size = 384;

    let mut send_buffer: Vec<u8> = vec![0; buffer_size];
    let mut receive_buffer: Vec<u8> = vec![0; buffer_size];

    let mut config = TachyonConfig::default();
    config.drop_packet_chance = server_drop_chance;
    config.drop_reliable_only = drop_reliable_only;

    let mut server = Tachyon::create(config);
    server.bind(address);

    let mut config = TachyonConfig::default();
    config.drop_packet_chance = client_drop_chance;
    config.drop_reliable_only = drop_reliable_only;
    let mut client = Tachyon::create(config);
    client.connect(address);

    let mut client_remote = NetworkAddress::default();
    for _ in 0..loop_count {

        if sleep_in_loop {
            thread::sleep(time::Duration::from_millis(10));
            send_receive(true,true,channel_id, message_type, &mut client, &mut server, &mut send_buffer, &mut receive_buffer, send_message_size, &mut client_remote);
        } else {
            send_receive(true,true,channel_id, message_type, &mut client, &mut server, &mut send_buffer, &mut receive_buffer, send_message_size, &mut client_remote);
        }
        
        
        
    }

    for _ in 0..200 {
        send_receive(true,false,channel_id,message_type,&mut client,&mut server, &mut send_buffer, &mut receive_buffer, send_message_size, &mut client_remote);
    }

    let channel = client.get_channel(client_address, channel_id).unwrap();
    channel.receiver.publish();
    channel.receiver.set_resend_list();
    channel.update_stats();
    print!(
        "CLIENT {0} current_seq:{1} last_seq:{2}, missing:{3}\n",
        channel.stats,
        channel.receiver.current_sequence,
        channel.receiver.last_sequence,
        channel.receiver.resend_list.len()
    );

    match server.get_channel(client_remote, channel_id) {
        Some(channel) => {
            channel.receiver.publish();
            channel.receiver.set_resend_list();
            channel.update_stats();
            print!(
                "SERVER {0} current_seq:{1} last_seq:{2} missing:{3}\n",
                channel.stats,
                channel.receiver.current_sequence,
                channel.receiver.last_sequence,
                channel.receiver.resend_list.len()
            );
        }
        None => {}
    }

    print!(
        "Dropped client:{0} server:{1}\n\n",
        client.stats.packets_dropped, server.stats.packets_dropped
    );

    print!(
        "Unreliable client sent:{0} received:{1}\n\n",
        client.stats.unreliable_sent, client.stats.unreliable_received
    );
    print!(
        "Unreliable server sent:{0} received:{1}\n\n",
        server.stats.unreliable_sent, server.stats.unreliable_received
    );

    match server.get_channel(client_remote, channel_id) {
        Some(channel) => {
            show_channel_debug(&mut channel.receiver);
        }
        None => {}
    }
}

#[test]
#[serial]
fn many_clients() {
    let address = NetworkAddress::test_address();

    let client_count = 200;
    let loop_count = 4;
    let channel_id = 1;
    let message_type = MESSAGE_TYPE_RELIABLE;
    //let message_type = MESSAGE_TYPE_UNRELIABLE;

    let msize = 384;
    let mut send_buffer: Vec<u8> = vec![0; 1024];
    let mut receive_buffer: Vec<u8> = vec![0; 4096];

    let mut config = TachyonConfig::default();
    config.drop_packet_chance = 0;

    let mut server = Tachyon::create(config);
    server.bind(address);

    let mut clients: Vec<Tachyon> = Vec::new();

    for _ in 0..client_count {
        let mut client = Tachyon::create(config);
        client.connect(address);
        clients.push(client);
    }

    for mut client in &mut clients {
        let mut client_remote = NetworkAddress::default();
        for _ in 0..loop_count {
            send_receive(false,true,channel_id, message_type, &mut client, &mut server,&mut send_buffer, &mut receive_buffer,msize, &mut client_remote);
        }
    }

    let now = Instant::now();

    for _ in 0..10000 {
        let _receive_result = server.receive_loop(&mut receive_buffer);
        if _receive_result.length == 0 || _receive_result.error > 0 {
            break;
        }
    }

    for _ in 0..100 {
        server.update();
    }
    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);
    print!("Server CombinedStats:{0}\n", server.get_combined_stats());
}
