
use crate::tachyon::*;
use super::{pool::{Pool, PoolServerRef, OutBufferCounts}, ffi::copy_send_result};

#[no_mangle]
pub extern "C" fn pool_create(max_servers: u8, receive_buffer_len: u32, out_buffer_len: u32) -> *mut Pool {
    let pool = Pool::create(max_servers, receive_buffer_len, out_buffer_len);
    let b = Box::new(pool);
    return Box::into_raw(b);
}

#[no_mangle]
pub extern "C" fn pool_destroy(pool: *mut Pool) {
    if !pool.is_null() {
        let _b = unsafe { Box::from_raw(pool) };
    }
}

#[no_mangle]
pub extern "C" fn pool_create_server(pool_ptr: *mut Pool, config_ptr: *const TachyonConfig, naddress: *const NetworkAddress) -> u16 {
    let pool = unsafe { &mut *pool_ptr };
    let config: TachyonConfig = unsafe { std::ptr::read(config_ptr as *const _) };
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    match pool.create_server(config, address) {
        Some(server) => {
            return server.id;
        }
        None => return 0,
    }
}

#[no_mangle]
pub extern "C" fn pool_get_available(pool_ptr: *mut Pool, pool_ref_ptr: *mut PoolServerRef) -> i32 {
    let pool = unsafe { &mut *pool_ptr };

    match pool.get_available_server() {
        Some(pool_ref) => {
            unsafe {
                (*pool_ref_ptr) = pool_ref;
            }
            return 1;
        }
        None => return -1,
    }
}

#[no_mangle]
pub extern "C" fn pool_get_server_having_connection(pool_ptr: *mut Pool, naddress: *const NetworkAddress) -> u16 {
    let pool = unsafe { &mut *pool_ptr };
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    return pool.get_server_having_connection(address);
}

#[no_mangle]
pub extern "C" fn pool_get_server_having_identity(pool_ptr: *mut Pool, id: u32) -> u16 {
    let pool = unsafe { &mut *pool_ptr };

    return pool.get_server_having_identity(id);
}

#[no_mangle]
pub extern "C" fn pool_get_connections(pool_ptr: *mut Pool, connections: *mut Connection, max: i32) -> i32 {
    let pool = unsafe { &mut *pool_ptr };
    let mut list: Vec<Connection> = Vec::new();
    
    for server in pool.servers.values_mut() {
        let server_list = server.get_connections(max);
        for conn in server_list {
            list.push(conn);
        }
    }

    if list.len() > 0 {

        unsafe {
            std::ptr::copy_nonoverlapping(list.as_ptr(), connections, list.len());
        }
    }
    return list.len() as i32;
}

#[no_mangle]
pub extern "C" fn pool_update_servers(pool_ptr: *mut Pool) {
    let pool = unsafe { &mut *pool_ptr };
    for server in pool.servers.values_mut() {
        server.update();
    }
}

#[no_mangle]
pub extern "C" fn pool_receive_blocking(pool_ptr: *mut Pool) {
    let pool = unsafe { &mut *pool_ptr };
    pool.receive_blocking_out_buffer();
}

#[no_mangle]
pub extern "C" fn pool_get_next_out_buffer(pool_ptr: *mut Pool, receive_buffer_ptr: *mut u8, result: *mut OutBufferCounts) {
    let pool = unsafe { &mut *pool_ptr };
    let slice = unsafe { std::slice::from_raw_parts_mut(receive_buffer_ptr, pool.receive_buffer_len as usize) };
    let res = pool.get_next_out_buffer(slice);
    unsafe {
        (*result) = res;
    }
}

#[no_mangle]
pub extern "C" fn pool_receive(pool_ptr: *mut Pool) -> i32 {
    let pool = unsafe { &mut *pool_ptr };
    if pool.receive() {
        return 1;
    } else {
        return -1;
    }
}

#[no_mangle]
pub extern "C" fn pool_finish_receive(pool_ptr: *mut Pool) -> i32 {
    let pool = unsafe { &mut *pool_ptr };
    let result = pool.finish_receive();
    return result.1;
}

#[no_mangle]
pub extern "C" fn pool_send_reliable_to(pool_ptr: *mut Pool, channel: u8, naddress: *const NetworkAddress, data: *mut u8, length: i32, ret: *mut TachyonSendResult) {
    let pool = unsafe { &mut *pool_ptr };
    
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    let server_id =  pool.get_server_having_connection(address);
    if let Some(tachyon) = pool.get_server(server_id) {
        let slice = unsafe { std::slice::from_raw_parts_mut(data, length as usize) };

        let result = tachyon.send_reliable(channel, address, slice, length as usize);
        copy_send_result(result, ret);
    }
}

#[no_mangle]
pub extern "C" fn pool_send_unreliable_to(pool_ptr: *mut Pool, naddress: *const NetworkAddress, data: *mut u8, length: i32, ret: *mut TachyonSendResult) {
    let pool = unsafe { &mut *pool_ptr };
    
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    let server_id =  pool.get_server_having_connection(address);
    if let Some(tachyon) = pool.get_server(server_id) {
        let slice = unsafe { std::slice::from_raw_parts_mut(data, length as usize) };

        let result = tachyon.send_unreliable(address, slice, length as usize);
        copy_send_result(result, ret);
    }
}