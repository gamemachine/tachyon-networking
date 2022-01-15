
use std::time::Instant;

use crate::tachyon::*;
use crate::tachyon::receiver::*;
use crate::tachyon::header::*;



pub fn show_channel_debug(channel: &mut Receiver) {
    let start = channel.current_sequence;
    let end = channel.last_sequence;
    print!("current:{0} last:{1}\n", channel.current_sequence, channel.last_sequence);
    channel.set_resend_list();
    for seq in &channel.resend_list {
        print!("not received:{0} \n", seq);
    }
    
}

fn send_receive(update: bool, send: bool, channel_id: u8, message_type: u8, client: &mut Tachyon, server: &mut Tachyon, send_buffer: &mut [u8], receive_buffer: &mut [u8],
     send_message_size: usize, client_remote: &mut NetworkAddress) {

    let client_address = NetworkAddress::default();
    //let mut client_remote = NetworkAddress::default();

    if send {
        let send_result: TachyonSendResult;
        if message_type == MESSAGE_TYPE_RELIABLE {
            send_result = client.send_reliable(channel_id, client_address, send_buffer, send_message_size);
        } else {
            send_result = client.send_unreliable(client_address, send_buffer, send_message_size);
        }
        assert_eq!(0,send_result.error);
        assert!(send_result.sent_len > 0);
    }
    
    
    
    let receive_result = server.receive_loop(receive_buffer);
    assert_eq!(0, receive_result.error);

    if send {
        if receive_result.length > 0 {
            assert!(receive_result.address.port > 0);
            client_remote.copy_from(receive_result.address);
            if client_remote.port > 0 {
                if message_type == MESSAGE_TYPE_RELIABLE {
                    server.send_reliable(channel_id, *client_remote, send_buffer, send_message_size);
                } else {
                    server.send_unreliable( *client_remote, send_buffer, send_message_size);
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
fn general_stress() {
    let address = NetworkAddress::test_address();
    let client_address = NetworkAddress::default();

    let drop_reliable_only = 0;
    let client_drop_chance = 2;
    let server_drop_chance = 2;
    let loop_count = 20000;
    let channel_id = 1;
    let message_type = MESSAGE_TYPE_RELIABLE;
    //let message_type = MESSAGE_TYPE_UNRELIABLE;
    let send_message_size = 32;
    
    let mut send_buffer: Vec<u8> = vec![0;2048];
    let mut receive_buffer: Vec<u8> = vec![0;4096];

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
        send_receive(true,true,channel_id, message_type, &mut client, &mut server, &mut send_buffer, &mut receive_buffer, send_message_size, &mut client_remote);
    }

    for _ in 0..200 {
        send_receive(true,false,channel_id, message_type, &mut client, &mut server, &mut send_buffer, &mut receive_buffer, send_message_size, &mut client_remote);
    }
   


    let channel = client.get_channel(client_address, channel_id).unwrap();
    channel.receiver.publish();
    channel.receiver.set_resend_list();
    channel.update_stats();
    print!("CLIENT {0} current_seq:{1} last_seq:{2}, missing:{3}\n", channel.stats, channel.receiver.current_sequence, channel.receiver.last_sequence, channel.receiver.resend_list.len());
    

    match server.get_channel(client_remote, channel_id) {
        Some(channel) => {
            channel.receiver.publish();
            channel.receiver.set_resend_list();
            channel.update_stats();
            print!("SERVER {0} current_seq:{1} last_seq:{2} missing:{3}\n",
            channel.stats, channel.receiver.current_sequence, channel.receiver.last_sequence,channel.receiver.resend_list.len());
        },
        None => {},
    }

    print!("Dropped client:{0} server:{1}\n\n", client.stats.packets_dropped, server.stats.packets_dropped);

    print!("Unreliable client sent:{0} received:{1}\n\n", client.stats.unreliable_sent, client.stats.unreliable_received);
    print!("Unreliable server sent:{0} received:{1}\n\n", server.stats.unreliable_sent, server.stats.unreliable_received);

    match server.get_channel(client_remote, channel_id) {
        Some(channel) => {
            show_channel_debug(&mut channel.receiver);
        },
        None => {},
    }

    println!("socket send_time:{0} receive_time:{1}\n", server.counters.socket_send_time / 1000,server.counters.socket_receive_time / 1000);
    
  
}

#[test]
fn many_clients() {
    let address = NetworkAddress::test_address();
    let client_address = NetworkAddress::default();

    let channel_id = 1;
    let message_type = MESSAGE_TYPE_RELIABLE;
    //let message_type = MESSAGE_TYPE_UNRELIABLE;

    let msize = 32;
    let mut send_buffer: Vec<u8> = vec![0;1024];
    let mut receive_buffer: Vec<u8> = vec![0;4096];

    let mut config = TachyonConfig::default();
    //config.drop_packet_chance = 1;
    
    let mut server = Tachyon::create(config);
    server.bind(address);

    let mut clients: Vec<Tachyon> = Vec::new();

    for _ in 0..2000 {
        let mut client = Tachyon::create(config);
        client.connect(address);
        clients.push(client);
    }
    
    for mut client in &mut clients {
        let mut client_remote = NetworkAddress::default();
        for _ in 0..40 {
            send_receive(false,true,channel_id, message_type, &mut client, &mut server, &mut send_buffer, &mut receive_buffer, msize, &mut client_remote);
        }
    }
    
    let now = Instant::now();
    for _ in 0..100 {
        let receive_result = server.receive_loop(&mut receive_buffer);
        server.update();
    }
    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);
    println!("socket send_time:{0} receive_time:{1}\n", server.counters.socket_send_time / 1000,server.counters.socket_receive_time / 1000);
    print!("CombinedStats:{0}\n", server.get_combined_stats());

}