use ethnum::{u256, U256};
use crate::errors::Error;
use crate::memory::Memory;
use crate::stack::Stack;
use crate::storage::Storage;
use crate::transaction::{Account, Address, Transaction};
use crate::transient::Transient;
use crate::transitions::{TransitionContext, TransitionFunction, TransitionOutput, ADD, ADDMOD, ADDRESS, AND, BALANCE, BASEFEE, BLOBBASEFEE, BLOBHASH, BLOCKHASH, BYTE, CALL, CALLCODE, CALLDATACOPY, CALLDATALOAD, CALLDATASIZE, CALLER, CALLVALUE, CHAINID, CODECOPY, CODESIZE, COINBASE, CREATE, CREATE2, DELEGATECALL, DIV, DUP1, DUP10, DUP11, DUP12, DUP13, DUP14, DUP15, DUP16, DUP2, DUP3, DUP4, DUP5, DUP6, DUP7, DUP8, DUP9, EQ, EXP, EXTCODECOPY, EXTCODEHASH, EXTCODESIZE, GAS, GASLIMIT, GASPRICE, GT, INVALID, ISZERO, JUMP, JUMPDEST, JUMPI, KECCAK256, LOG0, LOG1, LOG2, LOG3, LOG4, LT, MCOPY, MLOAD, MOD, MSIZE, MSTORE, MSTORE8, MUL, MULMOD, NOT, NUMBER, OR, ORIGIN, PC, POP, PREVRANDAO, PUSH0, PUSH1, PUSH10, PUSH11, PUSH12, PUSH13, PUSH14, PUSH15, PUSH16, PUSH17, PUSH18, PUSH19, PUSH2, PUSH20, PUSH21, PUSH22, PUSH23, PUSH24, PUSH25, PUSH26, PUSH27, PUSH28, PUSH29, PUSH3, PUSH30, PUSH31, PUSH32, PUSH4, PUSH5, PUSH6, PUSH7, PUSH8, PUSH9, RETURN, RETURNDATACOPY, RETURNDATASIZE, REVERT, SAR, SDIV, SELFBALANCE, SELFDESTRUCT, SGT, SHL, SHR, SIGNEXTEND, SLOAD, SLT, SMOD, SSTORE, STATICCALL, STOP, SUB, SWAP1, SWAP10, SWAP11, SWAP12, SWAP13, SWAP14, SWAP15, SWAP16, SWAP2, SWAP3, SWAP4, SWAP5, SWAP6, SWAP7, SWAP8, SWAP9, TIMESTAMP, TLOAD, TSTORE, XOR};

#[derive(Debug)]
pub struct State {
    accounts: Storage<Address, Account>,
    latest_caller: Address,
    remaining_gas: usize,
    stack: Stack,
    memory: Memory,
    storage: Storage<u256, u256>,
    pub stop_flag: bool,
    pub pc: usize,
    returndata: Vec<u8>,
    revert_flag: bool,
    transaction: Transaction,
    transient: Transient,
}

pub struct StateParameters {
    pub accounts: Storage<Address, Account>,
    pub storage: Storage<u256, u256>,
    pub transaction: Transaction,
}

impl State {
    pub fn new(parameters: StateParameters) -> Self {
        Self {
            accounts: parameters.accounts,
            latest_caller: parameters.transaction.from,
            remaining_gas: parameters.transaction.gas,
            stack: Stack::new(),
            memory: Memory::new(),
            storage: parameters.storage,
            stop_flag: false,
            pc: 0,
            returndata: Default::default(),
            revert_flag: false,
            transaction: parameters.transaction,
            transient: Transient::new(),
        }
    }

    fn execute_transition<const I: usize, const O: usize>(&mut self, f: TransitionFunction<I, O>) -> Result<TransitionOutput, Error> {
        let mut context = TransitionContext {
            accounts: &mut self.accounts,
            caller: &self.latest_caller,
            gas: &self.remaining_gas,
            memory: &mut self.memory,
            pc: &mut self.pc,
            returndata: &mut self.returndata,
            stop_flag: &mut self.stop_flag,
            revert_flag: &mut self.revert_flag,
            storage: &mut self.storage,
            transaction: &self.transaction,
            transient: &mut self.transient,
        };
        let mut input = [U256::ZERO; I];
        for i in 0..I {
            input[i] = match self.stack.pop() {
                Some(x) => x,
                _ => return Err(Error::EmptyStack),
            }
        };
        let output = f(&mut context, input)?;
        if output.cost > self.remaining_gas { self.remaining_gas = 0; return Err(Error::OutOfGas); }
        self.remaining_gas -= output.cost;
        self.pc += output.jump;
        for o in (0..O).rev() {
            self.stack.push(output.result[o])?;
        }
        Ok(TransitionOutput { cost: output.cost, jump: output.jump })
    }

