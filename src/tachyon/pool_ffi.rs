
use crate::tachyon::*;
use super::pool::{Pool, PoolServerRef, OutBufferCounts};

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
pub extern "C" fn pool_create_server(pool_ptr: *mut Pool, config_ptr: *const TachyonConfig, naddress: *const NetworkAddress, id: *mut u16) -> *mut Tachyon {
    let pool = unsafe { &mut *pool_ptr };
    let config: TachyonConfig = unsafe { std::ptr::read(config_ptr as *const _) };
    let address: NetworkAddress = unsafe { std::ptr::read(naddress as *const _) };
    match pool.create_server(config, address) {
        Some(server) => {
            unsafe {
                (*id) = server.id;
            }
            return server;
        }
        None => return std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn pool_get_server(pool_ptr: *mut Pool, id: u16) -> *mut Tachyon {
    let pool = unsafe { &mut *pool_ptr };

    match pool.get_server(id) {
        Some(server) => {
            return server;
        }
        None => return std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn pool_get_available(pool_ptr: *mut Pool, naddress: *mut PoolServerRef) -> i32 {
    let pool = unsafe { &mut *pool_ptr };

    match pool.get_available_server() {
        Some(server) => {
            unsafe {
                (*naddress) = server;
            }
            return 1;
        }
        None => return -1,
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