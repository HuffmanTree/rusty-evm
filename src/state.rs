use std::collections::HashMap;
use ethnum::{u256, U256};
use crate::memory::Memory;
use crate::stack::Stack;
use crate::storage::Storage;
use crate::transaction::Transaction;
use crate::transitions::{TransitionContext, TransitionFunction, TransitionOutput, ADD, ADDMOD, AND, BYTE, DIV, EQ, EXP, GAS, GT, ISZERO, JUMP, JUMPDEST, JUMPI, LT, MLOAD, MOD, MSIZE, MSTORE, MSTORE8, MUL, MULMOD, NOT, OR, PC, POP, SAR, SDIV, SGT, SHL, SHR, SIGNEXTEND, SLOAD, SLT, SMOD, SSTORE, STOP, SUB, XOR};

struct State {
    remaining_gas: usize,
    stack: Stack,
    memory: Memory,
    storage: Storage,
    stop_flag: bool,
    pc: usize,
    transaction: Transaction,
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
        }
    }

    fn transition_builder<const I: usize, const O: usize>(&mut self, f: TransitionFunction<I, O>) -> Result<TransitionOutput, String> {
        let mut context = TransitionContext {
            code: &self.transaction.data,
            gas: &self.remaining_gas,
            memory: &mut self.memory,
            pc: &mut self.pc,
            stop_flag: &mut self.stop_flag,
            storage: &mut self.storage,
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
        for o in 0..O {
            if let Err(e) = self.stack.push(output.result[o]) {
                return Err(e.to_string());
            }
        }
        Ok(TransitionOutput { cost: output.cost, jump: output.jump })
    }

    fn stop(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(STOP) }
    fn add(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(ADD) }
    fn mul(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(MUL) }
    fn sub(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(SUB) }
    fn div(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(DIV) }
    fn sdiv(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(SDIV) }
    fn r#mod(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(MOD) }
    fn smod(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(SMOD) }
    fn addmod(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(ADDMOD) }
    fn mulmod(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(MULMOD) }
    fn exp(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(EXP) }
    fn signextend(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(SIGNEXTEND) }
    fn lt(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(LT) }
    fn gt(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(GT) }
    fn slt(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(SLT) }
    fn sgt(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(SGT) }
    fn eq(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(EQ) }
    fn iszero(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(ISZERO) }
    fn and(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(AND) }
    fn or(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(OR) }
    fn xor(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(XOR) }
    fn not(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(NOT) }
    fn byte(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(BYTE) }
    fn shl(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(SHL) }
    fn shr(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(SHR) }
    fn sar(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(SAR) }
    fn pop(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(POP) }
    fn mload(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(MLOAD) }
    fn mstore(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(MSTORE) }
    fn mstore8(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(MSTORE8) }
    fn sload(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(SLOAD) }
    fn sstore(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(SSTORE) }
    fn jump(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(JUMP) }
    fn jumpi(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(JUMPI) }
    fn pc(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(PC) }
    fn msize(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(MSIZE) }
    fn gas(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(GAS) }
    fn jumpdest(&mut self) -> Result<TransitionOutput, String> { self.transition_builder(JUMPDEST) }
}

#[cfg(test)]
mod tests {
    use crate::{storage::StorageValue, transitions::TransitionFunctionOutput};

    use super::*;
    use ethnum::{uint,u256};

