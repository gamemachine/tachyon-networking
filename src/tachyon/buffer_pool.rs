use std::collections::VecDeque;

pub const BYTE_BUFFER_SIZE_DEFAULT: usize = 1240;
const POOL_SIZE_DEFAULT: usize = 512;


pub struct ByteBuffer {
    data: Vec<u8>,
    pub length: usize,
    pub pooled: bool,
    pub version: u64
}

impl ByteBuffer {
    pub fn create(length: usize) -> Self {
        let byte_buffer = ByteBuffer {
            data: vec![0;length],
            length: length,
            pooled: false,
            version: 0
        };
        return byte_buffer;
    }

    pub fn get(&self) -> &[u8] {
        return &self.data;
    }

    pub fn get_mut(&mut self) -> &mut [u8] {
        return &mut self.data;
    }
}

impl<Idx> std::ops::Index<Idx> for ByteBuffer
where
    Idx: std::slice::SliceIndex<[u8]>,
{
    type Output = Idx::Output;

    fn index(&self, index: Idx) -> &Self::Output {
        &self.data[index]
    }
}

impl<Idx> std::ops::IndexMut<Idx> for ByteBuffer
where
    Idx: std::slice::SliceIndex<[u8]>
{
    fn index_mut(&mut self, index: Idx) -> &mut Self::Output {
        &mut self.data[index]
    }
}

pub struct ByteBufferPool {
    pub buffer_size: usize,
    pool_size: usize,
    buffers: VecDeque<ByteBuffer>,
    count: usize
}

impl ByteBufferPool {

    pub fn default() -> Self {
        let pool = ByteBufferPool::create(BYTE_BUFFER_SIZE_DEFAULT, POOL_SIZE_DEFAULT);
        return pool;
    }

    pub fn create(buffer_size: usize, max_buffers: usize) -> Self {
        let pool = ByteBufferPool {
            buffer_size,
            pool_size: max_buffers,
            buffers: VecDeque::new(),
            count: 0
        };
        return pool;
    }

    pub fn len(&self) -> usize {
        return self.buffers.len();
    }

    pub fn return_buffer(&mut self, mut byte_buffer: ByteBuffer) -> bool {
        if byte_buffer.length <= self.buffer_size {
            if self.count < self.pool_size {
                byte_buffer.version += 1;
                self.buffers.push_back(byte_buffer);
                self.count += 1;
                return true;
            }
        }
        return false;
    }

    pub fn get_buffer(&mut self, length: usize) -> ByteBuffer {
        if length > self.buffer_size {
            let buffer = ByteBuffer {
                data: vec![0; length],
                length: length,
                pooled: false,
                version: 0
            };
            return buffer;
        }

        match self.buffers.pop_front() {
            Some(mut pooled) => {
                self.count -= 1;
                //pooled.data[0..length].fill(0);
                pooled.length = length;
                return pooled;
            },
            None => {
                let data: Vec<u8> = vec![0; self.buffer_size];
                let buffer = ByteBuffer {
                    data,
                    length: length,
                    pooled: true,
                    version: 0
                };
                return buffer;
            },
        }
    } 
}


#[cfg(test)]
mod tests {

    use crate::tachyon::buffer_pool::{ByteBuffer, POOL_SIZE_DEFAULT, BYTE_BUFFER_SIZE_DEFAULT};

    use super::ByteBufferPool;

    #[test]
    fn get_return_within_limits() {
        let mut pool = ByteBufferPool::default();

        let buffer = pool.get_buffer(BYTE_BUFFER_SIZE_DEFAULT);
        assert!(pool.return_buffer(buffer));
        assert_eq!(1, pool.len());
    }

    #[test]
    fn get_allocates_over_max() {
        let mut pool = ByteBufferPool::default();

        let buffer = pool.get_buffer(BYTE_BUFFER_SIZE_DEFAULT);
        pool.return_buffer(buffer);
       pool.get_buffer(BYTE_BUFFER_SIZE_DEFAULT + 1);
       assert_eq!(1, pool.len());
    }

    #[test]
    fn will_not_return_over_max_buffer_size() {
        let mut pool = ByteBufferPool::default();

       let buffer = pool.get_buffer(BYTE_BUFFER_SIZE_DEFAULT + 1);
       assert!(!pool.return_buffer(buffer));
       assert_eq!(0, pool.len());
    }

    #[test]
    fn will_not_return_if_full() {
        let mut pool = ByteBufferPool::default();

        for _ in 0..POOL_SIZE_DEFAULT {
            let buffer = ByteBuffer::create(1024);
            assert!(pool.return_buffer(buffer));
        }
 
         let buffer = ByteBuffer::create(1024);
         assert!(!pool.return_buffer(buffer));
    }


}
