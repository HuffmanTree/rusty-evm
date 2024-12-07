use std::collections::HashMap;
use ethnum::u256;
use crate::{errors::Error, state::{State, StateParameters}, storage::Storage, transaction::{Address, Transaction}};

struct EVM {
    accounts: Storage<Address, u256>,
    storage: Storage<u256, u256>,
}

struct Parameters {
    initial_accounts: HashMap::<Address, u256>,
    initial_storage: HashMap::<u256, u256>,
}

impl EVM {
    fn new(parameters: Parameters) -> Self {
        Self {
            accounts: Storage::new(parameters.initial_accounts),
            storage: Storage::new(parameters.initial_storage),
        }
    }

    fn run(self: EVM, transaction: Transaction) -> Result<(), Error> {
        let mut state = State::new(StateParameters {
            accounts: self.accounts,
            storage: self.storage,
            transaction,
        });

        while !state.stop_flag {
            state.execute_next_opcode()?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ethnum::uint;

    use super::*;

    #[test]
    fn simple_add() {
        let evm = EVM::new(Parameters { initial_storage: Default::default(), initial_accounts: Default::default() });

        assert!(evm.run(Transaction {
            data: vec![0x60, 0x42, 0x60, 0xFF, 0x01], // PUSH1 0x42 PUSH1 0xFF ADD
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 50,
            nonce: 0,
            to: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            value: uint!("0"),
        }).is_ok());
    }

    #[test]
    fn out_of_gas() {
        let evm = EVM::new(Parameters { initial_storage: Default::default(), initial_accounts: Default::default() });

        assert_eq!(evm.run(Transaction {
            data: vec![0x60, 0x42, 0x60, 0xFF, 0x01], // PUSH1 0x42 PUSH1 0xFF ADD
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 2,
            nonce: 0,
            to: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            value: uint!("0"),
        }), Err(Error::OutOfGas));
    }
}
