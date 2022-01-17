
use crate::tachyon::*;

use super::pool::Pool;

#[no_mangle]
pub extern "C" fn null_test() -> *mut Pool {
    return std::ptr::null_mut();
}

#[no_mangle]
pub extern "C" fn create_pool() -> *mut Pool {
    let pool = Pool::create();
    let b = Box::new(pool);
    return Box::into_raw(b);
}

#[no_mangle]
pub extern "C" fn destroy_pool(pool: *mut Pool) {
    if !pool.is_null() {
        let _b = unsafe {Box::from_raw(pool)};
    }
}

#[no_mangle]
pub extern "C" fn create_pool_server(pool_ptr: *mut Pool, config_ptr: *const TachyonConfig, naddress: *const NetworkAddress) -> *mut Tachyon {
    let pool = unsafe {&mut*pool_ptr};
    let config: TachyonConfig = unsafe { std::ptr::read(config_ptr as *const _) };
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    match pool.create_server(config, address) {
        Some(server) => {return server;},
        None => return std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn get_pool_server(pool_ptr: *mut Pool, id: u16) -> *mut Tachyon {
    let pool = unsafe {&mut*pool_ptr};
    
    match pool.get_server(id) {
        Some(server) => {return server;},
        None => return std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn create_tachyon(config_ptr: *const TachyonConfig) -> *mut Tachyon {
    let config: TachyonConfig = unsafe { std::ptr::read(config_ptr as *const _) };
    let tachyon = Tachyon::create(config);
    let b = Box::new(tachyon);
    return Box::into_raw(b);
}

#[no_mangle]
pub extern "C" fn destroy_tachyon(tachyon: *mut Tachyon) {
    if !tachyon.is_null() {
        let _b = unsafe {Box::from_raw(tachyon)};
    }
}

#[no_mangle]
pub extern "C" fn bind_socket(tachyon_ptr: *mut Tachyon, naddress: *const NetworkAddress) -> i32 {
    let tachyon = unsafe {&mut*tachyon_ptr};
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    match tachyon.bind(address) {
        true => return 1,
        false => return -1,
    }
}

#[no_mangle]
pub extern "C" fn connect_socket(tachyon_ptr: *mut Tachyon, naddress: *const NetworkAddress) -> i32 {
    let tachyon = unsafe {&mut*tachyon_ptr};
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    match tachyon.connect(address) {
        true => return 1,
        false => return -1,
    }
}

#[no_mangle]
pub extern "C" fn configure_channel(tachyon_ptr: *mut Tachyon, channel_id: u8, ordered: u8) -> i32 {
    let tachyon = unsafe {&mut*tachyon_ptr};
    let res = tachyon.configure_channel(channel_id, ordered == 1);
    if res {
        return 1;
    } else {
        return -1;
    }
}

fn copy_send_result(from: TachyonSendResult, to: *mut TachyonSendResult) {
    unsafe {
        (*to).sent_len = from.sent_len;
        (*to).error = from.error;
        (*to).header = from.header;
    }
}

#[no_mangle]
pub extern "C" fn send_unreliable(tachyon_ptr: *mut Tachyon, data: *mut u8, length: i32, ret: *mut TachyonSendResult) {
    let tachyon = unsafe {&mut*tachyon_ptr};
    let address = NetworkAddress::default();
    let slice = unsafe {std::slice::from_raw_parts_mut(data, length as usize)};

    let result = tachyon.send_unreliable(address, slice, length as usize);
    copy_send_result(result, ret);
}

#[no_mangle]
pub extern "C" fn send_unreliable_to(tachyon_ptr: *mut Tachyon, naddress: *const NetworkAddress, data: *mut u8, length: i32, ret: *mut TachyonSendResult) {
    let tachyon = unsafe {&mut*tachyon_ptr};
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    let slice = unsafe {std::slice::from_raw_parts_mut(data, length as usize)};
    
    let result = tachyon.send_unreliable(address, slice, length as usize);
    copy_send_result(result, ret);
}

#[no_mangle]
pub extern "C" fn send_reliable(tachyon_ptr: *mut Tachyon, channel: u8, data: *mut u8, length: i32, ret: *mut TachyonSendResult) {
    let tachyon = unsafe {&mut*tachyon_ptr};
    let address = NetworkAddress::default();
    let slice = unsafe {std::slice::from_raw_parts_mut(data, length as usize)};

    let result = tachyon.send_reliable(channel,address, slice, length as usize);
    copy_send_result(result, ret);
}


#[no_mangle]
pub extern "C" fn send_reliable_to(tachyon_ptr: *mut Tachyon, channel: u8, naddress: *const NetworkAddress, data: *mut u8, length: i32, ret: *mut TachyonSendResult) {
    let tachyon = unsafe {&mut*tachyon_ptr};
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    let slice = unsafe {std::slice::from_raw_parts_mut(data, length as usize)};
    
    let result = tachyon.send_reliable(channel,address, slice, length as usize);
    copy_send_result(result, ret);
}


#[no_mangle]
pub extern "C" fn receive(tachyon_ptr: *mut Tachyon, data: *mut u8, receive_buffer_len: u32, ret: *mut TachyonReceiveResult) {
    let tachyon = unsafe {&mut*tachyon_ptr};
    let slice = unsafe {std::slice::from_raw_parts_mut(data, receive_buffer_len as usize)};
    let result = tachyon.receive_loop(slice);

    unsafe {
        (*ret).address = result.address;
        (*ret).length = result.length;
        (*ret).error = result.error;
    }
}

#[no_mangle]
pub extern "C" fn tachyon_update(tachyon_ptr: *mut Tachyon) {
    let tachyon = unsafe {&mut*tachyon_ptr};
    tachyon.update();
}

#[no_mangle]
pub extern "C" fn get_connections(tachyon_ptr: *mut Tachyon, addresses: *mut Connection, max: u16) -> u16 {
    let tachyon = unsafe {&mut*tachyon_ptr};
    
    let list = tachyon.get_connections(max);
    
    unsafe {
        std::ptr::copy_nonoverlapping(list.as_ptr(), addresses, list.len());
    }
    
    return list.len() as u16;
}

#[no_mangle]
pub extern "C" fn get_stats(tachyon_ptr: *mut Tachyon, stats: *mut TachyonStats) {
    let tachyon = unsafe {&mut*tachyon_ptr};
    let combined = tachyon.get_combined_stats();
    unsafe {
        (*stats).channel_stats = combined.channel_stats;
        (*stats).packets_dropped = combined.packets_dropped;
        (*stats).unreliable_sent = combined.unreliable_sent;
        (*stats).unreliable_received = combined.unreliable_received;
    }
}
