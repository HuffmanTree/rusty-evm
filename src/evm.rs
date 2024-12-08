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

    use crate::storage::StorageValue;

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
    fn return_simple_add() {
        let mut evm = EVM::new(Parameters { initial_storage: Default::default(), initial_accounts: Default::default() });

        // 0x42 + 0xFF = 321
        // 256 + 65 = 321
        assert_eq!(evm.run(Transaction {
            data: vec![0x60, 0x42, 0x60, 0xFF, 0x01, 0x5F, 0x52, 0x60, 0x20, 0x5F, 0xF3], // PUSH1 0x42 PUSH1 0xFF ADD PUSH0 MSTORE PUSH1 0x20 PUSH0 RETURN
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 50,
            nonce: 0,
            to: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            value: uint!("0"),
        }), Ok(OperationResult { data: vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 65], remaining_gas: 28, revert: false }));
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

    #[test]
    fn minimal_payable() {
        /*
         * pragma solidity 0.8.28;
         *
         * contract Minimal {
         *     constructor() payable { }
         * }
         */
        let mut evm = EVM::new(Parameters { initial_storage: Default::default(), initial_accounts: Default::default() });

        assert_eq!(evm.run(Transaction {
            data: vec![/* begin init code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x60, 0x3e, 0x80, 0x60, 0x0f, 0x5f, 0x39, 0x5f, 0xf3, 0xfe, /* end init code - begin runtime code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x5f, 0x5f, 0xfd, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0x8b, 0xed, 0xd2, 0xa9, 0xf3, 0x84, 0x28, 0xfa, 0xa2, 0x5c, 0x83, 0xb9, 0x72, 0xe1, 0x98, 0xde, 0x6d, 0x27, 0xb2, 0xe5, 0x4f, 0x67, 0x72, 0xfc, 0x3b, 0x30, 0x34, 0x5c, 0x11, 0x20, 0x3d, 0x47, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x00, 0x08, 0x1c, 0x00, 0x33 /* end runtime code */],
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 100,
            nonce: 0,
            to: Address(uint!("0")),
            value: uint!("1"),
        }), Ok(OperationResult { data: vec![0x60, 0x80, 0x60, 0x40, 0x52, 0x5f, 0x5f, 0xfd, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0x8b, 0xed, 0xd2, 0xa9, 0xf3, 0x84, 0x28, 0xfa, 0xa2, 0x5c, 0x83, 0xb9, 0x72, 0xe1, 0x98, 0xde, 0x6d, 0x27, 0xb2, 0xe5, 0x4f, 0x67, 0x72, 0xfc, 0x3b, 0x30, 0x34, 0x5c, 0x11, 0x20, 0x3d, 0x47, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x00, 0x08, 0x1c, 0x00, 0x33], remaining_gas: 58, revert: false })); // `data` contains the runtime code
    }

    #[test]
    fn minimal_nonpayable() {
        /*
         * pragma solidity 0.8.28;
         *
         * contract Minimal {
         *     constructor() { }
         * }
         */
        let mut evm = EVM::new(Parameters { initial_storage: Default::default(), initial_accounts: Default::default() });

        assert_eq!(evm.run(Transaction {
            data: vec![/* begin init code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x34, 0x80, 0x15, 0x60, 0x0e, 0x57, 0x5f, 0x5f, 0xfd, 0x5b, 0x50, 0x60, 0x3e, 0x80, 0x60, 0x1a, 0x5f, 0x39, 0x5f, 0xf3, 0xfe, /* end init code - begin runtime code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x5f, 0x5f, 0xfd, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0xb2, 0xff, 0x2a, 0x7f, 0x02, 0x82, 0x1b, 0x6b, 0xd9, 0xd0, 0x4d, 0x01, 0x4b, 0x86, 0x15, 0x65, 0x7f, 0x21, 0xda, 0xac, 0x71, 0xc6, 0x47, 0x5d, 0xcf, 0xb1, 0x97, 0xec, 0x74, 0x3d, 0x0a, 0xfd, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x00, 0x08, 0x1c, 0x00, 0x33 /* end runtime code */],
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 100,
            nonce: 0,
            to: Address(uint!("0")),
            value: uint!("0"),
        }), Ok(OperationResult { data: vec![0x60, 0x80, 0x60, 0x40, 0x52, 0x5f, 0x5f, 0xfd, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0xb2, 0xff, 0x2a, 0x7f, 0x02, 0x82, 0x1b, 0x6b, 0xd9, 0xd0, 0x4d, 0x01, 0x4b, 0x86, 0x15, 0x65, 0x7f, 0x21, 0xda, 0xac, 0x71, 0xc6, 0x47, 0x5d, 0xcf, 0xb1, 0x97, 0xec, 0x74, 0x3d, 0x0a, 0xfd, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x00, 0x08, 0x1c, 0x00, 0x33], remaining_gas: 34, revert: false })); // `data` contains the runtime code

        assert_eq!(evm.run(Transaction {
            data: vec![/* begin init code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x34, 0x80, 0x15, 0x60, 0x0e, 0x57, 0x5f, 0x5f, 0xfd, 0x5b, 0x50, 0x60, 0x3e, 0x80, 0x60, 0x1a, 0x5f, 0x39, 0x5f, 0xf3, 0xfe, /* end init code - begin runtime code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x5f, 0x5f, 0xfd, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0xb2, 0xff, 0x2a, 0x7f, 0x02, 0x82, 0x1b, 0x6b, 0xd9, 0xd0, 0x4d, 0x01, 0x4b, 0x86, 0x15, 0x65, 0x7f, 0x21, 0xda, 0xac, 0x71, 0xc6, 0x47, 0x5d, 0xcf, 0xb1, 0x97, 0xec, 0x74, 0x3d, 0x0a, 0xfd, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x00, 0x08, 0x1c, 0x00, 0x33 /* end runtime code */],
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 100,
            nonce: 0,
            to: Address(uint!("0")),
            value: uint!("1"), // we pay a non payable contract
        }), Ok(OperationResult { data: vec![], remaining_gas: 57, revert: true })); // `data` is empty and the execution is reverted
    }

    #[test]
    fn minimal_param() {
        /*
         * pragma solidity 0.8.28;
         *
         * contract Minimal {
         *     uint256 private x;
         *     constructor (uint256 _x) payable {
         *         x =_x;
         *     }
         * }
         */
        let mut evm = EVM::new(Parameters { initial_storage: Default::default(), initial_accounts: Default::default() });

        assert_eq!(evm.run(Transaction {
            data: vec![/* begin init code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x60, 0x40, 0x51, 0x60, 0xcd, 0x38, 0x03, 0x80, 0x60, 0xcd, 0x83, 0x39, 0x81, 0x81, 0x01, 0x60, 0x40, 0x52, 0x81, 0x01, 0x90, 0x60, 0x21, 0x91, 0x90, 0x60, 0x5e, 0x56, 0x5b, 0x80, 0x5f, 0x81, 0x90, 0x55, 0x50, 0x50, 0x60, 0x84, 0x56, 0x5b, 0x5f, 0x5f, 0xfd, 0x5b, 0x5f, 0x81, 0x90, 0x50, 0x91, 0x90, 0x50, 0x56, 0x5b, 0x60, 0x40, 0x81, 0x60, 0x30, 0x56, 0x5b, 0x81, 0x14, 0x60, 0x49, 0x57, 0x5f, 0x5f, 0xfd, 0x5b, 0x50, 0x56, 0x5b, 0x5f, 0x81, 0x51, 0x90, 0x50, 0x60, 0x58, 0x81, 0x60, 0x39, 0x56, 0x5b, 0x92, 0x91, 0x50, 0x50, 0x56, 0x5b, 0x5f, 0x60, 0x20, 0x82, 0x84, 0x03, 0x12, 0x15, 0x60, 0x70, 0x57, 0x60, 0x6f, 0x60, 0x2c, 0x56, 0x5b, 0x5b, 0x5f, 0x60, 0x7b, 0x84, 0x82, 0x85, 0x01, 0x60, 0x4c, 0x56, 0x5b, 0x91, 0x50, 0x50, 0x92, 0x91, 0x50, 0x50, 0x56, 0x5b, 0x60, 0x3e, 0x80, 0x60, 0x8f, 0x5f, 0x39, 0x5f, 0xf3, 0xfe, /* end init code - begin runtime code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x5f, 0x5f, 0xfd, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0x9a, 0xe1, 0xab, 0x8f, 0x3e, 0x0b, 0xe0, 0xe3, 0x7d, 0xe3, 0x35, 0xff, 0x4d, 0xed, 0x04, 0x6c, 0xf7, 0x7c, 0xe4, 0x5f, 0xd8, 0xb7, 0xfd, 0x61, 0x4f, 0x6a, 0x28, 0x4d, 0x5e, 0x41, 0xd3, 0xf1, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x00, 0x08, 0x1c, 0x00, 0x33, /* end runtime code - begin constructor arguments */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5 /* end constructor arguments */],
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 70000,
            nonce: 0,
            to: Address(uint!("0")),
            value: uint!("0"),
        }), Ok(OperationResult { data: vec![0x60, 0x80, 0x60, 0x40, 0x52, 0x5f, 0x5f, 0xfd, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0x9a, 0xe1, 0xab, 0x8f, 0x3e, 0x0b, 0xe0, 0xe3, 0x7d, 0xe3, 0x35, 0xff, 0x4d, 0xed, 0x04, 0x6c, 0xf7, 0x7c, 0xe4, 0x5f, 0xd8, 0xb7, 0xfd, 0x61, 0x4f, 0x6a, 0x28, 0x4d, 0x5e, 0x41, 0xd3, 0xf1, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x00, 0x08, 0x1c, 0x00, 0x33], remaining_gas: 47534, revert: false })); // `data` contains the runtime code
        assert_eq!(evm.storage.load(uint!("0")), StorageValue { original_value: uint!("0"), value: uint!("5"), warm: true });
    }
}
