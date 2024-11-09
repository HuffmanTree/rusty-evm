use ethnum::U256;
use crate::stack::Stack;
use crate::transitions::{TransitionFunction, TransitionOutput, ADD, ADDMOD, AND, BYTE, DIV, EQ, EXP, GT, ISZERO, LT, MOD, MUL, MULMOD, NOT, OR, SAR, SDIV, SGT, SHL, SHR, SIGNEXTEND, SLT, SMOD, SUB, XOR};

struct State {
    stack: Stack,
    stop_flag: bool,
}

impl State {
    fn new() -> Self {
        Self {
            stack: Stack::new(),
            stop_flag: false,
        }
    }

    fn stop(&mut self) -> Result<TransitionOutput, ()> {
        self.stop_flag = true;
        Ok(TransitionOutput { cost: 0, jump: 0 })
    }

    fn transition_builder<const I: usize, const O: usize>(&mut self, f: TransitionFunction<I, O>) -> Result<TransitionOutput, String> {
        let mut input = [U256::ZERO; I];
        for i in 0..I {
            input[i] = match self.stack.pop() {
                Some(x) => x,
                _ => return Err("Stack is empty".to_string()),
            }
        };
        let output = match f(input) {
            Ok(y) => y,
            Err(s) => return Err(s),
        };
        for o in 0..O {
            if let Err(e) = self.stack.push(output.result[o]) {
                return Err(e.to_string());
            }
        }
        Ok(TransitionOutput { cost: output.cost, jump: output.jump })
    }

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
}

#[cfg(test)]
mod tests {
    use crate::transitions::TransitionFunctionOutput;

    use super::*;
    use ethnum::{uint,u256};

    #[test]
    fn transition_builder_fails_if_not_enough_parmeters_in_stack() {
        let mut state = State::new();

        assert_eq!(state.transition_builder(
            |input: [u256; 1]| Result::<TransitionFunctionOutput<1>, String>::Ok(TransitionFunctionOutput { cost: 3, result: [input[0]], jump: 1 })
        ), Err("Stack is empty".to_string()));
    }

    #[test]
    fn transition_builder_fails_if_transition_function_fails() {
        let mut state = State::new();

        assert_eq!(state.transition_builder(
            |_input: [u256; 0]| Result::<TransitionFunctionOutput<0>, String>::Err("Fail".to_string())
        ), Err("Fail".to_string()));
    }

    #[test]
    fn transition_builder_fails_if_too_much_outputs() {
        let mut state = State::new();

        assert_eq!(state.transition_builder(
            |_input: [u256; 0]| Result::<TransitionFunctionOutput<1025>, String>::Ok(TransitionFunctionOutput { cost: 3, result: [U256::ZERO; 1025], jump: 1 })
        ), Err("Stack overflow".to_string()));
    }

    #[test]
    fn set_the_stop_flag_to_true() {
        let mut state = State::new();

        assert_eq!(state.stop(), Ok(TransitionOutput { cost: 0, jump: 0 }));
        assert!(state.stop_flag);
    }

    #[test]
    fn adds_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new();
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
        let mut state = State::new();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();
        state.stack.push(uint!("1")).unwrap();

        assert_eq!(state.add(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn fails_to_add_if_not_enough_items() {
        let mut state = State::new();

        assert_eq!(state.add(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn multiplies_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new();
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
        let mut state = State::new();
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();
        state.stack.push(uint!("2")).unwrap();

        assert_eq!(state.mul(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")));
    }

    #[test]
    fn fails_to_multiply_if_not_enough_items() {
        let mut state = State::new();

        assert_eq!(state.mul(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn subtracts_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new();
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
        let mut state = State::new();
        state.stack.push(uint!("1")).unwrap();
        state.stack.push(uint!("0")).unwrap();

        assert_eq!(state.sub(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")));
    }

    #[test]
    fn fails_to_subtract_if_not_enough_items() {
        let mut state = State::new();

        assert_eq!(state.sub(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn divides_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new();
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
        let mut state = State::new();
        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("6")).unwrap();

        assert_eq!(state.div(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn fails_to_divide_if_not_enough_items() {
        let mut state = State::new();

        assert_eq!(state.div(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn sign_divides_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new();
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
        let mut state = State::new();
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
        let mut state = State::new();
        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("6")).unwrap();

        assert_eq!(state.sdiv(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn fails_to_sign_divide_if_not_enough_items() {
        let mut state = State::new();

        assert_eq!(state.sdiv(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn takes_the_reminder_of_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new();
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
        let mut state = State::new();
        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("6")).unwrap();

        assert_eq!(state.r#mod(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn fails_to_take_the_reminder_if_not_enough_items() {
        let mut state = State::new();

        assert_eq!(state.r#mod(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn sign_rems_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new();
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
        let mut state = State::new();
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
        let mut state = State::new();
        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("6")).unwrap();

        assert_eq!(state.sdiv(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn fails_to_sign_rem_if_not_enough_items() {
        let mut state = State::new();

        assert_eq!(state.smod(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn adds_modulo() {
        let mut state = State::new();
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
        let mut state = State::new();
        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("6")).unwrap();
        state.stack.push(uint!("4")).unwrap();

        assert_eq!(state.addmod(), Ok(TransitionOutput { cost: 8, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn fails_to_add_modulo_if_not_enough_items() {
        let mut state = State::new();

        assert_eq!(state.addmod(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn multiplies_modulo() {
        let mut state = State::new();
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
        let mut state = State::new();
        state.stack.push(uint!("0")).unwrap();
        state.stack.push(uint!("6")).unwrap();
        state.stack.push(uint!("4")).unwrap();

        assert_eq!(state.mulmod(), Ok(TransitionOutput { cost: 8, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn fails_to_multiply_modulo_if_not_enough_items() {
        let mut state = State::new();

        assert_eq!(state.mulmod(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn exponentiates() {
        let mut state = State::new();
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
        let mut state = State::new();

        assert_eq!(state.exp(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn sign_extends() {
        let mut state = State::new();
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
    }

    #[test]
    fn fails_to_sign_extend_if_not_enough_items() {
        let mut state = State::new();

        assert_eq!(state.signextend(), Err("Stack is empty".to_string()));
    }

    #[test]
    fn compare_values() {
        let mut state = State::new();
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
        let mut state = State::new();
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
        let mut state = State::new();
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
}
