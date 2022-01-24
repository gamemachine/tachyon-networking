pub struct SequenceBuffer<T> {
    pub values: Vec<Option<T>>,
    pub partition_by: u16,
}

impl<T> SequenceBuffer<T> {
    pub fn sequence_to_index(&self, sequence: u16) -> usize {
        return (sequence % self.partition_by) as usize;
    }

    pub fn insert(&mut self, sequence: u16, value: T) -> Option<&mut T> {
        let index = self.sequence_to_index(sequence);
        self.values[index] = Some(value);
        return self.values[index].as_mut();
    }

    pub fn remove(&mut self, sequence: u16) {
        let index = self.sequence_to_index(sequence);
        self.values[index] = None;
    }

    pub fn remove_at_index(&mut self, index: usize) {
        self.values[index] = None;
    }

    pub fn is_some(&self, sequence: u16) -> bool {
        let index = self.sequence_to_index(sequence);
        return self.values[index].is_some();
    }

    pub fn take(&mut self, sequence: u16) -> Option<T> {
        let index = self.sequence_to_index(sequence);
        return self.values[index].take();
    }

    pub fn get(&self, sequence: u16) -> Option<&T> {
        let index = self.sequence_to_index(sequence);
        match self.values.get(index) {
            Some(value) => {
                let value_ref = value.as_ref();
                return value_ref;
            }
            None => {
                return None;
            }
        }
    }

    pub fn get_at_index(&self, index: usize) -> Option<&T> {
        match self.values.get(index) {
            Some(value) => {
                let value_ref = value.as_ref();
                return value_ref;
            }
            None => {
                return None;
            }
        }
    }

    pub fn get_mut(&mut self, sequence: u16) -> Option<&mut T> {
        let index = self.sequence_to_index(sequence);
        match self.values.get_mut(index) {
            Some(value) => {
                let value_ref = value.as_mut();
                return value_ref;
            }
            None => {
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tachyon::sequence_buffer::SequenceBuffer;

    #[test]
    fn basic_test() {
        let data: Vec<u8> = vec![0; 32];
        let mut buffer: SequenceBuffer<Vec<u8>> = SequenceBuffer {
            values: vec![None; 1024],
            partition_by: 1024,
        };

        buffer.insert(1, data);
        let option = buffer.get(1);
        assert!(option.is_some());
    }
}
