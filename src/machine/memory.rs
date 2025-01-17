use ethnum::{u256, U256};
use crate::blockchain::errors::Error;

#[derive(Default, Debug)]
pub struct Memory(pub Vec<u8>);

#[derive(Debug, PartialEq, Eq)]
pub struct ReadWriteOperation<T> {
    offset: usize,
    pub size: usize,
    extension_size: usize,
    pub extension_cost: usize,
    pub result: T,
}

impl Memory {
    pub fn new() -> Self {
        Self(Default::default())
    }

    fn extension_size(&self, offset: usize, size: usize) -> usize {
        if self.size() >= offset + size { 0_usize } else { (((offset + size - self.size() - 1) >> 5) + 1) << 5 }
    }

    fn memory_cost(memory_byte_size: usize) -> usize {
        let memory_size_word = (memory_byte_size + 31) >> 5;
        (memory_size_word.pow(2) >> 9) + (3 * memory_size_word)
    }

    fn memory_expansion_cost(&self, last_memory_byte_size: usize) -> usize {
        let new_memory_byte_size = self.size();
        Memory::memory_cost(new_memory_byte_size) - Memory::memory_cost(last_memory_byte_size)
    }

    pub fn size(&self) -> usize {
        self.0.len()
    }

    fn try_offset_size(offset: u256, size: u256) -> Result<(usize, usize), Error> {
        let (offset, size): (usize, usize) = match (offset.try_into(), size.try_into()) {
            (Ok(offset), Ok(size)) => (offset, size),
            _ => return Err(Error::MemoryOutOfBounds),
        };
        if size == 0 || offset <= usize::MAX - size + 1 {
            Ok((offset, size))
        } else {
            Err(Error::MemoryOutOfBounds)
        }
    }

    pub fn store_byte(&mut self, offset: u256, value: u256) -> Result<ReadWriteOperation<()>, Error> {
        let (offset, size) = Memory::try_offset_size(offset, U256::ONE)?;
        let last_memory_byte_size = self.size();
        let extension_size = self.extension_size(offset, size);
        self.0.append(&mut vec![0; extension_size]);

        self.0[offset] = (value & 0xFF).try_into().unwrap();
        Ok(ReadWriteOperation::<()> { offset, size, extension_size, extension_cost: self.memory_expansion_cost(last_memory_byte_size), result: () })
    }

    pub fn store_word(&mut self, offset: u256, mut value: u256) -> Result<ReadWriteOperation<()>, Error> {
        let (offset, size) = Memory::try_offset_size(offset, u256::from(32_u8))?;
        let last_memory_byte_size = self.size();
        let extension_size = self.extension_size(offset, size);
        self.0.append(&mut vec![0; extension_size]);

        for i in 0..size {
            self.0[offset + 31 - i] = (value & 0xFF).try_into().unwrap();
            value >>= 8;
        }
        Ok(ReadWriteOperation::<()> { offset, size, extension_size, extension_cost: self.memory_expansion_cost(last_memory_byte_size), result: () })
    }

    pub fn store(&mut self, offset: u256, size: u256, value: Vec<u8>) -> Result<ReadWriteOperation<()>, Error> {
        let (offset, size) = Memory::try_offset_size(offset, size)?;
        let last_memory_byte_size = self.size();
        let extension_size = self.extension_size(offset, size);
        self.0.append(&mut vec![0; extension_size]);

        for i in 0..size {
            self.0[offset + i] = *value.get(i).unwrap_or(&0_u8);
        }
        Ok(ReadWriteOperation::<()> { offset, size, extension_size, extension_cost: self.memory_expansion_cost(last_memory_byte_size), result: () })
    }

    pub fn load_word(&mut self, offset: u256) -> Result<ReadWriteOperation<u256>, Error> {
        let (offset, size) = Memory::try_offset_size(offset, u256::from(32_u8))?;
        let last_memory_byte_size = self.size();
        let extension_size = self.extension_size(offset, size);
        self.0.append(&mut vec![0; extension_size]);

        let mut result = U256::ZERO;
        for i in 0..size {
            result <<= 8;
            result |= u256::from(match self.0.get(offset + i) {
                Some(v) => *v,
                None => 0,
            });
        }
        Ok(ReadWriteOperation::<u256> { offset, size, extension_size, extension_cost: self.memory_expansion_cost(last_memory_byte_size), result })
    }

