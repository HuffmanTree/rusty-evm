use ethnum::u256;

pub struct Memory {
    arr: Vec<u8>,
}

impl Memory {
    pub fn new() -> Self {
        Self { arr: Vec::<u8>::new() }
    }

    fn extension_cost(extension_size: usize) -> usize {
        let memory_size_word = (extension_size + 31) / 32;
        memory_size_word.pow(2) / 512 + (3 * memory_size_word)
    }

    fn store(&mut self, offset: usize, value: Vec<u8>) -> (usize, usize) {
        let mut extension_size = 0_usize;
        if self.arr.is_empty() {
            self.arr = vec![0; 32];
            extension_size = 32;
        }
        if self.arr.len() < offset + value.len() {
            let s = offset + value.len() - self.arr.len();
            self.arr.append(&mut vec![0; s]);
            extension_size += s;
        }
        for (i, v) in value.iter().enumerate() {
            self.arr[offset + i] = v.clone();
        }
        (extension_size, Memory::extension_cost(extension_size))
    }

    fn load(&self, offset: usize) -> u256 {
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

        res
    }

    fn access(&self, offset: usize, size: usize) -> Vec<u8> {
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
        assert_eq!(memory.store(4, vec![4, 5, 6, 7]), (32, 3));
        assert_eq!(memory.arr, vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn allocates_the_necessary() {
        let mut memory = Memory { arr: vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] };

        assert_eq!(memory.store(30, vec![30, 31, 32, 33]), (2, 3));
        assert_eq!(memory.arr, vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 30, 31, 32, 33]);
    }

    #[test]
    fn loads_32_bytes_padded_with_zeros() {
        let memory = Memory { arr: vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] };

        assert_eq!(memory.load(6), uint!("0x0607000000000000000000000000000000000000000000000000000000000000"));
    }

    #[test]
    fn access_an_arbitrary_number_of_bytes() {
        let memory = Memory { arr: vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0] };

        assert_eq!(memory.access(2, 5), vec![0, 0, 4, 5, 6]);
        assert_eq!(memory.access(2, 10), vec![0, 0, 4, 5, 6, 7, 0, 0, 0, 0]);
    }
}
