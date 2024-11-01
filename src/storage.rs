use ethnum::u256;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
struct StorageValue {
    value: u256,
    warm: bool,
}

struct Storage {
    store: HashMap<u128, StorageValue>
}

impl Storage {
    fn new() -> Self {
        Self { store: HashMap::<u128, StorageValue>::new() }
    }

    fn store(&mut self, key: u128, value: u256) -> Option<StorageValue> {
        self.store.insert(key, StorageValue { value, warm: false })
    }

    fn load(&mut self, key: u128) -> StorageValue {
        match self.store.get_mut(&key) {
            Some(v) => {
                let res = v.clone();
                v.warm = true;
                res
            },
            None => StorageValue { value: u256::from(0_u8), warm: false },
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

        storage.store(42, uint!("0x0000000004050607000000000000000000000000000000000000000000000000"));

        assert_eq!(storage.store.get(&42).unwrap().clone(), StorageValue {
            value: uint!("0x0000000004050607000000000000000000000000000000000000000000000000"),
            warm: false,
        });
    }

    #[test]
    fn loads_an_existing_value_and_warms_the_slot() {
        let mut storage = Storage::new();
        storage.store.insert(42, StorageValue {
            value: uint!("0x0000000004050607000000000000000000000000000000000000000000000000"),
            warm: false,
        });

        assert_eq!(storage.load(42), StorageValue {
            value: uint!("0x0000000004050607000000000000000000000000000000000000000000000000"),
            warm: false,
        });
        assert_eq!(storage.load(42), StorageValue {
            value: uint!("0x0000000004050607000000000000000000000000000000000000000000000000"),
            warm: true,
        });
    }

    #[test]
    fn loads_a_non_existing_value() {
        let mut storage = Storage::new();

        assert_eq!(storage.load(42), StorageValue {
            value: uint!("0"),
            warm: false,
        });
        assert_eq!(storage.load(42), StorageValue {
            value: uint!("0"),
            warm: false,
        });
    }
}
