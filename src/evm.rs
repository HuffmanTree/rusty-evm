use std::collections::HashMap;
use ethnum::{u256, U256};
use crate::{errors::Error, state::{State, StateParameters}, storage::Storage, transaction::{Account, Address, Transaction}};

struct EVM {
    accounts: Storage<Address, Account>,
    storage: Storage<u256, u256>,
}

struct Parameters {
    initial_accounts: HashMap::<Address, Account>,
    initial_storage: HashMap::<u256, u256>,
}

#[derive(Debug, Eq, PartialEq)]
struct OperationResult {
    data: Vec<u8>,
    remaining_gas: usize,
    revert: bool,
}

impl EVM {
    fn new(parameters: Parameters) -> Self {
        Self {
            accounts: Storage::new(parameters.initial_accounts),
            storage: Storage::new(parameters.initial_storage),
        }
    }

    fn run(&mut self, transaction: Transaction) -> Result<OperationResult, Error> {
        if transaction.to.0 == U256::ZERO {
            let tx = transaction.clone();
            self.accounts.store(tx.contract_address(), Account { balance: tx.value, code: tx.data });
        }

        let mut state = State::new(StateParameters {
            accounts: &mut self.accounts,
            storage: &mut self.storage,
            transaction,
        });

        while !state.stop_flag {
            state.execute_next_opcode()?;
        }

        Ok(OperationResult {
            data: state.returndata,
            remaining_gas: state.remaining_gas,
            revert: state.revert_flag,
        })
    }
}

#[cfg(test)]
mod tests {
    use ethnum::uint;

    use super::*;

    #[test]
    fn simple_add() {
        let mut evm = EVM::new(Parameters { initial_storage: Default::default(), initial_accounts: Default::default() });

        assert_eq!(evm.run(Transaction {
            data: vec![0x60, 0x42, 0x60, 0xFF, 0x01], // PUSH1 0x42 PUSH1 0xFF ADD
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 50,
            nonce: 0,
            to: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            value: uint!("0"),
        }), Ok(OperationResult { data: vec![], revert: false, remaining_gas: 41 }));
    }

    #[test]
    fn out_of_gas() {
        let mut evm = EVM::new(Parameters { initial_storage: Default::default(), initial_accounts: Default::default() });

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
