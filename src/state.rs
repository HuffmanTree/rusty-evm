use ethnum::u256;
use crate::stack::Stack;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethnum::uint;

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
}
