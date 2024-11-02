use crate::stack::Stack;

struct State {
    gas: u32,
    pc: usize,
    stack: Stack,
    stop_flag: bool,
}

struct StateParameters {
    gas: u32,
}

#[derive(Debug,PartialEq,Eq)]
struct TransitionOutput {
    cost: u32,
    jump: usize,
}

impl State {
    fn new(parameters: StateParameters) -> Self {
        Self {
            gas: parameters.gas,
            pc: 0,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethnum::uint;

    #[test]
    fn set_the_stop_flag_to_true() {
        let mut state = State::new(StateParameters { gas: 8 });

        assert_eq!(state.stop(), Ok(TransitionOutput { cost: 0, jump: 0 }));
        assert!(state.stop_flag);
    }

    #[test]
    fn adds_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new(StateParameters { gas: 8 });
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
        let mut state = State::new(StateParameters { gas: 8 });
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();
        state.stack.push(uint!("1")).unwrap();

        assert_eq!(state.add(), Ok(TransitionOutput { cost: 3, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0")));
    }

    #[test]
    fn fails_to_add_if_not_enough_items() {
        let mut state = State::new(StateParameters { gas: 8 });

        assert_eq!(state.add(), Err("Stack is empty"));
    }

    #[test]
    fn multiplies_the_two_numbers_on_top_of_the_stack() {
        let mut state = State::new(StateParameters { gas: 8 });
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
        let mut state = State::new(StateParameters { gas: 8 });
        state.stack.push(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")).unwrap();
        state.stack.push(uint!("2")).unwrap();

        assert_eq!(state.mul(), Ok(TransitionOutput { cost: 5, jump: 1 }));
        assert_eq!(state.stack.pop(), Some(uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")));
    }

    #[test]
    fn fails_to_multiply_if_not_enough_items() {
        let mut state = State::new(StateParameters { gas: 8 });

        assert_eq!(state.mul(), Err("Stack is empty"));
    }
}
