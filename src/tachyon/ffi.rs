
use std::panic::catch_unwind;

use crate::tachyon::*;

#[no_mangle]
pub extern "C" fn register_callbacks(tachyon_ptr: *mut Tachyon, identity_event_callback: Option<IdentityEventCallback>,
     connection_event_callback: Option<ConnectionEventCallback>) {
    let tachyon = unsafe { &mut *tachyon_ptr };

    if identity_event_callback.is_some() {
        tachyon.identity_event_callback = identity_event_callback;
    }

    if connection_event_callback.is_some() {
        tachyon.connection_event_callback = connection_event_callback;
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
        let _b = unsafe { Box::from_raw(tachyon) };
    }
}

#[no_mangle]
pub extern "C" fn bind_socket(tachyon_ptr: *mut Tachyon, naddress: *const NetworkAddress) -> i32 {
    let tachyon = unsafe { &mut *tachyon_ptr };
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    match tachyon.bind(address) {
        true => return 1,
        false => return -1,
    }
}

#[no_mangle]
pub extern "C" fn connect_socket(
    tachyon_ptr: *mut Tachyon,
    naddress: *const NetworkAddress,
) -> i32 {
    let tachyon = unsafe { &mut *tachyon_ptr };
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    match tachyon.connect(address) {
        true => return 1,
        false => return -1,
    }
}

#[no_mangle]
pub extern "C" fn configure_channel(tachyon_ptr: *mut Tachyon, channel_id: u8, config_ptr: *const ChannelConfig) -> i32 {
    let tachyon = unsafe { &mut *tachyon_ptr };
    let channel_config = unsafe { &*config_ptr };
    let res = tachyon.configure_channel(channel_id, *channel_config);
    if res {
        return 1;
    } else {
        return -1;
    }
}

pub fn copy_send_result(from: TachyonSendResult, to: *mut TachyonSendResult) {
    unsafe {
        (*to).sent_len = from.sent_len;
        (*to).error = from.error;
        (*to).header = from.header;
    }
}

#[no_mangle]
pub extern "C" fn send_unreliable_to(tachyon_ptr: *mut Tachyon, naddress: *const NetworkAddress, data: *mut u8, length: i32, ret: *mut TachyonSendResult) {
    let tachyon = unsafe { &mut *tachyon_ptr };
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    let slice = unsafe { std::slice::from_raw_parts_mut(data, length as usize) };

    let result = tachyon.send_unreliable(address, slice, length as usize);
    copy_send_result(result, ret);
}

#[no_mangle]
pub extern "C" fn send_reliable_to(tachyon_ptr: *mut Tachyon, channel: u8, naddress: *const NetworkAddress, data: *mut u8, length: i32, ret: *mut TachyonSendResult) {
    let tachyon = unsafe { &mut *tachyon_ptr };
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    let slice = unsafe { std::slice::from_raw_parts_mut(data, length as usize) };

    let result = tachyon.send_reliable(channel, address, slice, length as usize);
    copy_send_result(result, ret);
}

#[no_mangle]
pub extern "C" fn receive(tachyon_ptr: *mut Tachyon, data: *mut u8, receive_buffer_len: u32, ret: *mut TachyonReceiveResult) {
    let tachyon = unsafe { &mut *tachyon_ptr };
    let slice = unsafe { std::slice::from_raw_parts_mut(data, receive_buffer_len as usize) };
    let result = tachyon.receive_loop(slice);

    unsafe {
        (*ret).channel = result.channel;
        (*ret).address = result.address;
        (*ret).length = result.length;
        (*ret).error = result.error;
    }
}

#[no_mangle]
pub extern "C" fn tachyon_update(tachyon_ptr: *mut Tachyon) {
    let tachyon = unsafe { &mut *tachyon_ptr };
    tachyon.update();
}

#[no_mangle]
pub extern "C" fn tachyon_get_connection(tachyon_ptr: *mut Tachyon, naddress: *const NetworkAddress, connection: *mut Connection) {
    let tachyon = unsafe { &mut *tachyon_ptr };
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    if let Some(conn) = tachyon.get_connection(address) {
        unsafe {
            (*connection) = *conn;
        }
    }
}

#[no_mangle]
pub extern "C" fn tachyon_get_connection_by_identity(tachyon_ptr: *mut Tachyon, id: u32, connection: *mut Connection) {
    let tachyon = unsafe { &mut *tachyon_ptr };
    if let Some(conn) = tachyon.get_connection_by_identity(id) {
        unsafe {
            (*connection) = *conn;
        }
    }
}

#[no_mangle]
pub extern "C" fn get_connections(tachyon_ptr: *mut Tachyon, connections: *mut Connection, max: i32) -> i32 {
    let tachyon = unsafe {&mut*tachyon_ptr};
    let list = tachyon.get_connections(max);
    
    if list.len() > 0 {

        unsafe {
            std::ptr::copy_nonoverlapping(list.as_ptr(), connections, list.len());
        }
    }
    
    
    return list.len() as i32;
}

#[no_mangle]
pub extern "C" fn tachyon_get_config(tachyon_ptr: *mut Tachyon, config: *mut TachyonConfig, identity: *mut Identity) {
    let tachyon = unsafe { &mut *tachyon_ptr };
    unsafe {
        (*config) = tachyon.config;
        (*identity) = tachyon.identity;
    }
}

#[no_mangle]
pub extern "C" fn set_identity(tachyon_ptr: *mut Tachyon, id: u32, session_id: u32, on_self: u32) {
    let tachyon = unsafe { &mut *tachyon_ptr };
    if on_self == 1 {
        tachyon.identity.id = id;
        tachyon.identity.session_id = session_id;
    } else {
        tachyon.set_identity(id, session_id);
    }
}

#[no_mangle]
pub extern "C" fn get_stats(tachyon_ptr: *mut Tachyon, stats: *mut TachyonStats) {
    let tachyon = unsafe { &mut *tachyon_ptr };
    let combined = tachyon.get_combined_stats();
    unsafe {
        (*stats).channel_stats = combined.channel_stats;
        (*stats).packets_dropped = combined.packets_dropped;
        (*stats).unreliable_sent = combined.unreliable_sent;
        (*stats).unreliable_received = combined.unreliable_received;
    }
}
