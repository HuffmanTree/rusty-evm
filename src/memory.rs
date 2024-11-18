use ethnum::{u256, U256};

pub struct Memory {
    arr: Vec<u8>,
}

impl Memory {
    pub fn new() -> Self {
        Self { arr: Vec::<u8>::new() }
    }

    fn extension_size(&self, offset: usize, size: usize) -> usize {
        if self.size() >= offset + size { 0_usize } else { (((offset + size - self.size() - 1) >> 5) + 1) << 5 }
    }

    fn extension_cost(extension_size: usize) -> usize {
        let memory_size_word = (extension_size + 31) / 32;
        memory_size_word.pow(2) / 512 + (3 * memory_size_word)
    }

    pub fn size(&self) -> usize {
        self.arr.len()
    }

    pub fn store_byte(&mut self, offset: usize, value: u256) -> (usize, usize) {
        let extension_size = self.extension_size(offset, 1);
        self.arr.append(&mut vec![0; extension_size]);

        self.arr[offset] = (value & 0xFF).try_into().unwrap();
        (extension_size, Memory::extension_cost(extension_size))
    }

    pub fn store_word(&mut self, offset: usize, mut value: u256) -> (usize, usize) {
        let extension_size = self.extension_size(offset, 32);
        self.arr.append(&mut vec![0; extension_size]);

        for i in 0..32 {
            self.arr[offset + 31 - i] = (value & 0xFF).try_into().unwrap();
            value >>= 8;
        }
        (extension_size, Memory::extension_cost(extension_size))
    }

    pub fn load_word(&mut self, offset: usize) -> (usize, usize, u256) {
        let extension_size = self.extension_size(offset, 32);
        self.arr.append(&mut vec![0; extension_size]);

        let mut res = U256::ZERO;
        for i in 0..32 {
            res <<= 8;
            res |= u256::from(match self.arr.get(offset + i) {
                Some(v) => *v,
                None => 0,
            });
        }
        (extension_size, Memory::extension_cost(extension_size), res)
    }

    pub fn access(&self, offset: usize, size: usize) -> Vec<u8> {
        let mut res = Vec::<u8>::new();

        for i in 0..size {
            res.push(*self.arr.get(offset + i).unwrap_or(&0_u8));
        }

        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethnum::uint;

    #[test]
    fn allocates_32_bytes_if_memory_is_empty() {
        let mut memory = Memory::new();

        assert_eq!(memory.arr.len(), 0);
        assert_eq!(memory.store_word(4, uint!("0x0000000000000000000000000000000000000000000000000000000004050607")), (64, 6));
        assert_eq!(memory.arr, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn loads_32_bytes_padded_with_zeros() {
        let mut memory = Memory { arr: vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] };

        assert_eq!(memory.load_word(6).2, uint!("0x0607000000000000000000000000000000000000000000000000000000000000"));
    }

    #[test]
    fn access_an_arbitrary_number_of_bytes() {
        let memory = Memory { arr: vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0] };

        assert_eq!(memory.access(2, 5), vec![0, 0, 4, 5, 6]);
        assert_eq!(memory.access(2, 10), vec![0, 0, 4, 5, 6, 7, 0, 0, 0, 0]);
    }

    #[test]
    fn returns_the_current_size() {
        let memory = Memory { arr: vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0] };

        assert_eq!(memory.size(), 11);
    }

    #[test]
    fn computes_extension_size() {
        let memory = Memory { arr: vec![0; 32] };

        assert_eq!(memory.extension_size(0, 32), 0);
        assert_eq!(memory.extension_size(1, 32), 32);
        assert_eq!(memory.extension_size(31, 32), 32);
        assert_eq!(memory.extension_size(32, 32), 32);
        assert_eq!(memory.extension_size(33, 32), 64);
        assert_eq!(memory.extension_size(64, 32), 64);
        assert_eq!(memory.extension_size(65, 32), 96);
    }

    #[test]
    fn store_byte() {
        let mut memory = Memory::new();

        assert_eq!(memory.store_byte(2, uint!("0xFFAB")), (32, 3));
        assert_eq!(memory.arr, vec![0, 0, 0xAB, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        assert_eq!(memory.store_byte(32, uint!("0xFFAB")), (32, 3));
        assert_eq!(memory.arr, vec![0, 0, 0xAB, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xAB, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }
}