    pub fn execute_next_opcode(&mut self) -> Result<TransitionOutput, Error> {
        match self.transaction.data.get(self.pc).unwrap_or(&0) {
            0x00 => self.execute_transition(STOP),
            0x01 => self.execute_transition(ADD),
            0x02 => self.execute_transition(MUL),
            0x03 => self.execute_transition(SUB),
            0x04 => self.execute_transition(DIV),
            0x05 => self.execute_transition(SDIV),
            0x06 => self.execute_transition(MOD),
            0x07 => self.execute_transition(SMOD),
            0x08 => self.execute_transition(ADDMOD),
            0x09 => self.execute_transition(MULMOD),
            0x0A => self.execute_transition(EXP),
            0x0B => self.execute_transition(SIGNEXTEND),
            0x10 => self.execute_transition(LT),
            0x11 => self.execute_transition(GT),
            0x12 => self.execute_transition(SLT),
            0x13 => self.execute_transition(SGT),
            0x14 => self.execute_transition(EQ),
            0x15 => self.execute_transition(ISZERO),
            0x16 => self.execute_transition(AND),
            0x17 => self.execute_transition(OR),
            0x18 => self.execute_transition(XOR),
            0x19 => self.execute_transition(NOT),
            0x1A => self.execute_transition(BYTE),
            0x1B => self.execute_transition(SHL),
            0x1C => self.execute_transition(SHR),
            0x1D => self.execute_transition(SAR),
            0x20 => self.execute_transition(KECCAK256),
            0x30 => self.execute_transition(ADDRESS),
            0x31 => self.execute_transition(BALANCE),
            0x32 => self.execute_transition(ORIGIN),
            0x33 => self.execute_transition(CALLER),
            0x34 => self.execute_transition(CALLVALUE),
            0x35 => self.execute_transition(CALLDATALOAD),
            0x36 => self.execute_transition(CALLDATASIZE),
            0x37 => self.execute_transition(CALLDATACOPY),
            0x38 => self.execute_transition(CODESIZE),
            0x39 => self.execute_transition(CODECOPY),
            0x3A => self.execute_transition(GASPRICE),
            0x3B => self.execute_transition(EXTCODESIZE),
            0x3C => self.execute_transition(EXTCODECOPY),
            0x3D => self.execute_transition(RETURNDATASIZE),
            0x3E => self.execute_transition(RETURNDATACOPY),
            0x3F => self.execute_transition(EXTCODEHASH),
            0x40 => self.execute_transition(BLOCKHASH),
            0x41 => self.execute_transition(COINBASE),
            0x42 => self.execute_transition(TIMESTAMP),
            0x43 => self.execute_transition(NUMBER),
            0x44 => self.execute_transition(PREVRANDAO),
            0x45 => self.execute_transition(GASLIMIT),
            0x46 => self.execute_transition(CHAINID),
            0x47 => self.execute_transition(SELFBALANCE),
            0x48 => self.execute_transition(BASEFEE),
            0x49 => self.execute_transition(BLOBHASH),
            0x4A => self.execute_transition(BLOBBASEFEE),
            0x50 => self.execute_transition(POP),
            0x51 => self.execute_transition(MLOAD),
            0x52 => self.execute_transition(MSTORE),
            0x53 => self.execute_transition(MSTORE8),
            0x54 => self.execute_transition(SLOAD),
            0x55 => self.execute_transition(SSTORE),
            0x56 => self.execute_transition(JUMP),
            0x57 => self.execute_transition(JUMPI),
            0x58 => self.execute_transition(PC),
            0x59 => self.execute_transition(MSIZE),
            0x5A => self.execute_transition(GAS),
            0x5B => self.execute_transition(JUMPDEST),
            0x5C => self.execute_transition(TLOAD),
            0x5D => self.execute_transition(TSTORE),
            0x5E => self.execute_transition(MCOPY),
            0x5F => self.execute_transition(PUSH0),
            0x60 => self.execute_transition(PUSH1),
            0x61 => self.execute_transition(PUSH2),
            0x62 => self.execute_transition(PUSH3),
            0x63 => self.execute_transition(PUSH4),
            0x64 => self.execute_transition(PUSH5),
            0x65 => self.execute_transition(PUSH6),
            0x66 => self.execute_transition(PUSH7),
            0x67 => self.execute_transition(PUSH8),
            0x68 => self.execute_transition(PUSH9),
            0x69 => self.execute_transition(PUSH10),
            0x6A => self.execute_transition(PUSH11),
            0x6B => self.execute_transition(PUSH12),
            0x6C => self.execute_transition(PUSH13),
            0x6D => self.execute_transition(PUSH14),
            0x6E => self.execute_transition(PUSH15),
            0x6F => self.execute_transition(PUSH16),
            0x70 => self.execute_transition(PUSH17),
            0x71 => self.execute_transition(PUSH18),
            0x72 => self.execute_transition(PUSH19),
            0x73 => self.execute_transition(PUSH20),
            0x74 => self.execute_transition(PUSH21),
            0x75 => self.execute_transition(PUSH22),
            0x76 => self.execute_transition(PUSH23),
            0x77 => self.execute_transition(PUSH24),
            0x78 => self.execute_transition(PUSH25),
            0x79 => self.execute_transition(PUSH26),
            0x7A => self.execute_transition(PUSH27),
            0x7B => self.execute_transition(PUSH28),
            0x7C => self.execute_transition(PUSH29),
            0x7D => self.execute_transition(PUSH30),
            0x7E => self.execute_transition(PUSH31),
            0x7F => self.execute_transition(PUSH32),
            0x80 => self.execute_transition(DUP1),
            0x81 => self.execute_transition(DUP2),
            0x82 => self.execute_transition(DUP3),
            0x83 => self.execute_transition(DUP4),
            0x84 => self.execute_transition(DUP5),
            0x85 => self.execute_transition(DUP6),
            0x86 => self.execute_transition(DUP7),
            0x87 => self.execute_transition(DUP8),
            0x88 => self.execute_transition(DUP9),
            0x89 => self.execute_transition(DUP10),
            0x8A => self.execute_transition(DUP11),
            0x8B => self.execute_transition(DUP12),
            0x8C => self.execute_transition(DUP13),
            0x8D => self.execute_transition(DUP14),
            0x8E => self.execute_transition(DUP15),
            0x8F => self.execute_transition(DUP16),
            0x90 => self.execute_transition(SWAP1),
            0x91 => self.execute_transition(SWAP2),
            0x92 => self.execute_transition(SWAP3),
            0x93 => self.execute_transition(SWAP4),
            0x94 => self.execute_transition(SWAP5),
            0x95 => self.execute_transition(SWAP6),
            0x96 => self.execute_transition(SWAP7),
            0x97 => self.execute_transition(SWAP8),
            0x98 => self.execute_transition(SWAP9),
            0x99 => self.execute_transition(SWAP10),
            0x9A => self.execute_transition(SWAP11),
            0x9B => self.execute_transition(SWAP12),
            0x9C => self.execute_transition(SWAP13),
            0x9D => self.execute_transition(SWAP14),
            0x9E => self.execute_transition(SWAP15),
            0x9F => self.execute_transition(SWAP16),
            0xA0 => self.execute_transition(LOG0),
            0xA1 => self.execute_transition(LOG1),
            0xA2 => self.execute_transition(LOG2),
            0xA3 => self.execute_transition(LOG3),
            0xA4 => self.execute_transition(LOG4),
            0xF0 => self.execute_transition(CREATE),
            0xF1 => self.execute_transition(CALL),
            0xF2 => self.execute_transition(CALLCODE),
            0xF3 => self.execute_transition(RETURN),
            0xF4 => self.execute_transition(DELEGATECALL),
            0xF5 => self.execute_transition(CREATE2),
            0xFA => self.execute_transition(STATICCALL),
            0xFD => self.execute_transition(REVERT),
            0xFE => self.execute_transition(INVALID),
            0xFF => self.execute_transition(SELFDESTRUCT),
            _ => self.execute_transition(INVALID),
        }
    }

}

