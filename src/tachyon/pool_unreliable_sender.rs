use rustc_hash::FxHashMap;

use super::{
    pool::{Pool, SendTarget}, TachyonSendResult, network_address::NetworkAddress, connection::Connection, unreliable_sender::UnreliableSender, ffi::copy_send_result
};

pub struct PoolUnreliableSender {
    pub identity_to_conn_map: FxHashMap<u32, Connection>,
    pub address_to_conn_map: FxHashMap<NetworkAddress, Connection>,
    pub senders: FxHashMap<u16, UnreliableSender>
}

impl PoolUnreliableSender {
    pub fn create() -> Self {
        PoolUnreliableSender {
            identity_to_conn_map: FxHashMap::default(),
            address_to_conn_map: FxHashMap::default(),
            senders: FxHashMap::default()
        }
    }

    pub fn send_to_target(&mut self, target: SendTarget, data: &mut [u8], length: i32) -> TachyonSendResult {
        if target.identity_id > 0 {
            return self.send_to_identity(target.identity_id, data, length);
        } else {
            return self.send_to_address(target.address, data, length);
        }
    }

    fn send_to_identity(&mut self, id: u32, data: &mut [u8], length: i32) -> TachyonSendResult {
        if let Some(conn) = self.identity_to_conn_map.get(&id) {
            if let Some(sender) = self.senders.get_mut(&conn.tachyon_id) {
                return sender.send(conn.address, data, length as usize);
            }
        }
        return TachyonSendResult::default();
    }

    fn send_to_address(&mut self, address: NetworkAddress, data: &mut [u8], length: i32) -> TachyonSendResult {
        if let Some(conn) = self.address_to_conn_map.get(&address) {
            if let Some(sender) = self.senders.get_mut(&conn.tachyon_id) {
                return sender.send(address, data, length as usize);
            }
        }
        return TachyonSendResult::default();
    }

    pub fn build(&mut self, pool: &mut Pool) {
        self.address_to_conn_map.clear();
        self.identity_to_conn_map.clear();
        self.senders.clear();
        
        for server in pool.servers.values() {
            if !self.senders.contains_key(&server.id) {
                if let Some(sender) = server.create_unreliable_sender() {
                    self.senders.insert(server.id, sender);
                }
            }
            for conn in server.connections.values() {
                self.address_to_conn_map.insert(conn.address, *conn);
                if conn.identity.id > 0 {
                    self.identity_to_conn_map.insert(conn.identity.id, *conn);
                }
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn pool_unreliable_sender_create(pool_ptr: *mut Pool) -> *mut PoolUnreliableSender {
    let pool = unsafe { &mut *pool_ptr };
    let mut sender = PoolUnreliableSender::create();
    sender.build(pool);

    let b = Box::new(sender);
    return Box::into_raw(b);
}

#[no_mangle]
pub extern "C" fn pool_unreliable_sender_destroy(pool: *mut PoolUnreliableSender) {
    if !pool.is_null() {
        let _b = unsafe { Box::from_raw(pool) };
    }
}

#[no_mangle]
pub extern "C" fn pool_unreliable_sender_build(pool_ptr: *mut Pool, sender_ptr: *mut PoolUnreliableSender) {
    let pool = unsafe { &mut *pool_ptr };
    let sender = unsafe { &mut *sender_ptr };
    sender.build(pool)
}

#[no_mangle]
pub extern "C" fn pool_unreliable_sender_send(sender_ptr: *mut PoolUnreliableSender, target_ptr: *const SendTarget,  data_ptr: *mut u8, length: i32, ret: *mut TachyonSendResult) {
    let sender = unsafe { &mut *sender_ptr };
    let target: SendTarget = unsafe { std::ptr::read(target_ptr as *const _) };
    let data = unsafe { std::slice::from_raw_parts_mut(data_ptr, length as usize) };

    let result =  sender.send_to_target(target, data, length);
    copy_send_result(result, ret);
}
