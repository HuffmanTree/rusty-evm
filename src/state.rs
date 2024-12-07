use std::collections::HashMap;
use ethnum::{u256, U256};
use crate::errors::Error;
use crate::memory::Memory;
use crate::stack::Stack;
use crate::storage::Storage;
use crate::transaction::{Address, Transaction};
use crate::transient::Transient;
use crate::transitions::{TransitionContext, TransitionFunction, TransitionOutput, ADD, ADDMOD, ADDRESS, AND, BALANCE, BYTE, CALLDATACOPY, CALLDATALOAD, CALLDATASIZE, CALLER, CALLVALUE, DIV, DUP1, DUP10, DUP11, DUP12, DUP13, DUP14, DUP15, DUP16, DUP2, DUP3, DUP4, DUP5, DUP6, DUP7, DUP8, DUP9, EQ, EXP, GAS, GT, INVALID, ISZERO, JUMP, JUMPDEST, JUMPI, KECCAK256, LT, MCOPY, MLOAD, MOD, MSIZE, MSTORE, MSTORE8, MUL, MULMOD, NOT, OR, ORIGIN, PC, POP, PUSH0, PUSH1, PUSH10, PUSH11, PUSH12, PUSH13, PUSH14, PUSH15, PUSH16, PUSH17, PUSH18, PUSH19, PUSH2, PUSH20, PUSH21, PUSH22, PUSH23, PUSH24, PUSH25, PUSH26, PUSH27, PUSH28, PUSH29, PUSH3, PUSH30, PUSH31, PUSH32, PUSH4, PUSH5, PUSH6, PUSH7, PUSH8, PUSH9, RETURN, REVERT, SAR, SDIV, SGT, SHL, SHR, SIGNEXTEND, SLOAD, SLT, SMOD, SSTORE, STOP, SUB, SWAP1, SWAP10, SWAP11, SWAP12, SWAP13, SWAP14, SWAP15, SWAP16, SWAP2, SWAP3, SWAP4, SWAP5, SWAP6, SWAP7, SWAP8, SWAP9, TLOAD, TSTORE, XOR};

struct State {
    accounts: Storage<Address, u256>,
    latest_caller: Address,
    remaining_gas: usize,
    stack: Stack,
    memory: Memory,
    storage: Storage<u256, u256>,
    stop_flag: bool,
    pc: usize,
    returndata: Vec<u8>,
    revert_flag: bool,
    transaction: Transaction,
    transient: Transient,
}

struct StateParameters {
    initial_accounts: HashMap::<Address, u256>,
    initial_storage: HashMap::<u256, u256>,
    transaction: Transaction,
}