#[cfg(test)]
mod tests {
    use crate::{transaction::Address, transitions::TransitionFunctionOutput};

    use super::*;
    use ethnum::{u256, uint};

    #[test]
    fn handles_gas() {
        let mut state = State::new(StateParameters { storage: Default::default(), accounts: Default::default(), transaction: Transaction { from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), data: Default::default(), gas: 7, value: U256::ZERO } });

        assert_eq!(state.remaining_gas, 7);

        assert_eq!(state.execute_transition(
            |_, _: [u256; 0]| Ok(TransitionFunctionOutput { cost: 3, result: [], jump: 1 })
        ), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.remaining_gas, 4);

        assert_eq!(state.execute_transition(
            |_, _: [u256; 0]| Ok(TransitionFunctionOutput { cost: 3, result: [], jump: 1 })
        ), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.remaining_gas, 1);

        assert_eq!(state.execute_transition(
            |_, _: [u256; 0]| Ok(TransitionFunctionOutput { cost: 3, result: [], jump: 1 })
        ), Err(Error::OutOfGas));
        assert_eq!(state.remaining_gas, 0);

        // the input transaction gas is untouched
        assert_eq!(state.transaction.gas, 7);
    }

    #[test]
    fn moves_code_pointer() {
        let mut state = State::new(StateParameters { storage: Default::default(), accounts: Default::default(), transaction: Transaction { from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), data: Default::default(), gas: 7, value: U256::ZERO } });

        assert_eq!(state.pc, 0);

        assert_eq!(state.execute_transition(
            |_, _: [u256; 0]| Ok(TransitionFunctionOutput { cost: 3, result: [], jump: 1 })
        ), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.pc, 1);

        assert_eq!(state.execute_transition(
            |_, _: [u256; 0]| Ok(TransitionFunctionOutput { cost: 3, result: [], jump: 2 })
        ), Ok(TransitionOutput { cost: 3, jump: 2 }));
        assert_eq!(state.pc, 3);
    }

    #[test]
    fn transition_builder_fails_if_not_enough_parmeters_in_stack() {
        let mut state = State::new(StateParameters { storage: Default::default(), accounts: Default::default(), transaction: Transaction { from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), data: Default::default(), gas: 20, value: U256::ZERO } });

        assert_eq!(state.execute_transition(
            |_, input: [u256; 1]| Ok(TransitionFunctionOutput { cost: 3, result: [input[0]], jump: 1 })
        ), Err(Error::EmptyStack));
    }

    #[test]
    fn transition_builder_fails_if_too_much_outputs() {
        let mut state = State::new(StateParameters { storage: Default::default(), accounts: Default::default(), transaction: Transaction { from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), data: Default::default(), gas: 20, value: U256::ZERO } });

        assert_eq!(state.execute_transition(
            |_, _input: [u256; 0]| Ok(TransitionFunctionOutput { cost: 3, result: [U256::ZERO; 1025], jump: 1 })
        ), Err(Error::StackOverflow));
    }

    #[test]
    fn transition_builder_fails_if_transition_function_fails() {
        let mut state = State::new(StateParameters { storage: Default::default(), accounts: Default::default(), transaction: Transaction { from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), data: Default::default(), gas: 20, value: U256::ZERO } });

        assert_eq!(state.execute_transition(
            |_, _input: [u256; 0]| Result::<TransitionFunctionOutput<0>, Error>::Err(Error::InvalidJumpDest)
        ), Err(Error::InvalidJumpDest));
    }

    #[test]
    fn preserve_stack_order() {
        let mut state = State::new(StateParameters { storage: Default::default(), accounts: Default::default(), transaction: Transaction { from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), data: Default::default(), gas: 20, value: U256::ZERO } });

        state.stack.push(uint!("0x0C")).unwrap();
        state.stack.push(uint!("0x0B")).unwrap();
        state.stack.push(uint!("0x0A")).unwrap();

        assert_eq!(state.execute_transition(
            |_, input: [u256; 3]| Ok(TransitionFunctionOutput { cost: 3, result: input, jump: 1 })
        ), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0x0A")));
        assert_eq!(state.stack.pop(), Some(uint!("0x0B")));
        assert_eq!(state.stack.pop(), Some(uint!("0x0C")));
        assert_eq!(state.stack.pop(), None);
    }
}
