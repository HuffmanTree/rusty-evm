pub mod errors;
pub mod primitives;
pub mod storage;

use ethnum::u256;
use crate::blockchain::errors::Error;
use crate::blockchain::primitives::{Account, Address};
use crate::blockchain::storage::Storage;
use std::collections::HashMap;

#[derive(Default)]
pub struct WorldState {
    pub accounts: Storage<Address, Account>,
    pub chain_id: u256,
    pub storage: HashMap<Address, Storage<u256, u256>>,
}

impl WorldState {
    pub fn decrease_balance(&mut self, address: Address, cost: u256) -> Result<(), Error> {
        let account = self.accounts.load(address).value;

        self.accounts.store(address, Account {
            balance: account.check_enough_funds(cost)?,
            code: account.code,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ethnum::uint;
    use storage::StorageValue;
    use super::*;

    #[test]
    fn decrease_balance() {
        let mut s = WorldState::default();
        s.accounts.0.insert(Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), StorageValue::<Account> {
            original_value: Account::default(),
            value: Account {
                balance: uint!("42"),
                code: vec![],
            },
            warm: true,
        });

        assert_eq!(s.decrease_balance(Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), uint!("50")), Err(Error::InsufficientFunds(uint!("50"))));
        assert!(s.decrease_balance(Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), uint!("40")).is_ok());
        assert_eq!(s.accounts.load(Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C"))).value, Account {
            balance: uint!("2"),
            code: vec![],
        });
    }
}
