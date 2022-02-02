use std::alloc::{alloc_zeroed, Layout, dealloc};


#[repr(C)]
pub struct MemoryBlock {
    pub memory: *mut u8,
    pub length: u32
}

impl MemoryBlock {

    pub fn allocate(length: u32) -> Option<MemoryBlock> {
        match Layout::array::<u8>(length as usize) {
            Ok(layout) => {
                unsafe {
                    let block = MemoryBlock {
                        memory: alloc_zeroed(layout),
                        length:  length
                    };
                    return Some(block);
                }
            },
            Err(_) => {
                return None;
            },
        }
    }

    pub fn free(block: MemoryBlock) -> i32 {
        if block.length == 0 {
            return -3;
        }
    
        match Layout::array::<u8>(block.length as usize) {
            Ok(layout) => {
                unsafe {
                    dealloc(block.memory, layout);
                    return 1;
                }
            },
            Err(_) => {
                return -1;
            },
        }
    }
}

#[no_mangle]
pub extern "C" fn allocate_memory_block(length: u32, block_ptr: *mut MemoryBlock) -> i32 {
    match MemoryBlock::allocate(length) {
        Some(block) => {
            unsafe {
                (*block_ptr) = block;
                return 1;
            }
        },
        None => {
            return -1;
        },
    }
}

#[no_mangle]
pub extern "C" fn free_memory_block(block_ptr: *mut MemoryBlock) -> i32 {
    if block_ptr.is_null() {
        return -2;
    }
    let block: MemoryBlock = unsafe { std::ptr::read(block_ptr as *mut _) };
    return MemoryBlock::free(block);
}