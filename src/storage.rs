use ethnum::{u256, U256};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageValue {
    pub original_value: u256,
    pub value: u256,
    pub warm: bool,
}

pub struct Storage {
    store: HashMap<u256, StorageValue>
}

impl Storage {
    pub fn new(init: HashMap::<u256, u256>) -> Self {
        let mut store = HashMap::<u256, StorageValue>::new();
        for (key, value) in init {
            store.insert(key, StorageValue { original_value: value, value, warm: false });
        }
        Self { store }
    }

    pub fn store(&mut self, key: u256, value: u256) -> Option<StorageValue> {
        match self.store.get(&key) {
            Some(v) => self.store.insert(key, StorageValue { original_value: v.original_value, value, warm: true }),
            None => self.store.insert(key, StorageValue { original_value: U256::ZERO, value, warm: true }),
        }
    }

    pub fn load(&mut self, key: u256) -> StorageValue {
        match self.store.get_mut(&key) {
            Some(v) => {
                let res = v.clone();
                v.warm = true;
                res
            },
            None => {
                self.store.insert(key, StorageValue { original_value: U256::ZERO, value: U256::ZERO, warm: true });
                StorageValue { original_value: U256::ZERO, value: U256::ZERO, warm: false }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use ethnum::uint;

    use super::*;

    #[test]
    fn builds_a_storage_with_initial_values() {
        let mut init = HashMap::<u256, u256>::new();
        init.insert(uint!("42"), uint!("3"));
        init.insert(uint!("43"), uint!("4"));

        let storage = Storage::new(init);

        assert_eq!(storage.store.get(&uint!("42")).unwrap().clone(), StorageValue {
            original_value: uint!("3"),
            value: uint!("3"),
            warm: false,
        });
        assert_eq!(storage.store.get(&uint!("43")).unwrap().clone(), StorageValue {
            original_value: uint!("4"),
            value: uint!("4"),
            warm: false,
        });
    }

    #[test]
    fn stores_a_value() {
        let mut storage = Storage::new(HashMap::<u256, u256>::new());

        storage.store(uint!("42"), uint!("0x0000000004050607000000000000000000000000000000000000000000000000"));

        assert_eq!(storage.store.get(&uint!("42")).unwrap().clone(), StorageValue {
            original_value: uint!("0"),
            value: uint!("0x0000000004050607000000000000000000000000000000000000000000000000"),
            warm: true,
        });
    }

    #[test]
    fn loads_an_existing_value_and_warms_the_slot() {
        let mut storage = Storage::new(HashMap::<u256, u256>::new());
        storage.store.insert(uint!("42"), StorageValue {
            original_value: uint!("0"),
            value: uint!("0x0000000004050607000000000000000000000000000000000000000000000000"),
            warm: false,
        });

        assert_eq!(storage.load(uint!("42")), StorageValue {
            original_value: uint!("0"),
            value: uint!("0x0000000004050607000000000000000000000000000000000000000000000000"),
            warm: false,
        });
        assert_eq!(storage.load(uint!("42")), StorageValue {
            original_value: uint!("0"),
            value: uint!("0x0000000004050607000000000000000000000000000000000000000000000000"),
            warm: true,
        });

        storage.store(uint!("42"), uint!("0x0000000004050607000000000000000000000000000000000000000000000001"));

        assert_eq!(storage.load(uint!("42")), StorageValue {
            original_value: uint!("0"),
            value: uint!("0x0000000004050607000000000000000000000000000000000000000000000001"),
            warm: true,
        });
    }

    #[test]
    fn loads_a_non_existing_value_and_warms_the_slot() {
        let mut storage = Storage::new(HashMap::<u256, u256>::new());

        assert_eq!(storage.load(uint!("42")), StorageValue {
            original_value: uint!("0"),
            value: uint!("0"),
            warm: false,
        });
        assert_eq!(storage.load(uint!("42")), StorageValue {
            original_value: uint!("0"),
            value: uint!("0"),
            warm: true,
        });
    }
}
