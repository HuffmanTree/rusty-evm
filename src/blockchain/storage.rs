use std::{collections::HashMap, hash::Hash};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct StorageValue<V> {
    pub original_value: V,
    pub value: V,
    pub warm: bool,
}

#[derive(Default, Debug)]
pub struct Storage<K, V>(pub HashMap<K, StorageValue<V>>);

impl<K, V> Storage<K, V> where K: Hash + Eq, V: Default + Clone {
    pub fn new(init: HashMap::<K, V>) -> Self {
        let mut store = HashMap::<K, StorageValue<V>>::new();
        for (key, value) in init {
            store.insert(key, StorageValue { original_value: value.clone(), value, warm: false });
        }
        Self(store)
    }

    pub fn store(&mut self, key: K, value: V) -> Option<StorageValue<V>> {
        match self.0.get(&key) {
            Some(v) => self.0.insert(key, StorageValue { original_value: v.original_value.clone(), value, warm: true }),
            None => self.0.insert(key, StorageValue { original_value: Default::default(), value, warm: true }),
        }
    }

    pub fn load(&mut self, key: K) -> StorageValue<V> {
        match self.0.get_mut(&key) {
            Some(v) => {
                let res = v.clone();
                v.warm = true;
                res
            },
            None => {
                self.0.insert(key, StorageValue { original_value: Default::default(), value: Default::default(), warm: true });
                StorageValue { original_value: Default::default(), value: Default::default(), warm: false }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use ethnum::{uint, u256};

    use super::*;

    #[test]
    fn builds_a_storage_with_initial_values() {
        let mut init = HashMap::<u256, u256>::new();
        init.insert(uint!("42"), uint!("3"));
        init.insert(uint!("43"), uint!("4"));

        let storage = Storage::new(init);

        assert_eq!(storage.0.get(&uint!("42")).unwrap().clone(), StorageValue {
            original_value: uint!("3"),
            value: uint!("3"),
            warm: false,
        });
        assert_eq!(storage.0.get(&uint!("43")).unwrap().clone(), StorageValue {
            original_value: uint!("4"),
            value: uint!("4"),
            warm: false,
        });
    }

    #[test]
    fn stores_a_value() {
        let mut storage = Storage::new(HashMap::<u256, u256>::new());

        storage.store(uint!("42"), uint!("0x0000000004050607000000000000000000000000000000000000000000000000"));

        assert_eq!(storage.0.get(&uint!("42")).unwrap().clone(), StorageValue {
            original_value: uint!("0"),
            value: uint!("0x0000000004050607000000000000000000000000000000000000000000000000"),
            warm: true,
        });
    }

    #[test]
    fn loads_an_existing_value_and_warms_the_slot() {
        let mut storage = Storage::new(HashMap::<u256, u256>::new());
        storage.0.insert(uint!("42"), StorageValue {
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
