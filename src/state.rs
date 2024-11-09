use ethnum::{u256, U256};
use crate::stack::Stack;

trait IsNeg {
    fn is_neg(&self) -> bool;
}

trait Abs {
    fn abs(&self) -> Self;
}

trait WrappingSignedDiv {
    fn wrapping_signed_div(&self, rhs: Self) -> Self;
}

trait WrappingSignedRem {
    fn wrapping_signed_rem(&self, rhs: Self) -> Self;
}

trait NeededSizeInBytes {
    fn needed_size_in_bytes(self) -> u32;
}

impl IsNeg for u256 {
    fn is_neg(&self) -> bool {
        (self & u256::from_str_hex("0x8000000000000000000000000000000000000000000000000000000000000000").unwrap()) != 0
    }
}

impl Abs for u256 {
    fn abs(&self) -> Self {
        if self.is_neg() { self.wrapping_neg() } else { self.clone() }
    }
}

impl NeededSizeInBytes for u32 {
    fn needed_size_in_bytes(mut self) -> u32 {
        let mut n = 0_u32;
        while self != 0 {
            self >>= 8;
            n += 1;
        }
        n
    }
}

impl WrappingSignedDiv for u256 {
    fn wrapping_signed_div(&self, rhs: Self) -> Self {
        let negate = self.is_neg() ^ rhs.is_neg();
        let res = self.abs().wrapping_div(rhs.abs());
        if negate { res.wrapping_neg() } else { res }
    }
}

impl WrappingSignedRem for u256 {
    fn wrapping_signed_rem(&self, rhs: Self) -> Self {
        let negate = self.is_neg();
        let res = self.abs().wrapping_rem(rhs.abs());
        if negate { res.wrapping_neg() } else { res }
    }
}

struct State {
    stack: Stack,
    stop_flag: bool,
}

struct TransitionFunctionOutput<const O: usize> {
    cost: u32,
    result: [u256; O],
    jump: usize,
}

#[derive(Debug,PartialEq,Eq)]
struct TransitionOutput {
    cost: u32,
    jump: usize,
}

impl State {
    fn new() -> Self {
        Self {
            stack: Stack::new(Option::None),
            stop_flag: false,
        }
    }

    fn stop(&mut self) -> Result<TransitionOutput, ()> {
        self.stop_flag = true;
        Ok(TransitionOutput { cost: 0, jump: 0 })
    }

