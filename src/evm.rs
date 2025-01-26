use ethnum::u256;
use crate::blockchain::WorldState;
use crate::blockchain::storage::Storage;
use crate::blockchain::primitives::{Account, Address, Block, Transaction};
use crate::machine::{ExecutionResult, Machine};
use crate::machine::context::TransactionContext;
use std::collections::HashMap;

struct EvmParameters {
    accounts: HashMap::<Address, Account>,
    chain_id: u256,
    storage: HashMap<Address, HashMap::<u256, u256>>,
}

#[derive(Default)]
struct Evm(WorldState);

impl Evm {
    fn new(parameters: EvmParameters) -> Self {
        let accounts = Storage::new(parameters.accounts);
        let mut storage = HashMap::<Address, Storage<u256, u256>>::default();
        for (address, store) in parameters.storage {
            storage.insert(address, Storage::new(store));
        }
        let world_state = WorldState { accounts, chain_id: parameters.chain_id, storage };

        Self(world_state)
    }

    fn run(&mut self, block: Block, tx: Transaction) -> ExecutionResult {
        let tctx = TransactionContext { block, tx };
        Machine::execute_transaction(&mut self.0, &tctx)
    }
}

#[cfg(test)]
mod tests {
    use ethnum::uint;
    use crate::blockchain::errors::Error;
    use crate::blockchain::storage::StorageValue;
    use crate::machine::ExecutionOutput;
    use super::*;

    impl Evm {
        fn with_accounts(&mut self, accounts: &[(Address, Account)]) {
            self.0.accounts = Default::default();
            for (address, account) in accounts {
                self.0.accounts.0.insert(*address, StorageValue {
                    original_value: account.clone(),
                    value: account.clone(),
                    warm: false,
                });
            }
        }
    }

    #[test]
    fn simple_add() {
        let mut evm = Evm::default();
        evm.with_accounts(&[(Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), Account { balance: 30000000u32.into(), code: vec![] })]);

