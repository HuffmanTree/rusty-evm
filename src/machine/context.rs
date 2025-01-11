use ethnum::u256;
use crate::blockchain::WorldState;
use crate::blockchain::primitives::{Address, Block, Transaction};
use crate::machine::memory::Memory;
use crate::machine::stack::Stack;
use crate::machine::transient::Transient;

#[derive(Default, Clone, Debug, Eq, PartialEq)]
pub struct Log {
    pub data: Vec<u8>,
    pub topics: [Option<u256>; 4],
}

#[derive(Default)]
pub struct CallContextContract {
    pub address: Address,
    pub caller: Address,
    pub code: Vec<u8>,
    pub gas: usize,
    pub input: Vec<u8>,
    pub logs: Vec<Log>,
    pub value: u256,
}

#[derive(Default)]
pub struct TransactionContext {
    pub block: Block,
    pub tx: Transaction,
}

#[derive(Default)]
pub struct CallContext {
    pub contract: CallContextContract,
    pub memory: Memory,
    pub pc: usize,
    pub r#return: Vec<u8>,
    pub returndata: Vec<u8>,
    pub revert: bool,
    pub stack: Stack,
    pub stop: bool,
    pub transient: Transient,
}

impl CallContext {
    pub fn from_transaction(s: &mut WorldState, tx: &Transaction) -> Self {
        let contract_address = tx.contract_address();
        let contract_input = &tx.data;
        let contract = CallContextContract {
            address: contract_address,
            caller: tx.from,
            code: if tx.is_contract_creation() { contract_input.clone() } else { s.accounts.load(contract_address).value.code },
            gas: tx.gas,
            input: contract_input.clone(),
            logs: Vec::default(),
            value: tx.value,
        };
        Self {
            contract,
            memory: Memory::new(),
            pc: 0,
            r#return: Vec::default(),
            returndata: Vec::default(),
            revert: false,
            stack: Stack::new(),
            stop: false,
            transient: Transient::new(),
        }
    }
}
