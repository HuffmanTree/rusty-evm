use ethnum::u256;

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

    pub fn store(&mut self, offset: usize, mut value: u256) -> (usize, usize) {
        let extension_size = self.extension_size(offset, 32);
        self.arr.append(&mut vec![0; extension_size]);

        let mut i = 32_usize;
        while i > 0 {
            self.arr[offset + i - 1] = (value & 0xFF).try_into().unwrap();
            value >>= 8;
            i -= 1;
        }
        (extension_size, Memory::extension_cost(extension_size))
    }

    pub fn load(&mut self, offset: usize) -> (usize, usize, u256) {
        let extension_size = self.extension_size(offset, 32);
        self.arr.append(&mut vec![0; extension_size]);

        let mut res = u256::from(0_u8);
        let mut i = 0_usize;

        while i < 32 {
            res <<= 8;
            res |= u256::from(match self.arr.get(offset + i) {
                Some(v) => v.clone(),
                None => 0,
            });
            i += 1;
        }

        (extension_size, Memory::extension_cost(extension_size), res)
    }

    pub fn access(&self, offset: usize, size: usize) -> Vec<u8> {
        let mut res = Vec::<u8>::new();

        for i in 0..size {
            res.push(self.arr.get(offset + i).unwrap_or(&0_u8).clone());
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
        assert_eq!(memory.store(4, uint!("0x0000000000000000000000000000000000000000000000000000000004050607")), (64, 6));
        assert_eq!(memory.arr, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn loads_32_bytes_padded_with_zeros() {
        let mut memory = Memory { arr: vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] };

        assert_eq!(memory.load(6).2, uint!("0x0607000000000000000000000000000000000000000000000000000000000000"));
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
}
