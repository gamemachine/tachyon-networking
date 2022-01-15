
use crate::tachyon::*;


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
    return tachyon.bind(address);
}

#[no_mangle]
pub extern "C" fn connect_socket(tachyon_ptr: *mut Tachyon, naddress: *const NetworkAddress) -> i32 {
    let tachyon = unsafe {&mut*tachyon_ptr};
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    return tachyon.connect(address);
}

#[no_mangle]
pub extern "C" fn create_channel(tachyon_ptr: *mut Tachyon, naddress: *const NetworkAddress, channel_id: u8, ordered: u8) -> u8 {
    let tachyon = unsafe {&mut*tachyon_ptr};
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };

    let created = tachyon.create_channel(address, channel_id, ordered == 1);
    if created {
        return 1;
    } else {
        return 0;
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
pub extern "C" fn get_client_addresses(tachyon_ptr: *mut Tachyon, addresses: *mut NetworkAddress, max: u16) -> u16 {
    let tachyon = unsafe {&mut*tachyon_ptr};
    
    let list = tachyon.get_address_list(max);
    
    unsafe {
        std::ptr::copy_nonoverlapping(list.as_ptr(), addresses, list.len());
    }
    
    return list.len() as u16;
}

#[no_mangle]
pub extern "C" fn get_stats(tachyon_ptr: *mut Tachyon, stats: *mut TachyonStats) -> i32 {
    let tachyon = unsafe {&mut*tachyon_ptr};
    let combined = tachyon.get_combined_stats();
    unsafe {
        (*stats).channel_stats = combined.channel_stats;
        (*stats).packets_dropped = combined.packets_dropped;
        (*stats).unreliable_sent = combined.unreliable_sent;
        (*stats).unreliable_received = combined.unreliable_received;
    }
    return 0;
}
