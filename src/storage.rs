use ethnum::{u256, U256};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageValue {
    pub value: u256,
    pub warm: bool,
}

pub struct Storage {
    store: HashMap<u256, StorageValue>
}

impl Storage {
    pub fn new() -> Self {
        Self { store: HashMap::<u256, StorageValue>::new() }
    }

    pub fn store(&mut self, key: u256, value: u256) -> Option<StorageValue> {
        match self.store.get(&key) {
            Some(v) => self.store.insert(key, StorageValue { value, warm: v.warm }),
            None => self.store.insert(key, StorageValue { value, warm: false }),
        }
    }

    pub fn load(&mut self, key: u256) -> StorageValue {
        match self.store.get_mut(&key) {
            Some(v) => {
                let res = v.clone();
                v.warm = true;
                res
            },
            None => StorageValue { value: U256::ZERO, warm: false },
        }
    }
}

#[cfg(test)]
mod tests {
    use ethnum::uint;

    use super::*;

    #[test]
    fn stores_a_value() {
        let mut storage = Storage::new();

        storage.store(uint!("42"), uint!("0x0000000004050607000000000000000000000000000000000000000000000000"));

        assert_eq!(storage.store.get(&uint!("42")).unwrap().clone(), StorageValue {
            value: uint!("0x0000000004050607000000000000000000000000000000000000000000000000"),
            warm: false,
        });
    }

    #[test]
    fn loads_an_existing_value_and_warms_the_slot() {
        let mut storage = Storage::new();
        storage.store.insert(uint!("42"), StorageValue {
            value: uint!("0x0000000004050607000000000000000000000000000000000000000000000000"),
            warm: false,
        });

        assert_eq!(storage.load(uint!("42")), StorageValue {
            value: uint!("0x0000000004050607000000000000000000000000000000000000000000000000"),
            warm: false,
        });
        assert_eq!(storage.load(uint!("42")), StorageValue {
            value: uint!("0x0000000004050607000000000000000000000000000000000000000000000000"),
            warm: true,
        });

        storage.store(uint!("42"), uint!("0x0000000004050607000000000000000000000000000000000000000000000001"));

        assert_eq!(storage.load(uint!("42")), StorageValue {
            value: uint!("0x0000000004050607000000000000000000000000000000000000000000000001"),
            warm: true,
        });
    }

    #[test]
    fn loads_a_non_existing_value() {
        let mut storage = Storage::new();

        assert_eq!(storage.load(uint!("42")), StorageValue {
            value: uint!("0"),
            warm: false,
        });
        assert_eq!(storage.load(uint!("42")), StorageValue {
            value: uint!("0"),
            warm: false,
        });
    }
}