impl State {
    fn new(parameters: StateParameters) -> Self {
        Self {
            accounts: Storage::new(parameters.initial_accounts),
            latest_caller: parameters.transaction.from,
            remaining_gas: parameters.transaction.gas,
            stack: Stack::new(),
            memory: Memory::new(),
            storage: Storage::new(parameters.initial_storage),
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

    fn stop(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(STOP) }
    fn add(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(ADD) }
    fn mul(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(MUL) }
    fn sub(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SUB) }
    fn div(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DIV) }
    fn sdiv(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SDIV) }
    fn r#mod(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(MOD) }
    fn smod(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SMOD) }
    fn addmod(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(ADDMOD) }
    fn mulmod(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(MULMOD) }
    fn exp(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(EXP) }
    fn signextend(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SIGNEXTEND) }
    fn lt(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(LT) }
    fn gt(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(GT) }
    fn slt(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SLT) }
    fn sgt(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SGT) }
    fn eq(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(EQ) }
    fn iszero(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(ISZERO) }
    fn and(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(AND) }
    fn or(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(OR) }
    fn xor(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(XOR) }
    fn not(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(NOT) }
    fn byte(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(BYTE) }
    fn shl(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SHL) }
    fn shr(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SHR) }
    fn sar(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SAR) }
    fn keccak256(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(KECCAK256) }
    fn address(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(ADDRESS) }
    fn balance(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(BALANCE) }
    fn origin(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(ORIGIN) }
    fn caller(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(CALLER) }
    fn callvalue(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(CALLVALUE) }
    fn calldataload(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(CALLDATALOAD) }
    fn calldatasize(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(CALLDATASIZE) }
    fn calldatacopy(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(CALLDATACOPY) }
    fn pop(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(POP) }
    fn mload(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(MLOAD) }
    fn mstore(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(MSTORE) }
    fn mstore8(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(MSTORE8) }
    fn sload(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SLOAD) }
    fn sstore(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SSTORE) }
    fn jump(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(JUMP) }
    fn jumpi(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(JUMPI) }
    fn pc(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PC) }
    fn msize(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(MSIZE) }
    fn gas(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(GAS) }
    fn jumpdest(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(JUMPDEST) }
    fn tload(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(TLOAD) }
    fn tstore(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(TSTORE) }
    fn mcopy(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(MCOPY) }
    fn push0(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH0) }
    fn push1(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH1) }
    fn push2(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH2) }
    fn push3(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH3) }
    fn push4(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH4) }
    fn push5(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH5) }
    fn push6(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH6) }
    fn push7(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH7) }
    fn push8(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH8) }
    fn push9(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH9) }
    fn push10(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH10) }
    fn push11(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH11) }
    fn push12(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH12) }
    fn push13(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH13) }
    fn push14(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH14) }
    fn push15(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH15) }
    fn push16(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH16) }
    fn push17(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH17) }
    fn push18(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH18) }
    fn push19(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH19) }
    fn push20(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH20) }
    fn push21(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH21) }
    fn push22(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH22) }
    fn push23(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH23) }
    fn push24(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH24) }
    fn push25(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH25) }
    fn push26(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH26) }
    fn push27(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH27) }
    fn push28(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH28) }
    fn push29(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH29) }
    fn push30(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH30) }
    fn push31(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH31) }
    fn push32(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(PUSH32) }
    fn dup1(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP1) }
    fn dup2(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP2) }
    fn dup3(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP3) }
    fn dup4(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP4) }
    fn dup5(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP5) }
    fn dup6(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP6) }
    fn dup7(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP7) }
    fn dup8(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP8) }
    fn dup9(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP9) }
    fn dup10(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP10) }
    fn dup11(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP11) }
    fn dup12(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP12) }
    fn dup13(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP13) }
    fn dup14(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP14) }
    fn dup15(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP15) }
    fn dup16(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(DUP16) }
    fn swap1(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP1) }
    fn swap2(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP2) }
    fn swap3(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP3) }
    fn swap4(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP4) }
    fn swap5(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP5) }
    fn swap6(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP6) }
    fn swap7(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP7) }
    fn swap8(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP8) }
    fn swap9(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP9) }
    fn swap10(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP10) }
    fn swap11(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP11) }
    fn swap12(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP12) }
    fn swap13(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP13) }
    fn swap14(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP14) }
    fn swap15(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP15) }
    fn swap16(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(SWAP16) }
    fn r#return(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(RETURN) }
    fn revert(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(REVERT) }
    fn invalid(&mut self) -> Result<TransitionOutput, Error> { self.execute_transition(INVALID) }
}

#[cfg(test)]
mod tests {
    use crate::{transaction::Address, transitions::TransitionFunctionOutput};

    use super::*;
    use ethnum::{u256, uint};

    #[test]
    fn handles_gas() {
        let mut state = State::new(StateParameters { initial_storage: Default::default(), initial_accounts: Default::default(), transaction: Transaction { from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), data: Default::default(), gas: 7, value: U256::ZERO } });

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
        let mut state = State::new(StateParameters { initial_storage: Default::default(), initial_accounts: Default::default(), transaction: Transaction { from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), data: Default::default(), gas: 7, value: U256::ZERO } });

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
        let mut state = State::new(StateParameters { initial_storage: Default::default(), initial_accounts: Default::default(), transaction: Transaction { from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), data: Default::default(), gas: 20, value: U256::ZERO } });

        assert_eq!(state.execute_transition(
            |_, input: [u256; 1]| Ok(TransitionFunctionOutput { cost: 3, result: [input[0]], jump: 1 })
        ), Err(Error::EmptyStack));
    }

    #[test]
    fn transition_builder_fails_if_too_much_outputs() {
        let mut state = State::new(StateParameters { initial_storage: Default::default(), initial_accounts: Default::default(), transaction: Transaction { from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), data: Default::default(), gas: 20, value: U256::ZERO } });

        assert_eq!(state.execute_transition(
            |_, _input: [u256; 0]| Ok(TransitionFunctionOutput { cost: 3, result: [U256::ZERO; 1025], jump: 1 })
        ), Err(Error::StackOverflow));
    }

    #[test]
    fn transition_builder_fails_if_transition_function_fails() {
        let mut state = State::new(StateParameters { initial_storage: Default::default(), initial_accounts: Default::default(), transaction: Transaction { from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), data: Default::default(), gas: 20, value: U256::ZERO } });

        assert_eq!(state.execute_transition(
            |_, _input: [u256; 0]| Result::<TransitionFunctionOutput<0>, Error>::Err(Error::InvalidJumpDest)
        ), Err(Error::InvalidJumpDest));
    }

    #[test]
    fn preserve_stack_order() {
        let mut state = State::new(StateParameters { initial_storage: Default::default(), initial_accounts: Default::default(), transaction: Transaction { from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), data: Default::default(), gas: 20, value: U256::ZERO } });

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
