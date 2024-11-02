use ethnum::u256;
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

    fn stop(&mut self) -> Result<TransitionOutput, &str> {
        self.stop_flag = true;
        Ok(TransitionOutput { cost: 0, jump: 0 })
    }

    fn add(&mut self) -> Result<TransitionOutput, &str> {
        let (a, b) = match (self.stack.pop(), self.stack.pop()) {
            (Some(x), Some(y)) => (x, y),
            _ => return Err("Stack is empty"),
        };
        match self.stack.push(a.wrapping_add(b)) {
            Ok(_) => Ok(TransitionOutput { cost: 3, jump: 1 }),
            Err(s) => Err(s),
        }
    }

    fn mul(&mut self) -> Result<TransitionOutput, &str> {
        let (a, b) = match (self.stack.pop(), self.stack.pop()) {
            (Some(x), Some(y)) => (x, y),
            _ => return Err("Stack is empty"),
        };
        match self.stack.push(a.wrapping_mul(b)) {
            Ok(_) => Ok(TransitionOutput { cost: 5, jump: 1 }),
            Err(s) => Err(s),
        }
    }

    fn sub(&mut self) -> Result<TransitionOutput, &str> {
        let (a, b) = match (self.stack.pop(), self.stack.pop()) {
            (Some(x), Some(y)) => (x, y),
            _ => return Err("Stack is empty"),
        };
        match self.stack.push(a.wrapping_sub(b)) {
            Ok(_) => Ok(TransitionOutput { cost: 3, jump: 1 }),
            Err(s) => Err(s),
        }
    }

    fn div(&mut self) -> Result<TransitionOutput, &str> {
        let (a, b) = match (self.stack.pop(), self.stack.pop()) {
            (Some(x), Some(y)) => (x, y),
            _ => return Err("Stack is empty"),
        };
        match self.stack.push(if b == 0 { u256::from(0_u8) } else { a.wrapping_div(b) }) {
            Ok(_) => Ok(TransitionOutput { cost: 5, jump: 1 }),
            Err(s) => Err(s),
        }
    }

    fn sdiv(&mut self) -> Result<TransitionOutput, &str> {
        let (a, b) = match (self.stack.pop(), self.stack.pop()) {
            (Some(x), Some(y)) => (x, y),
            _ => return Err("Stack is empty"),
        };
        match self.stack.push(if b == 0 { u256::from(0_u8) } else { a.wrapping_signed_div(b) }) {
            Ok(_) => Ok(TransitionOutput { cost: 5, jump: 1 }),
            Err(s) => Err(s),
        }
    }

    fn r#mod(&mut self) -> Result<TransitionOutput, &str> {
        let (a, b) = match (self.stack.pop(), self.stack.pop()) {
            (Some(x), Some(y)) => (x, y),
            _ => return Err("Stack is empty"),
        };
        match self.stack.push(if b == 0 { u256::from(0_u8) } else { a.wrapping_rem(b) }) {
            Ok(_) => Ok(TransitionOutput { cost: 5, jump: 1 }),
            Err(s) => Err(s),
        }
    }

    fn smod(&mut self) -> Result<TransitionOutput, &str> {
        let (a, b) = match (self.stack.pop(), self.stack.pop()) {
            (Some(x), Some(y)) => (x, y),
            _ => return Err("Stack is empty"),
        };
        match self.stack.push(if b == 0 { u256::from(0_u8) } else { a.wrapping_signed_rem(b) }) {
            Ok(_) => Ok(TransitionOutput { cost: 5, jump: 1 }),
            Err(s) => Err(s),
        }
    }

    fn addmod(&mut self) -> Result<TransitionOutput, &str> {
        todo!();
    }

    fn mulmod(&mut self) -> Result<TransitionOutput, &str> {
        todo!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethnum::uint;

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

        assert_eq!(state.add(), Err("Stack is empty"));
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

        assert_eq!(state.mul(), Err("Stack is empty"));
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

        assert_eq!(state.sub(), Err("Stack is empty"));
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

        assert_eq!(state.div(), Err("Stack is empty"));
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

        assert_eq!(state.sdiv(), Err("Stack is empty"));
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

        assert_eq!(state.r#mod(), Err("Stack is empty"));
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

        assert_eq!(state.sdiv(), Err("Stack is empty"));
    }
}