        assert_eq!(evm.run(Block::default(), Transaction {
            data: vec![0x60, 0x42, 0x60, 0xFF, 0x01], // PUSH1 0x42 PUSH1 0xFF ADD
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 53130,
            gas_price: 50,
            nonce: 0,
            to: Address::default(),
            value: uint!("0"),
        }), Ok(ExecutionOutput { data: vec![], revert: false, remaining_gas: 41 }));
    }

    #[test]
    fn return_simple_add() {
        let mut evm = Evm::default();
        evm.with_accounts(&[(Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), Account { balance: 30000000u32.into(), code: vec![] })]);

        // 0x42 + 0xFF = 321
        // 256 + 65 = 321
        assert_eq!(evm.run(Block::default(), Transaction {
            data: vec![0x60, 0x42, 0x60, 0xFF, 0x01, 0x5F, 0x52, 0x60, 0x20, 0x5F, 0xF3], // PUSH1 0x42 PUSH1 0xFF ADD PUSH0 MSTORE PUSH1 0x20 PUSH0 RETURN
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 59626,
            gas_price: 50,
            nonce: 0,
            to: Address::default(),
            value: uint!("0"),
        }), Ok(ExecutionOutput { data: vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 65], remaining_gas: 28, revert: false }));
    }

    #[test]
    fn intrisic_gas_too_low() {
        let mut evm = Evm::default();
        evm.with_accounts(&[(Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), Account { balance: 30000000u32.into(), code: vec![] })]);

        assert_eq!(evm.run(Block::default(), Transaction {
            data: vec![0x60, 0x42, 0x60, 0xFF, 0x01], // PUSH1 0x42 PUSH1 0xFF ADD
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 21000,
            nonce: 0,
            gas_price: 50,
            to: Address::default(),
            value: uint!("0"),
        }), Err(Error::IntrisicGasTooLow(53080)));
    }

    #[test]
    fn insufficient_funds() {
        let mut evm = Evm::default();

        assert_eq!(evm.run(Block::default(), Transaction {
            data: vec![0x60, 0x42, 0x60, 0xFF, 0x01], // PUSH1 0x42 PUSH1 0xFF ADD
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 54000,
            nonce: 0,
            gas_price: 50,
            to: Address::default(),
            value: uint!("0"),
        }), Err(Error::InsufficientFunds(uint!("2700000"))));
    }

    #[test]
    fn out_of_gas() {
        let mut evm = Evm::default();
        evm.with_accounts(&[(Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), Account { balance: 30000000u32.into(), code: vec![] })]);

        assert_eq!(evm.run(Block::default(), Transaction {
            data: vec![0x60, 0x42, 0x60, 0xFF, 0x01], // PUSH1 0x42 PUSH1 0xFF ADD
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 53082,
            nonce: 0,
            gas_price: 50,
            to: Address::default(),
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
        let mut evm = Evm::default();
        evm.with_accounts(&[(Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), Account { balance: 30000000u32.into(), code: vec![] })]);

        assert_eq!(evm.run(Block::default(), Transaction {
            data: vec![/* begin init code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x60, 0x3e, 0x80, 0x60, 0x0f, 0x5f, 0x39, 0x5f, 0xf3, 0xfe, /* end init code - begin runtime code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x5f, 0x5f, 0xfd, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0x8b, 0xed, 0xd2, 0xa9, 0xf3, 0x84, 0x28, 0xfa, 0xa2, 0x5c, 0x83, 0xb9, 0x72, 0xe1, 0x98, 0xde, 0x6d, 0x27, 0xb2, 0xe5, 0x4f, 0x67, 0x72, 0xfc, 0x3b, 0x30, 0x34, 0x5c, 0x11, 0x20, 0x3d, 0x47, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x00, 0x08, 0x1c, 0x00, 0x33 /* end runtime code */],
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 66708,
            gas_price: 50,
            nonce: 0,
            to: Address::default(),
            value: uint!("1"),
        }), Ok(ExecutionOutput { data: vec![0x60, 0x80, 0x60, 0x40, 0x52, 0x5f, 0x5f, 0xfd, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0x8b, 0xed, 0xd2, 0xa9, 0xf3, 0x84, 0x28, 0xfa, 0xa2, 0x5c, 0x83, 0xb9, 0x72, 0xe1, 0x98, 0xde, 0x6d, 0x27, 0xb2, 0xe5, 0x4f, 0x67, 0x72, 0xfc, 0x3b, 0x30, 0x34, 0x5c, 0x11, 0x20, 0x3d, 0x47, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x00, 0x08, 0x1c, 0x00, 0x33], remaining_gas: 60, revert: false })); // `data` contains the runtime code
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
        let mut evm = Evm::default();
        evm.with_accounts(&[(Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), Account { balance: 30000000u32.into(), code: vec![] })]);

        assert_eq!(evm.run(Block::default(), Transaction {
            data: vec![/* begin init code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x34, 0x80, 0x15, 0x60, 0x0e, 0x57, 0x5f, 0x5f, 0xfd, 0x5b, 0x50, 0x60, 0x3e, 0x80, 0x60, 0x1a, 0x5f, 0x39, 0x5f, 0xf3, 0xfe, /* end init code - begin runtime code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x5f, 0x5f, 0xfd, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0xb2, 0xff, 0x2a, 0x7f, 0x02, 0x82, 0x1b, 0x6b, 0xd9, 0xd0, 0x4d, 0x01, 0x4b, 0x86, 0x15, 0x65, 0x7f, 0x21, 0xda, 0xac, 0x71, 0xc6, 0x47, 0x5d, 0xcf, 0xb1, 0x97, 0xec, 0x74, 0x3d, 0x0a, 0xfd, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x00, 0x08, 0x1c, 0x00, 0x33 /* end runtime code */],
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 66884,
            gas_price: 50,
            nonce: 0,
            to: Address::default(),
            value: uint!("0"),
        }), Ok(ExecutionOutput { data: vec![0x60, 0x80, 0x60, 0x40, 0x52, 0x5f, 0x5f, 0xfd, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0xb2, 0xff, 0x2a, 0x7f, 0x02, 0x82, 0x1b, 0x6b, 0xd9, 0xd0, 0x4d, 0x01, 0x4b, 0x86, 0x15, 0x65, 0x7f, 0x21, 0xda, 0xac, 0x71, 0xc6, 0x47, 0x5d, 0xcf, 0xb1, 0x97, 0xec, 0x74, 0x3d, 0x0a, 0xfd, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x00, 0x08, 0x1c, 0x00, 0x33], remaining_gas: 36, revert: false })); // `data` contains the runtime code

        assert_eq!(evm.run(Block::default(), Transaction {
            data: vec![/* begin init code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x34, 0x80, 0x15, 0x60, 0x0e, 0x57, 0x5f, 0x5f, 0xfd, 0x5b, 0x50, 0x60, 0x3e, 0x80, 0x60, 0x1a, 0x5f, 0x39, 0x5f, 0xf3, 0xfe, /* end init code - begin runtime code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x5f, 0x5f, 0xfd, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0xb2, 0xff, 0x2a, 0x7f, 0x02, 0x82, 0x1b, 0x6b, 0xd9, 0xd0, 0x4d, 0x01, 0x4b, 0x86, 0x15, 0x65, 0x7f, 0x21, 0xda, 0xac, 0x71, 0xc6, 0x47, 0x5d, 0xcf, 0xb1, 0x97, 0xec, 0x74, 0x3d, 0x0a, 0xfd, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x00, 0x08, 0x1c, 0x00, 0x33 /* end runtime code */],
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas_price: 50,
            gas: 54484,
            nonce: 0,
            to: Address::default(),
            value: uint!("1"), // we pay a non payable contract
        }), Ok(ExecutionOutput { data: vec![], remaining_gas: 57, revert: true })); // `data` is empty and the execution is reverted
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
        let mut evm = Evm::default();
        evm.with_accounts(&[(Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), Account { balance: 30000000u32.into(), code: vec![] })]);

        assert_eq!(evm.run(Block::default(), Transaction {
            data: vec![/* begin init code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x60, 0x40, 0x51, 0x60, 0xcd, 0x38, 0x03, 0x80, 0x60, 0xcd, 0x83, 0x39, 0x81, 0x81, 0x01, 0x60, 0x40, 0x52, 0x81, 0x01, 0x90, 0x60, 0x21, 0x91, 0x90, 0x60, 0x5e, 0x56, 0x5b, 0x80, 0x5f, 0x81, 0x90, 0x55, 0x50, 0x50, 0x60, 0x84, 0x56, 0x5b, 0x5f, 0x5f, 0xfd, 0x5b, 0x5f, 0x81, 0x90, 0x50, 0x91, 0x90, 0x50, 0x56, 0x5b, 0x60, 0x40, 0x81, 0x60, 0x30, 0x56, 0x5b, 0x81, 0x14, 0x60, 0x49, 0x57, 0x5f, 0x5f, 0xfd, 0x5b, 0x50, 0x56, 0x5b, 0x5f, 0x81, 0x51, 0x90, 0x50, 0x60, 0x58, 0x81, 0x60, 0x39, 0x56, 0x5b, 0x92, 0x91, 0x50, 0x50, 0x56, 0x5b, 0x5f, 0x60, 0x20, 0x82, 0x84, 0x03, 0x12, 0x15, 0x60, 0x70, 0x57, 0x60, 0x6f, 0x60, 0x2c, 0x56, 0x5b, 0x5b, 0x5f, 0x60, 0x7b, 0x84, 0x82, 0x85, 0x01, 0x60, 0x4c, 0x56, 0x5b, 0x91, 0x50, 0x50, 0x92, 0x91, 0x50, 0x50, 0x56, 0x5b, 0x60, 0x3e, 0x80, 0x60, 0x8f, 0x5f, 0x39, 0x5f, 0xf3, 0xfe, /* end init code - begin runtime code */ 0x60, 0x80, 0x60, 0x40, 0x52, 0x5f, 0x5f, 0xfd, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0x9a, 0xe1, 0xab, 0x8f, 0x3e, 0x0b, 0xe0, 0xe3, 0x7d, 0xe3, 0x35, 0xff, 0x4d, 0xed, 0x04, 0x6c, 0xf7, 0x7c, 0xe4, 0x5f, 0xd8, 0xb7, 0xfd, 0x61, 0x4f, 0x6a, 0x28, 0x4d, 0x5e, 0x41, 0xd3, 0xf1, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x00, 0x08, 0x1c, 0x00, 0x33, /* end runtime code - begin constructor arguments */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5 /* end constructor arguments */],
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 138796,
            gas_price: 50,
            nonce: 0,
            to: Address(uint!("0")),
            value: uint!("10"),
        }), Ok(ExecutionOutput { data: vec![0x60, 0x80, 0x60, 0x40, 0x52, 0x5f, 0x5f, 0xfd, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0x9a, 0xe1, 0xab, 0x8f, 0x3e, 0x0b, 0xe0, 0xe3, 0x7d, 0xe3, 0x35, 0xff, 0x4d, 0xed, 0x04, 0x6c, 0xf7, 0x7c, 0xe4, 0x5f, 0xd8, 0xb7, 0xfd, 0x61, 0x4f, 0x6a, 0x28, 0x4d, 0x5e, 0x41, 0xd3, 0xf1, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x00, 0x08, 0x1c, 0x00, 0x33], remaining_gas: 47538, revert: false })); // `data` contains the runtime code
        assert_eq!(
            evm.0.storage.get(&Address(uint!("0xDBCD4009C9B9D36CC85256A8377A034C24CE0044"))).unwrap().0.get(&uint!("0")).unwrap().value,
            uint!("5"),
        );
        assert_eq!(
            evm.0.accounts.0.get(&Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C"))).unwrap().value.balance,
            uint!("25437090"),
        );
        assert_eq!(
            evm.0.accounts.0.get(&Address(uint!("0xDBCD4009C9B9D36CC85256A8377A034C24CE0044"))).unwrap().value,
            Account { balance: uint!("10"), code: vec![0x60, 0x80, 0x60, 0x40, 0x52, 0x5f, 0x5f, 0xfd, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0x9a, 0xe1, 0xab, 0x8f, 0x3e, 0x0b, 0xe0, 0xe3, 0x7d, 0xe3, 0x35, 0xff, 0x4d, 0xed, 0x04, 0x6c, 0xf7, 0x7c, 0xe4, 0x5f, 0xd8, 0xb7, 0xfd, 0x61, 0x4f, 0x6a, 0x28, 0x4d, 0x5e, 0x41, 0xd3, 0xf1, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x00, 0x08, 0x1c, 0x00, 0x33] },
        );
    }

    #[test]
    fn minimal_setter() {
        /*
         * pragma solidity 0.8.28;
         *
         * contract MinimalSetter {
         *     uint256 private x;
         *     function setX(uint256 _x) public { x = _x; }
         * }
         */
        let mut evm = Evm::default();

        evm.with_accounts(&[
            (Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), Account { balance: 30000000u32.into(), code: vec![] }),
            (Address(uint!("0xDBCD4009C9B9D36CC85256A8377A034C24CE0044")), Account { balance: 0u32.into(), code: vec![0x60, 0x80, 0x60, 0x40, 0x52, 0x34, 0x80, 0x15, 0x60, 0xe, 0x57, 0x5f, 0x5f, 0xfd, 0x5b, 0x50, 0x60, 0x4, 0x36, 0x10, 0x60, 0x26, 0x57, 0x5f, 0x35, 0x60, 0xe0, 0x1c, 0x80, 0x63, 0x40, 0x18, 0xd9, 0xaa, 0x14, 0x60, 0x2a, 0x57, 0x5b, 0x5f, 0x5f, 0xfd, 0x5b, 0x60, 0x40, 0x60, 0x4, 0x80, 0x36, 0x3, 0x81, 0x1, 0x90, 0x60, 0x3c, 0x91, 0x90, 0x60, 0x7d, 0x56, 0x5b, 0x60, 0x42, 0x56, 0x5b, 0x0, 0x5b, 0x80, 0x5f, 0x81, 0x90, 0x55, 0x50, 0x50, 0x56, 0x5b, 0x5f, 0x5f, 0xfd, 0x5b, 0x5f, 0x81, 0x90, 0x50, 0x91, 0x90, 0x50, 0x56, 0x5b, 0x60, 0x5f, 0x81, 0x60, 0x4f, 0x56, 0x5b, 0x81, 0x14, 0x60, 0x68, 0x57, 0x5f, 0x5f, 0xfd, 0x5b, 0x50, 0x56, 0x5b, 0x5f, 0x81, 0x35, 0x90, 0x50, 0x60, 0x77, 0x81, 0x60, 0x58, 0x56, 0x5b, 0x92, 0x91, 0x50, 0x50, 0x56, 0x5b, 0x5f, 0x60, 0x20, 0x82, 0x84, 0x3, 0x12, 0x15, 0x60, 0x8f, 0x57, 0x60, 0x8e, 0x60, 0x4b, 0x56, 0x5b, 0x5b, 0x5f, 0x60, 0x9a, 0x84, 0x82, 0x85, 0x1, 0x60, 0x6b, 0x56, 0x5b, 0x91, 0x50, 0x50, 0x92, 0x91, 0x50, 0x50, 0x56, 0xfe, 0xa2, 0x64, 0x69, 0x70, 0x66, 0x73, 0x58, 0x22, 0x12, 0x20, 0x62, 0x58, 0x4a, 0x4b, 0x66, 0x87, 0xdb, 0x1d, 0xe8, 0x3d, 0xa4, 0xe2, 0xfc, 0xd8, 0x21, 0x6b, 0xd7, 0x7e, 0x9a, 0xe0, 0x6, 0x44, 0x89, 0xf2, 0x3, 0x80, 0xcf, 0xc6, 0x53, 0x20, 0x3, 0x85, 0x64, 0x73, 0x6f, 0x6c, 0x63, 0x43, 0x0, 0x8, 0x1c, 0x0, 0x33] }) // `code` contains the runtime code
        ]);

        assert_eq!(evm.run(Block::default(), Transaction {
            data: vec![/* begin function selector */ 0x40, 0x18, 0xd9, 0xaa, /* end function selector - begin function arguments */ 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x2a /* end function arguments */],
            from: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")),
            gas: 138796,
            gas_price: 50,
            nonce: 0,
            to: Address(uint!("0xDBCD4009C9B9D36CC85256A8377A034C24CE0044")),
            value: uint!("0"),
        }), Ok(ExecutionOutput { data: vec![], remaining_gas: 95100, revert: false }));

        assert_eq!(
            evm.0.storage.get(&Address(uint!("0xDBCD4009C9B9D36CC85256A8377A034C24CE0044"))).unwrap().0.get(&uint!("0")).unwrap().value,
            uint!("42"),
        );
    }
}
