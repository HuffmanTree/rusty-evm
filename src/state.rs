use std::collections::HashMap;
use ethnum::{u256, U256};
use crate::memory::Memory;
use crate::stack::Stack;
use crate::storage::Storage;
use crate::transaction::Transaction;
use crate::transient::Transient;
use crate::transitions::{TransitionContext, TransitionFunction, TransitionOutput, ADD, ADDMOD, AND, BYTE, DIV, EQ, EXP, GAS, GT, ISZERO, JUMP, JUMPDEST, JUMPI, LT, MLOAD, MOD, MSIZE, MSTORE, MSTORE8, MUL, MULMOD, NOT, OR, PC, POP, PUSH0, PUSH1, PUSH2, PUSH3, PUSH4, PUSH5, PUSH6, PUSH7, PUSH8, PUSH9, PUSH10, PUSH11, PUSH12, PUSH13, PUSH14, PUSH15, PUSH16, PUSH17, PUSH18, PUSH19, PUSH20, PUSH21, PUSH22, PUSH23, PUSH24, PUSH25, PUSH26, PUSH27, PUSH28, PUSH29, PUSH30, PUSH31, PUSH32, SAR, SDIV, SGT, SHL, SHR, SIGNEXTEND, SLOAD, SLT, SMOD, SSTORE, STOP, SUB, XOR};

struct State {
    remaining_gas: usize,
    stack: Stack,
    memory: Memory,
    storage: Storage,
    stop_flag: bool,
    pc: usize,
    transaction: Transaction,
    transient: Transient,
}

struct StateParameters {
    initial_storage: HashMap::<u256, u256>,
    transaction: Transaction,
}

impl State {
    fn new(parameters: StateParameters) -> Self {
        Self {
            remaining_gas: parameters.transaction.gas,
            stack: Stack::new(),
            memory: Memory::new(),
            storage: Storage::new(parameters.initial_storage),
            stop_flag: false,
            pc: 0,
            transaction: parameters.transaction,
            transient: Transient::new(),
        }
    }

    fn execute_transition<const I: usize, const O: usize>(&mut self, f: TransitionFunction<I, O>) -> Result<TransitionOutput, String> {
        let mut context = TransitionContext {
            code: &self.transaction.data,
            gas: &self.remaining_gas,
            memory: &mut self.memory,
            pc: &mut self.pc,
            stop_flag: &mut self.stop_flag,
            storage: &mut self.storage,
            transient: &mut self.transient,
        };
        let mut input = [U256::ZERO; I];
        for i in 0..I {
            input[i] = match self.stack.pop() {
                Some(x) => x,
                _ => return Err("Stack is empty".to_string()),
            }
        };
        let output = f(&mut context, input)?;
        if output.cost > self.remaining_gas { self.remaining_gas = 0; return Err("Out of gas".to_string()); }
        self.remaining_gas -= output.cost;
        for o in (0..O).rev() {
            if let Err(e) = self.stack.push(output.result[o]) {
                return Err(e.to_string());
            }
        }
        Ok(TransitionOutput { cost: output.cost, jump: output.jump })
    }

    fn stop(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(STOP) }
    fn add(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(ADD) }
    fn mul(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(MUL) }
    fn sub(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(SUB) }
    fn div(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(DIV) }
    fn sdiv(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(SDIV) }
    fn r#mod(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(MOD) }
    fn smod(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(SMOD) }
    fn addmod(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(ADDMOD) }
    fn mulmod(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(MULMOD) }
    fn exp(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(EXP) }
    fn signextend(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(SIGNEXTEND) }
    fn lt(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(LT) }
    fn gt(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(GT) }
    fn slt(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(SLT) }
    fn sgt(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(SGT) }
    fn eq(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(EQ) }
    fn iszero(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(ISZERO) }
    fn and(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(AND) }
    fn or(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(OR) }
    fn xor(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(XOR) }
    fn not(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(NOT) }
    fn byte(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(BYTE) }
    fn shl(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(SHL) }
    fn shr(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(SHR) }
    fn sar(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(SAR) }
    fn pop(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(POP) }
    fn mload(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(MLOAD) }
    fn mstore(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(MSTORE) }
    fn mstore8(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(MSTORE8) }
    fn sload(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(SLOAD) }
    fn sstore(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(SSTORE) }
    fn jump(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(JUMP) }
    fn jumpi(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(JUMPI) }
    fn pc(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PC) }
    fn msize(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(MSIZE) }
    fn gas(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(GAS) }
    fn jumpdest(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(JUMPDEST) }
    fn push0(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH0) }
    fn push1(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH1) }
    fn push2(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH2) }
    fn push3(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH3) }
    fn push4(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH4) }
    fn push5(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH5) }
    fn push6(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH6) }
    fn push7(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH7) }
    fn push8(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH8) }
    fn push9(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH9) }
    fn push10(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH10) }
    fn push11(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH11) }
    fn push12(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH12) }
    fn push13(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH13) }
    fn push14(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH14) }
    fn push15(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH15) }
    fn push16(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH16) }
    fn push17(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH17) }
    fn push18(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH18) }
    fn push19(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH19) }
    fn push20(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH20) }
    fn push21(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH21) }
    fn push22(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH22) }
    fn push23(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH23) }
    fn push24(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH24) }
    fn push25(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH25) }
    fn push26(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH26) }
    fn push27(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH27) }
    fn push28(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH28) }
    fn push29(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH29) }
    fn push30(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH30) }
    fn push31(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH31) }
    fn push32(&mut self) -> Result<TransitionOutput, String> { self.execute_transition(PUSH32) }
}

#[cfg(test)]
mod tests {
    use crate::transitions::TransitionFunctionOutput;

    use super::*;
    use ethnum::{u256, uint};

    #[test]
    fn handles_gas() {
        let mut state = State::new(StateParameters { initial_storage: Default::default(), transaction: Transaction { data: Default::default(), gas: 7 } });

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
        ), Err("Out of gas".to_string()));
        assert_eq!(state.remaining_gas, 0);

        // the input transaction gas is untouched
        assert_eq!(state.transaction.gas, 7);
    }

    #[test]
    fn transition_builder_fails_if_not_enough_parmeters_in_stack() {
        let mut state = State::new(StateParameters { initial_storage: Default::default(), transaction: Transaction { data: Default::default(), gas: 20 } });

        assert_eq!(state.execute_transition(
            |_, input: [u256; 1]| Ok(TransitionFunctionOutput { cost: 3, result: [input[0]], jump: 1 })
        ), Err("Stack is empty".to_string()));
    }

    #[test]
    fn transition_builder_fails_if_too_much_outputs() {
        let mut state = State::new(StateParameters { initial_storage: Default::default(), transaction: Transaction { data: Default::default(), gas: 20 } });

        assert_eq!(state.execute_transition(
            |_, _input: [u256; 0]| Ok(TransitionFunctionOutput { cost: 3, result: [U256::ZERO; 1025], jump: 1 })
        ), Err("Stack overflow".to_string()));
    }

    #[test]
    fn transition_builder_fails_if_transition_function_fails() {
        let mut state = State::new(StateParameters { initial_storage: Default::default(), transaction: Transaction { data: Default::default(), gas: 20 } });

        assert_eq!(state.execute_transition(
            |_, _input: [u256; 0]| Result::<TransitionFunctionOutput<0>, String>::Err("Error message".to_string())
        ), Err("Error message".to_string()));
    }

    #[test]
    fn preserve_stack_order() {
        let mut state = State::new(StateParameters { initial_storage: Default::default(), transaction: Transaction { data: Default::default(), gas: 20 } });

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
