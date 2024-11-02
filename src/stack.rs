use ethnum::u256;

pub struct Stack {
    size: u16,
    top: i32,
    arr: Vec<u256>,
}

impl Stack {
    pub fn new(s: Option<u16>) -> Self {
        Self { size: s.unwrap_or(1024), top: -1, arr: Vec::<u256>::new() }
    }

    pub fn pop(&mut self) -> Option<u256> {
        match self.arr.pop() {
            Option::None => None,
            Option::Some(elt) => { self.top -= 1; Some(elt) },
        }
    }

    pub fn push(&mut self, value: u256) -> Result<(), &str> {
        match self.top + 1 == self.size.into() {
            true => Err("Stack overflow"),
            false => { self.top += 1; self.arr.push(value); Ok(()) },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethnum::uint;

    #[test]
    fn fail_to_pop_from_an_empty_stack() {
        let mut stack = Stack::new(Option::None);

        assert_eq!(stack.pop(), None);
    }

    #[test]
    fn pops_from_the_stack() {
        let mut stack = Stack { size: 1, top: 0, arr: vec![uint!("7")] };

        assert_eq!(stack.pop(), Some(uint!("7")));
        assert_eq!(stack.size, 1);
        assert_eq!(stack.top, -1);
        assert_eq!(stack.arr.len(), 0);
    }

    #[test]
    fn fail_to_push_to_an_already_full_stack() {
        let mut stack = Stack::new(Option::Some(0));

        assert_eq!(stack.push(uint!("7")), Err("Stack overflow"));
    }

    #[test]
    fn pushes_to_the_stack() {
        let mut stack = Stack { size: 1, top: -1, arr: vec!() };

        assert_eq!(stack.push(uint!("7")), Ok(()));
        assert_eq!(stack.size, 1);
        assert_eq!(stack.top, 0);
        assert_eq!(stack.arr, vec!(uint!("7")));
    }
}
