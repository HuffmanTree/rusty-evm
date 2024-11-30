use ethnum::{u256, U256};
use std::collections::HashMap;

#[derive(Default)]
pub struct Transient(pub HashMap<u256, u256>);

impl Transient {
    pub fn new() -> Self {
        Self(Default::default())
    }

    pub fn store(&mut self, key: u256, value: u256) -> Option<u256> {
        self.0.insert(key, value)
    }

    pub fn load(&mut self, key: u256) -> u256 {
        *(self.0.get(&key).unwrap_or(&U256::ZERO))
    }
}

#[cfg(test)]
mod tests {
    use ethnum::uint;

    use super::*;

    #[test]
    fn store() {
        let mut transient = Transient::new();

        transient.store(uint!("42"), uint!("0x0000000004050607000000000000000000000000000000000000000000000000"));

        assert_eq!(transient.0.get(&uint!("42")), Some(&uint!("0x0000000004050607000000000000000000000000000000000000000000000000")));
    }

    #[test]
    fn load() {
        let mut transient = Transient::new();

        transient.0.insert(uint!("42"), uint!("0x0000000004050607000000000000000000000000000000000000000000000000"));

        assert_eq!(transient.load(uint!("42")), uint!("0x0000000004050607000000000000000000000000000000000000000000000000"));
        assert_eq!(transient.load(uint!("43")), uint!("0"));
    }
}