    pub fn load(&mut self, offset: u256, size: u256) -> Result<ReadWriteOperation<Vec<u8>>, Error> {
        let (offset, size) = Memory::try_offset_size(offset, size)?;
        let last_memory_byte_size = self.size();
        let extension_size = self.extension_size(offset, size);
        self.0.append(&mut vec![0; extension_size]);

        let mut result = Vec::<u8>::new();
        for i in 0..size {
            result.push(*self.0.get(offset + i).unwrap_or(&0_u8));
        }
        Ok(ReadWriteOperation::<Vec<u8>> { offset, size, extension_size, extension_cost: self.memory_expansion_cost(last_memory_byte_size), result })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethnum::uint;

    #[test]
    fn stores_a_word() {
        let mut memory = Memory::new();

        assert_eq!(memory.0.len(), 0);
        assert_eq!(memory.store_word(uint!("4"), uint!("0x0000000000000000000000000000000000000000000000000000000004050607")), Ok(ReadWriteOperation::<()> {
            offset: 4,
            size: 32,
            extension_size: 64,
            extension_cost: 6,
            result: (),
        }));
        assert_eq!(memory.0, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn fails_to_store_a_word_out_of_memory() {
        let mut memory = Memory(vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        assert_eq!(memory.store_word(uint!("0x10000000000000000"), uint!("0xFF")), Err(Error::MemoryOutOfBounds));
    }

    #[test]
    fn stores() {
        let mut memory = Memory::new();

        assert_eq!(memory.0.len(), 0);
        assert_eq!(memory.store(uint!("4"), uint!("40"), vec![4, 5, 6, 7]), Ok(ReadWriteOperation::<()> {
            offset: 4,
            size: 40,
            extension_size: 64,
            extension_cost: 6,
            result: (),
        }));
        assert_eq!(memory.0, vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn fails_to_store_out_of_memory() {
        let mut memory = Memory(vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        assert_eq!(memory.store(uint!("0x10000000000000000"), uint!("0xFF"), vec![]), Err(Error::MemoryOutOfBounds));
    }

    #[test]
    fn loads_a_word() {
        let mut memory = Memory(vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        assert_eq!(memory.load_word(uint!("6")), Ok(ReadWriteOperation::<u256> {
            offset: 6,
            size: 32,
            extension_size: 32,
            extension_cost: 3,
            result: uint!("0x0607000000000000000000000000000000000000000000000000000000000000"),
        }));
    }

    #[test]
    fn fails_to_load_a_word_out_of_memory() {
        let mut memory = Memory(vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        assert_eq!(memory.load_word(uint!("0x10000000000000000")), Err(Error::MemoryOutOfBounds));
    }

    #[test]
    fn loads() {
        let mut memory = Memory(vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        assert_eq!(memory.load(uint!("6"), uint!("4")), Ok(ReadWriteOperation::<Vec<u8>> {
            offset: 6,
            size: 4,
            extension_size: 0,
            extension_cost: 0,
            result: vec![6, 7, 0, 0],
        }));
    }

    #[test]
    fn fails_to_load_out_of_memory() {
        let mut memory = Memory(vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        assert_eq!(memory.load(uint!("0x10000000000000000"), uint!("4")), Err(Error::MemoryOutOfBounds));
    }

    #[test]
    fn returns_the_current_size() {
        let memory = Memory(vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0]);

        assert_eq!(memory.size(), 11);
    }

    #[test]
    fn computes_extension_size() {
        let memory = Memory(vec![0; 32]);

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

        assert_eq!(memory.store_byte(uint!("2"), uint!("0xFFAB")), Ok(ReadWriteOperation::<()> {
            offset: 2,
            size: 1,
            extension_size: 32,
            extension_cost: 3,
            result: (),
        }));
        assert_eq!(memory.0, vec![0, 0, 0xAB, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        assert_eq!(memory.store_byte(uint!("32"), uint!("0xFFAB")), Ok(ReadWriteOperation::<()> {
            offset: 32,
            size: 1,
            extension_size: 32,
            extension_cost: 3,
            result: (),
        }));
        assert_eq!(memory.0, vec![0, 0, 0xAB, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xAB, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn fails_to_store_a_byte_out_of_memory() {
        let mut memory = Memory(vec![0, 0, 0, 0, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        assert_eq!(memory.store_byte(uint!("0x10000000000000000"), uint!("0xFF")), Err(Error::MemoryOutOfBounds));
    }

    #[test]
    fn computes_usize_offset_and_size_or_fail() {
        // offset is greater than usize::MAX
        assert_eq!(Memory::try_offset_size(uint!("0x10000000000000000"), uint!("0")), Err(Error::MemoryOutOfBounds));

        // size is greater than usize::MAX
        assert_eq!(Memory::try_offset_size(uint!("0"), uint!("0x10000000000000000")), Err(Error::MemoryOutOfBounds));

        // offset + size - 1 is greater than usize::MAX
        assert_eq!(Memory::try_offset_size(uint!("0xFFFFFFFFFFFFFFFF"), uint!("2")), Err(Error::MemoryOutOfBounds));
        assert_eq!(Memory::try_offset_size(uint!("2"), uint!("0xFFFFFFFFFFFFFFFF")), Err(Error::MemoryOutOfBounds));

        // reading or writing 0 bytes makes no sense. addressing this case only to avoid panicing
        assert_eq!(Memory::try_offset_size(uint!("4"), uint!("0")), Ok((4, 0)));

        assert_eq!(Memory::try_offset_size(uint!("4"), uint!("32")), Ok((4, 32)));
    }
}