    #[test]
    fn handles_gas() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 7 } });

        assert_eq!(state.remaining_gas, 7);

        assert_eq!(state.transition_builder(
            |_, _: [u256; 0]| Ok(TransitionFunctionOutput { cost: 3, result: [], jump: 1 })
        ), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.remaining_gas, 4);

        assert_eq!(state.transition_builder(
            |_, _: [u256; 0]| Ok(TransitionFunctionOutput { cost: 3, result: [], jump: 1 })
        ), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.remaining_gas, 1);

        assert_eq!(state.transition_builder(
            |_, _: [u256; 0]| Ok(TransitionFunctionOutput { cost: 3, result: [], jump: 1 })
        ), Err("Out of gas".to_string()));
        assert_eq!(state.remaining_gas, 0);

        // the input transaction gas is untouched
        assert_eq!(state.transaction.gas, 7);
    }

    #[test]
    fn transition_builder_fails_if_not_enough_parmeters_in_stack() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        assert_eq!(state.transition_builder(
            |_, input: [u256; 1]| Ok(TransitionFunctionOutput { cost: 3, result: [input[0]], jump: 1 })
        ), Err("Stack is empty".to_string()));
    }

    #[test]
    fn transition_builder_fails_if_too_much_outputs() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        assert_eq!(state.transition_builder(
            |_, _input: [u256; 0]| Ok(TransitionFunctionOutput { cost: 3, result: [U256::ZERO; 1025], jump: 1 })
        ), Err("Stack overflow".to_string()));
    }

    #[test]
    fn transition_builder_fails_if_transition_function_fails() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        assert_eq!(state.transition_builder(
            |_, _input: [u256; 0]| Result::<TransitionFunctionOutput<0>, String>::Err("Error message".to_string())
        ), Err("Error message".to_string()));
    }

    #[test]
    fn set_the_stop_flag_to_true() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        assert!(!state.stop_flag);
        assert_eq!(state.stop(), Ok(TransitionOutput { cost: 0, jump: 0 }));
        assert!(state.stop_flag);
    }

    #[test]
    fn adds_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("5")).unwrap();
        state.stack.push(uint!("6")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.add(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("16")));
        assert_eq!(state.stack.pop(), Some(uint!("5")));
        assert_eq!(state.stack.pop(), None);
    }

    #[test]
    fn adds_with_an_overflow() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();
        state.stack.push(uint!("1")).unwrap();

        assert_eq!(state.add(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn fails_to_add_if_not_enough_items() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        assert_eq!(state.add(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn multiplies_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("5")).unwrap();
        state.stack.push(uint!("6")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.mul(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("60")));
        assert_eq!(state.stack.pop(), Some(uint!("5")));
        assert_eq!(state.stack.pop(), None);
    }

    #[test]
    fn multiplies_with_an_overflow() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();
        state.stack.push(uint!("2")).unwrap();

        assert_eq!(state.mul(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")));
    }

    #[test]
    fn fails_to_multiply_if_not_enough_items() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        assert_eq!(state.mul(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn subtracts_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("5")).unwrap();
        state.stack.push(uint!("6")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.sub(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("4")));
        assert_eq!(state.stack.pop(), Some(uint!("5")));
        assert_eq!(state.stack.pop(), None);
    }

    #[test]
    fn subtracts_with_an_overflow() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("1")).unwrap();
        state.stack.push(uint!("0")).unwrap();

        assert_eq!(state.sub(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")));
    }

    #[test]
    fn fails_to_subtract_if_not_enough_items() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        assert_eq!(state.sub(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn divides_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("5")).unwrap();
        state.stack.push(uint!("6")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.div(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));
        assert_eq!(state.stack.pop(), Some(uint!("5")));
        assert_eq!(state.stack.pop(), None);
    }

    #[test]
    fn dividing_by_zero_returns_zero_by_convention() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("6")).unwrap();

        assert_eq!(state.div(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn fails_to_divide_if_not_enough_items() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        assert_eq!(state.div(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn sign_divides_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("5")).unwrap();
        state.stack.push(uint!("6")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.sdiv(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));
        assert_eq!(state.stack.pop(), Some(uint!("5")));
        assert_eq!(state.stack.pop(), None);
    }

    #[test]
    fn sign_divides_with_negations() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")).unwrap();

        assert_eq!(state.sdiv(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("2")));

        state.stack.push(uint!("1")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")).unwrap();

        assert_eq!(state.sdiv(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")));
    }

    #[test]
    fn sign_dividing_by_zero_returns_zero_by_convention() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("6")).unwrap();

        assert_eq!(state.sdiv(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn fails_to_sign_divide_if_not_enough_items() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        assert_eq!(state.sdiv(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn takes_the_reminder_of_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("5")).unwrap();
        state.stack.push(uint!("6")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.r#mod(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("4")));
        assert_eq!(state.stack.pop(), Some(uint!("5")));
        assert_eq!(state.stack.pop(), None);
    }

    #[test]
    fn taking_the_reminder_by_zero_returns_zero_by_convention() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("6")).unwrap();

        assert_eq!(state.r#mod(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn fails_to_take_the_reminder_if_not_enough_items() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        assert_eq!(state.r#mod(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn sign_rems_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("5")).unwrap();
        state.stack.push(uint!("6")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.smod(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("4")));
        assert_eq!(state.stack.pop(), Some(uint!("5")));
        assert_eq!(state.stack.pop(), None);
    }

    #[test]
    fn sign_rems_with_negations() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFD")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF8")).unwrap();

        assert_eq!(state.smod(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")));

        state.stack.push(uint!("2")).unwrap();
        state.stack.push(uint!("3")).unwrap();

        assert_eq!(state.smod(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));

        state.stack.push(uint!("2")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFD")).unwrap();

        assert_eq!(state.smod(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")));

        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")).unwrap();
        state.stack.push(uint!("3")).unwrap();

        assert_eq!(state.smod(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));
    }

    #[test]
    fn sign_reming_by_zero_returns_zero_by_convention() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("6")).unwrap();

        assert_eq!(state.sdiv(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn fails_to_sign_rem_if_not_enough_items() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        assert_eq!(state.smod(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn adds_modulo() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 100 } });
        state.stack.push(uint!("8")).unwrap();
        state.stack.push(uint!("10")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.addmod(), Ok(TransitionOutput { cost: 8, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("4")));

        state.stack.push(uint!("2")).unwrap();
        state.stack.push(uint!("2")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();

        assert_eq!(state.addmod(), Ok(TransitionOutput { cost: 8, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));

        state.stack.push(uint!("3")).unwrap();
        state.stack.push(uint!("2")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFD")).unwrap();

        assert_eq!(state.addmod(), Ok(TransitionOutput { cost: 8, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("10")).unwrap();
        state.stack.push(uint!("1")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();

        assert_eq!(state.addmod(), Ok(TransitionOutput { cost: 8, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("6")));
    }

    #[test]
    fn add_modulo_by_zero_returns_zero_by_convention() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("6")).unwrap();
        state.stack.push(uint!("4")).unwrap();

        assert_eq!(state.addmod(), Ok(TransitionOutput { cost: 8, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn fails_to_add_modulo_if_not_enough_items() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        assert_eq!(state.addmod(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn multiplies_modulo() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 24 } });
        state.stack.push(uint!("8")).unwrap();
        state.stack.push(uint!("10")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.mulmod(), Ok(TransitionOutput { cost: 8, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("4")));

        state.stack.push(uint!("12")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();

        assert_eq!(state.mulmod(), Ok(TransitionOutput { cost: 8, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("9")));

        state.stack.push(uint!("3")).unwrap();
        state.stack.push(uint!("2")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFD")).unwrap();

        assert_eq!(state.mulmod(), Ok(TransitionOutput { cost: 8, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("2")));
    }

    #[test]
    fn multiply_modulo_by_zero_returns_zero_by_convention() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("6")).unwrap();
        state.stack.push(uint!("4")).unwrap();

        assert_eq!(state.mulmod(), Ok(TransitionOutput { cost: 8, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn fails_to_multiply_modulo_if_not_enough_items() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        assert_eq!(state.mulmod(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn exponentiates() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 1400 } });
        state.stack.push(uint!("2")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.exp(), Ok(TransitionOutput { cost: 60, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("100")));

        state.stack.push(uint!("2")).unwrap();
        state.stack.push(uint!("2")).unwrap();

        assert_eq!(state.exp(), Ok(TransitionOutput { cost: 60, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("4")));

        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("5")).unwrap();

        assert_eq!(state.exp(), Ok(TransitionOutput { cost: 10, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));

        state.stack.push(uint!("10")).unwrap();
        state.stack.push(uint!("2")).unwrap();

        assert_eq!(state.exp(), Ok(TransitionOutput { cost: 60, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1024")));

        state.stack.push(uint!("260")).unwrap();
        state.stack.push(uint!("2")).unwrap();

        assert_eq!(state.exp(), Ok(TransitionOutput { cost: 110, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("3")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFF")).unwrap();

        assert_eq!(state.exp(), Ok(TransitionOutput { cost: 60, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFFFFFFFFFFFFFFFD0000000000000002FFFFFFFFFFFFFFFF")));

        state.stack.push(uint!("0xFFFFFFFFFFFFFFF0")).unwrap();
        state.stack.push(uint!("3")).unwrap();

        assert_eq!(state.exp(), Ok(TransitionOutput { cost: 410, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xE9377A20E36295B65EA7F55D4A333F73CF25A1BE32FEBCF9702BDE500F57B8C1")));

        state.stack.push(uint!("0xFFFFFFFFFFFFFFF0FFFFFF")).unwrap();
        state.stack.push(uint!("5")).unwrap();

        assert_eq!(state.exp(), Ok(TransitionOutput { cost: 560, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0x49E63006C06484CE7E18DB842AD1771FC1C83AA03B09227A2EB3765958CCCCCD")));
    }

    #[test]
    fn fails_to_exponentiate_if_not_enough_items() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        assert_eq!(state.exp(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn sign_extends() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 200 } });
        state.stack.push(uint!("0x41")).unwrap();
        state.stack.push(uint!("0")).unwrap();

        assert_eq!(state.signextend(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0x41")));

        state.stack.push(uint!("0xEF41")).unwrap();
        state.stack.push(uint!("0")).unwrap();

        assert_eq!(state.signextend(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0x41")));

        state.stack.push(uint!("0xEF41")).unwrap();
        state.stack.push(uint!("1")).unwrap();

        assert_eq!(state.signextend(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEF41")));

        state.stack.push(uint!("0xEF41")).unwrap();
        state.stack.push(uint!("2")).unwrap();

        assert_eq!(state.signextend(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xEF41")));

        state.stack.push(uint!("0xEF41")).unwrap();
        state.stack.push(uint!("30")).unwrap();

        assert_eq!(state.signextend(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xEF41")));

        state.stack.push(uint!("0xEF41")).unwrap();
        state.stack.push(uint!("31")).unwrap();

        assert_eq!(state.signextend(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xEF41")));

        state.stack.push(uint!("0xEF41")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();

        assert_eq!(state.signextend(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xEF41")));
    }

    #[test]
    fn fails_to_sign_extend_if_not_enough_items() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        assert_eq!(state.signextend(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn compare_values() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 100 } });
        assert_eq!(state.lt(), Err("Stack is empty".to_string()));
        assert_eq!(state.gt(), Err("Stack is empty".to_string()));

        state.stack.push(uint!("10")).unwrap();
        state.stack.push(uint!("9")).unwrap();

        assert_eq!(state.lt(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));

        state.stack.push(uint!("10")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.lt(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("9")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.gt(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));

        state.stack.push(uint!("10")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.gt(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("10")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.eq(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));

        state.stack.push(uint!("10")).unwrap();
        state.stack.push(uint!("3")).unwrap();

        assert_eq!(state.eq(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("0")).unwrap();

        assert_eq!(state.iszero(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));

        state.stack.push(uint!("3")).unwrap();

        assert_eq!(state.iszero(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn compare_signed_values() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 100 } });
        assert_eq!(state.slt(), Err("Stack is empty".to_string()));
        assert_eq!(state.sgt(), Err("Stack is empty".to_string()));

        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();

        assert_eq!(state.slt(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));

        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();

        assert_eq!(state.slt(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();
        state.stack.push(uint!("0")).unwrap();

        assert_eq!(state.slt(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("2")).unwrap();
        state.stack.push(uint!("1")).unwrap();

        assert_eq!(state.slt(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));

        state.stack.push(uint!("10")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.slt(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();

        assert_eq!(state.sgt(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();

        assert_eq!(state.sgt(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));

        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();
        state.stack.push(uint!("0")).unwrap();

        assert_eq!(state.sgt(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));

        state.stack.push(uint!("2")).unwrap();
        state.stack.push(uint!("1")).unwrap();

        assert_eq!(state.sgt(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("10")).unwrap();
        state.stack.push(uint!("10")).unwrap();

        assert_eq!(state.sgt(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn bitwise_operations() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 300 } });
        assert_eq!(state.and(), Err("Stack is empty".to_string()));
        assert_eq!(state.or(), Err("Stack is empty".to_string()));
        assert_eq!(state.xor(), Err("Stack is empty".to_string()));
        assert_eq!(state.not(), Err("Stack is empty".to_string()));
        assert_eq!(state.byte(), Err("Stack is empty".to_string()));
        assert_eq!(state.shr(), Err("Stack is empty".to_string()));
        assert_eq!(state.shl(), Err("Stack is empty".to_string()));

        state.stack.push(uint!("0xFF")).unwrap();
        state.stack.push(uint!("0xFF")).unwrap();

        assert_eq!(state.and(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFF")));

        state.stack.push(uint!("0xFF")).unwrap();
        state.stack.push(uint!("0")).unwrap();

        assert_eq!(state.and(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("0x0F")).unwrap();
        state.stack.push(uint!("0xF0")).unwrap();

        assert_eq!(state.or(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFF")));

        state.stack.push(uint!("0xFF")).unwrap();
        state.stack.push(uint!("0")).unwrap();

        assert_eq!(state.or(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFF")));

        state.stack.push(uint!("0xFF")).unwrap();
        state.stack.push(uint!("0xFF")).unwrap();

        assert_eq!(state.or(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFF")));

        state.stack.push(uint!("0x0F")).unwrap();
        state.stack.push(uint!("0xF0")).unwrap();

        assert_eq!(state.xor(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFF")));

        state.stack.push(uint!("0xFF")).unwrap();
        state.stack.push(uint!("0")).unwrap();

        assert_eq!(state.xor(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFF")));

        state.stack.push(uint!("0xFF")).unwrap();
        state.stack.push(uint!("0xFF")).unwrap();

        assert_eq!(state.xor(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("0")).unwrap();

        assert_eq!(state.not(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")));

        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();

        assert_eq!(state.not(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("0x0112233445566778899AABBCCDDEEFF0")).unwrap();
        state.stack.push(uint!("16")).unwrap();

        assert_eq!(state.byte(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));

        state.stack.push(uint!("0x0112233445566778899AABBCCDDEEFF0")).unwrap();
        state.stack.push(uint!("31")).unwrap();

        assert_eq!(state.byte(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xF0")));

        state.stack.push(uint!("0x0112233445566778899AABBCCDDEEFF0")).unwrap();
        state.stack.push(uint!("15")).unwrap();

        assert_eq!(state.byte(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("0x0112233445566778899AABBCCDDEEFF0")).unwrap();
        state.stack.push(uint!("32")).unwrap();

        assert_eq!(state.byte(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("0x0112233445566778899AABBCCDDEEFF0")).unwrap();
        state.stack.push(uint!("28")).unwrap();

        assert_eq!(state.byte(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xCD")));

        state.stack.push(uint!("0x0112233445566778899AABBCCDDEEFF0")).unwrap();
        state.stack.push(uint!("19")).unwrap();

        assert_eq!(state.byte(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0x34")));

        state.stack.push(uint!("1")).unwrap();
        state.stack.push(uint!("1")).unwrap();

        assert_eq!(state.shl(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("2")));

        state.stack.push(uint!("0xFF00000000000000000000000000000000000000000000000000000000000000")).unwrap();
        state.stack.push(uint!("4")).unwrap();

        assert_eq!(state.shl(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xF000000000000000000000000000000000000000000000000000000000000000")));

        state.stack.push(uint!("2")).unwrap();
        state.stack.push(uint!("1")).unwrap();

        assert_eq!(state.shr(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));

        state.stack.push(uint!("0xFF")).unwrap();
        state.stack.push(uint!("4")).unwrap();

        assert_eq!(state.shr(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0x0F")));

        state.stack.push(uint!("2")).unwrap();
        state.stack.push(uint!("1")).unwrap();

        assert_eq!(state.sar(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));

        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")).unwrap();
        state.stack.push(uint!("4")).unwrap();

        assert_eq!(state.sar(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")));

        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")).unwrap();
        state.stack.push(uint!("600")).unwrap();

        assert_eq!(state.sar(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")));

        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();

        assert_eq!(state.sar(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")));

        state.stack.push(uint!("0x0FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();

        assert_eq!(state.sar(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));

        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")).unwrap();
        state.stack.push(uint!("0")).unwrap();

        assert_eq!(state.sar(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")));
    }

    #[test]
    fn pop() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });

        state.stack.push(uint!("42")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")).unwrap();

        assert_eq!(state.pop(), Ok(TransitionOutput { cost: 2, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("42")));
    }

    #[test]
    fn mload_no_memory_extension() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.memory.store_word(uint!("0"), uint!("0x4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369")).unwrap();
        assert_eq!(state.memory.size(), 32);

        state.stack.push(uint!("0")).unwrap();

        assert_eq!(state.mload(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0x4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369")));
        assert_eq!(state.memory.size(), 32);
        assert_eq!(state.memory.access(0, state.memory.size()), vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69]);
    }

    #[test]
    fn mload_memory_extension() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.memory.store_word(uint!("0"), uint!("0x4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369")).unwrap();
        assert_eq!(state.memory.size(), 32);

        state.stack.push(uint!("2")).unwrap();

        assert_eq!(state.mload(), Ok(TransitionOutput { cost: 6, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A023690000")));
        assert_eq!(state.memory.size(), 64);
        assert_eq!(state.memory.access(0, state.memory.size()), vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn mload_another_memory_extension() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        state.memory.store_word(uint!("0"), uint!("0x4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369")).unwrap();
        assert_eq!(state.memory.size(), 32);

        state.stack.push(uint!("30")).unwrap();

        assert_eq!(state.mload(), Ok(TransitionOutput { cost: 6, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0x2369000000000000000000000000000000000000000000000000000000000000")));
        assert_eq!(state.memory.size(), 64);
        assert_eq!(state.memory.access(0, state.memory.size()), vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn mload_big_memory_extension() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 51 } });
        state.memory.store_word(uint!("0"), uint!("0x4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369")).unwrap();
        assert_eq!(state.memory.size(), 32);

        state.stack.push(uint!("500")).unwrap();

        assert_eq!(state.mload(), Ok(TransitionOutput { cost: 51, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
        assert_eq!(state.memory.size(), 544);
    }

    #[test]
    fn mstore() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        assert_eq!(state.memory.size(), 0);

        state.stack.push(uint!("0xFF")).unwrap();
        state.stack.push(uint!("0")).unwrap();

        assert_eq!(state.mstore(), Ok(TransitionOutput { cost: 6, jump: 1 }));
        assert_eq!(state.memory.size(), 32);
        assert_eq!(state.memory.access(0, state.memory.size()), vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF]);

        state.stack.push(uint!("0xFF")).unwrap();
        state.stack.push(uint!("1")).unwrap();

        assert_eq!(state.mstore(), Ok(TransitionOutput { cost: 6, jump: 1 }));
        assert_eq!(state.memory.size(), 64);
        assert_eq!(state.memory.access(0, state.memory.size()), vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn mstore_empty_memory() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        assert_eq!(state.memory.size(), 0);

        state.stack.push(uint!("0xFF")).unwrap();
        state.stack.push(uint!("3")).unwrap();

        assert_eq!(state.mstore(), Ok(TransitionOutput { cost: 9, jump: 1 }));
        assert_eq!(state.memory.size(), 64);
        assert_eq!(state.memory.access(0, state.memory.size()), vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn mstore_big_memory_extension() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 544 } });
        assert_eq!(state.memory.size(), 0);

        state.stack.push(uint!("0xABFF")).unwrap();
        state.stack.push(uint!("500")).unwrap();

        assert_eq!(state.mstore(), Ok(TransitionOutput { cost: 54, jump: 1 }));
        assert_eq!(state.memory.size(), 544);
    }

    #[test]
    fn mstore8() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        assert_eq!(state.memory.size(), 0);

        state.stack.push(uint!("0xFFAB")).unwrap();
        state.stack.push(uint!("0")).unwrap();

        assert_eq!(state.mstore8(), Ok(TransitionOutput { cost: 6, jump: 1 }));
        assert_eq!(state.memory.size(), 32);
        assert_eq!(state.memory.access(0, state.memory.size()), vec![0xAB, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        state.stack.push(uint!("0xFFAB")).unwrap();
        state.stack.push(uint!("31")).unwrap();

        assert_eq!(state.mstore8(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.memory.size(), 32);
        assert_eq!(state.memory.access(0, state.memory.size()), vec![0xAB, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xAB]);
    }

    #[test]
    fn sload() {
        let mut initial_storage = HashMap::<u256, u256>::new();
        initial_storage.insert(uint!("42"), uint!("0xAB"));
        let mut state = State::new(StateParameters { initial_storage, transaction: Transaction { data: Vec::<u8>::new(), gas: 2200 } });

        state.stack.push(uint!("42")).unwrap();
        assert_eq!(state.sload(), Ok(TransitionOutput { cost: 2100, jump: 1 }));
        state.stack.push(uint!("42")).unwrap();
        assert_eq!(state.sload(), Ok(TransitionOutput { cost: 100, jump: 1 }));

        assert_eq!(state.stack.pop(), Some(uint!("0xAB")));
        assert_eq!(state.stack.pop(), Some(uint!("0xAB")));
    }

    #[test]
    fn sstore() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 22200 } });

        state.stack.push(uint!("0xFFFF")).unwrap();
        state.stack.push(uint!("0")).unwrap();
        assert_eq!(state.sstore(), Ok(TransitionOutput { cost: 22100, jump: 1 })); // clean storage - no previous value - cold slot
        assert_eq!(state.storage.load(uint!("0")), StorageValue { original_value: uint!("0"), value: uint!("0xFFFF"), warm: true });

        state.stack.push(uint!("0xFFFF")).unwrap();
        state.stack.push(uint!("0")).unwrap();
        assert_eq!(state.sstore(), Ok(TransitionOutput { cost: 100, jump: 1 })); // dirty storage - same value - warn slot
        assert_eq!(state.storage.load(uint!("0")), StorageValue { original_value: uint!("0"), value: uint!("0xFFFF"), warm: true });
    }

    #[test]
    fn sstore_with_original_value() {
        let mut initial_storage = HashMap::<u256, u256>::new();
        initial_storage.insert(uint!("1"), uint!("55"));
        let mut state = State::new(StateParameters { initial_storage, transaction: Transaction { data: Vec::<u8>::new(), gas: 5000 } });

        state.stack.push(uint!("10")).unwrap();
        state.stack.push(uint!("1")).unwrap();
        assert_eq!(state.sstore(), Ok(TransitionOutput { cost: 5000, jump: 1 })); // clean storage - different value - cold slot
        assert_eq!(state.storage.load(uint!("1")), StorageValue { original_value: uint!("55"), value: uint!("10"), warm: true });
    }

    #[test]
    fn jump() {
        let transaction = Transaction { data: vec![0_u8, 0_u8, 0x5B, 0_u8], gas: 20 };
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction });
        assert_eq!(state.pc, 0);

        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFF")).unwrap();
        assert_eq!(state.jump(), Err("Invalid jump destination".to_string())); // not a usize

        state.stack.push(uint!("0xFFFF")).unwrap();
        assert_eq!(state.jump(), Err("Invalid jump destination".to_string())); // not in range

        state.stack.push(uint!("1")).unwrap();
        assert_eq!(state.jump(), Err("Invalid jump destination".to_string())); // not a valid destination

        state.stack.push(uint!("2")).unwrap();
        assert_eq!(state.jump(), Ok(TransitionOutput { cost: 8, jump: 0 }));
        assert_eq!(state.pc, 2);
    }

    #[test]
    fn jumpi() {
        let transaction = Transaction { data: vec![0_u8, 0_u8, 0x5B, 0_u8], gas: 20 };
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction });
        assert_eq!(state.pc, 0);

        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("2")).unwrap();
        assert_eq!(state.jumpi(), Ok(TransitionOutput { cost: 10, jump: 1 }));
        assert_eq!(state.pc, 0);

        state.stack.push(uint!("1")).unwrap();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFF")).unwrap();
        assert_eq!(state.jumpi(), Err("Invalid jump destination".to_string())); // not a usize

        state.stack.push(uint!("1")).unwrap();
        state.stack.push(uint!("0xFFFF")).unwrap();
        assert_eq!(state.jumpi(), Err("Invalid jump destination".to_string())); // not in range

        state.stack.push(uint!("1")).unwrap();
        state.stack.push(uint!("1")).unwrap();
        assert_eq!(state.jumpi(), Err("Invalid jump destination".to_string())); // not a valid destination

        state.stack.push(uint!("1")).unwrap();
        state.stack.push(uint!("2")).unwrap();
        assert_eq!(state.jumpi(), Ok(TransitionOutput { cost: 10, jump: 0 }));
        assert_eq!(state.pc, 2);
    }

    #[test]
    fn pc() {
        let transaction = Transaction { data: vec![0_u8, 0_u8, 0x5B, 0_u8], gas: 20 };
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction });

        state.stack.push(uint!("2")).unwrap();
        assert_eq!(state.jump(), Ok(TransitionOutput { cost: 8, jump: 0 }));
        assert_eq!(state.pc, 2);
        assert_eq!(state.pc(), Ok(TransitionOutput { cost: 2, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("2")));
    }

    #[test]
    fn msize() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 20 } });
        assert_eq!(state.memory.size(), 0);

        state.stack.push(uint!("0xFF")).unwrap();
        state.stack.push(uint!("0")).unwrap();
        assert_eq!(state.mstore(), Ok(TransitionOutput { cost: 6, jump: 1 }));
        assert_eq!(state.memory.size(), 32);
        assert_eq!(state.msize(), Ok(TransitionOutput { cost: 2, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("32")));
    }

    #[test]
    fn gas() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 5 } });

        assert_eq!(state.gas(), Ok(TransitionOutput { cost: 2, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("3")));

        assert_eq!(state.gas(), Ok(TransitionOutput { cost: 2, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("1")));

        assert_eq!(state.gas(), Err("Out of gas".to_string()));
        assert_eq!(state.stack.pop(), None);
    }

    #[test]
    fn jumpdest() {
        let mut state = State::new(StateParameters { initial_storage: HashMap::<u256, u256>::new(), transaction: Transaction { data: Vec::<u8>::new(), gas: 1 } });

        assert_eq!(state.jumpdest(), Ok(TransitionOutput { cost: 1, jump: 1 }));
    }
}