    fn transition_builder<F, const I: usize, const O: usize>(&mut self, f: F) -> Result<TransitionOutput, String> where F: Fn([u256; I]) -> Result<TransitionFunctionOutput<O>, String> {
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

    fn add(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 3, result: [a.wrapping_add(b)], jump: 1 }))
    }

    fn mul(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 5, result: [a.wrapping_mul(b)], jump: 1 }))
    }

    fn sub(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 3, result: [a.wrapping_sub(b)], jump: 1 }))
    }

    fn div(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_div(b) }], jump: 1 }))
    }

    fn sdiv(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_signed_div(b) }], jump: 1 }))
    }

    fn r#mod(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_rem(b) }], jump: 1 }))
    }

    fn smod(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_signed_rem(b) }], jump: 1 }))
    }

    fn addmod(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b, n]: [u256; 3]| Ok(TransitionFunctionOutput { cost: 8, result: [if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_add(b.wrapping_rem(n)).wrapping_rem(n) }], jump: 1 }))
    }

    fn mulmod(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b, n]: [u256; 3]| Ok(TransitionFunctionOutput { cost: 8, result: [if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_mul(b.wrapping_rem(n)).wrapping_rem(n) }], jump: 1 }))
    }

    fn exp(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, e]: [u256; 2]| match TryInto::<u32>::try_into(e) {
            Ok(e) => Ok(TransitionFunctionOutput { cost: 10 + 50 * e.needed_size_in_bytes(), result: [a.wrapping_pow(e)], jump: 1 }),
            _ => Err("Exponent too large".to_string()),
        })
    }

    fn signextend(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[b, x]: [u256; 2]| match TryInto::<u32>::try_into(b) {
            Ok(b) => {
                let mask = U256::ONE.wrapping_shl((b + 1).wrapping_shl(3));
                let sign_mask = mask.wrapping_shr(1);
                let size_mask = mask - 1;
                let value = x & size_mask;

                Ok(TransitionFunctionOutput { cost: 5, result: [if (value & sign_mask) != 0 { !size_mask | value } else { value }], jump: 1 })
            },
            _ => Err("Size too large".to_string()),
        })
    }

    fn lt(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 3, result: [if a < b { U256::ONE } else { U256::ZERO }], jump: 1 }))
    }

    fn gt(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 3, result: [if a > b { U256::ONE } else { U256::ZERO }], jump: 1 }))
    }

    fn slt(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 3, result: [match (a.is_neg(), b.is_neg()) {
            (true, false) => { U256::ONE },
            (false, true) => { U256::ZERO },
            _ => if a < b { U256::ONE } else { U256::ZERO },
        }], jump: 1 }))
    }

    fn sgt(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 3, result: [match (a.is_neg(), b.is_neg()) {
            (true, false) => { U256::ZERO },
            (false, true) => { U256::ONE },
            _ => if a > b { U256::ONE } else { U256::ZERO },
        }], jump: 1 }))
    }

    fn eq(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 3, result: [if a == b { U256::ONE } else { U256::ZERO }], jump: 1 }))
    }

    fn iszero(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a]: [u256; 1]| Ok(TransitionFunctionOutput { cost: 3, result: [if a == U256::ZERO { U256::ONE } else { U256::ZERO }], jump: 1 }))
    }

    fn and(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 3, result: [a & b], jump: 1 }))
    }

    fn or(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 3, result: [a | b], jump: 1 }))
    }

    fn xor(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a, b]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 3, result: [a ^ b], jump: 1 }))
    }

    fn not(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[a]: [u256; 1]| Ok(TransitionFunctionOutput { cost: 3, result: [!a], jump: 1 }))
    }

    fn byte(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[i, x]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 3, result: [if i > 31 { U256::ZERO } else { (x >> (8 * (31 - i))) & 0xFF }], jump: 1 }))
    }

    fn shl(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[shift, value]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 3, result: [match TryInto::<u8>::try_into(shift) {
            Ok(shift) => value.wrapping_shl(shift.into()),
            _ => U256::ZERO,
        }], jump: 1 }))
    }

    fn shr(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[shift, value]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 3, result: [match TryInto::<u8>::try_into(shift) {
            Ok(shift) => value.wrapping_shr(shift.into()),
            _ => U256::ZERO,
        }], jump: 1 }))
    }

    fn sar(&mut self) -> Result<TransitionOutput, String> {
        self.transition_builder(|[shift, value]: [u256; 2]| Ok(TransitionFunctionOutput { cost: 3, result: [match (TryInto::<u8>::try_into(shift), value.is_neg()) {
            (Ok(shift), false) => value.wrapping_shr(shift.into()),
            (Ok(shift), true) => { if shift == 0 { value } else { !(U256::ONE.wrapping_shl((255 - shift + 1).into()) - 1) | value.wrapping_shr(shift.into()) } },
            (Err(_), false) => U256::ZERO,
            (Err(_), true) => U256::MAX,
        }], jump: 1 }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethnum::uint;

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
    fn u32_needed_size_in_bytes() {
        assert_eq!(0_u32.needed_size_in_bytes(), 0);
        assert_eq!(1_u32.needed_size_in_bytes(), 1);
        assert_eq!(2_u32.needed_size_in_bytes(), 1);

        assert_eq!(126_u32.needed_size_in_bytes(), 1);
        assert_eq!(127_u32.needed_size_in_bytes(), 1);
        assert_eq!(128_u32.needed_size_in_bytes(), 1);

        assert_eq!(254_u32.needed_size_in_bytes(), 1);
        assert_eq!(255_u32.needed_size_in_bytes(), 1);
        assert_eq!(256_u32.needed_size_in_bytes(), 2);
        assert_eq!(257_u32.needed_size_in_bytes(), 2);
    }

    #[test]
    fn u256_is_neg() {
        assert!(!uint!("6").is_neg());
        assert!(!uint!("10").is_neg());

        assert!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE").is_neg());
        assert!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").is_neg());
    }

    #[test]
    fn u256_abs() {
        assert_eq!(uint!("6").abs(), uint!("6"));
        assert_eq!(uint!("10").abs(), uint!("10"));

        assert_eq!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE").abs(), uint!("2"));
        assert_eq!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").abs(), uint!("1"));
    }

    #[test]
    #[should_panic(expected = "attempt to divide by zero")]
    fn u256_wrapping_signed_div() {
        assert_eq!(uint!("4").wrapping_signed_div(uint!("2")), uint!("2"));
        assert_eq!(uint!("4").wrapping_signed_div(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE"));
        assert_eq!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC").wrapping_signed_div(uint!("2")), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE"));
        assert_eq!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC").wrapping_signed_div(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")), uint!("2"));
        uint!("4").wrapping_signed_div(uint!("0"));
    }

    #[test]
    #[should_panic(expected = "attempt to calculate the remainder with a divisor of zero")]
    fn u256_wrapping_signed_rem() {
        assert_eq!(uint!("5").wrapping_signed_rem(uint!("2")), uint!("1"));
        assert_eq!(uint!("5").wrapping_signed_rem(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")), uint!("1"));
        assert_eq!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFB").wrapping_signed_rem(uint!("2")), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"));
        assert_eq!(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFB").wrapping_signed_rem(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"));
        uint!("5").wrapping_signed_rem(uint!("0"));
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
