#![allow(dead_code)]

use ethnum::u256;
use crate::{errors::Error, stack::Stack, storage::Storage, transaction::{Account, Address}};

#[derive(Default)]
pub struct WorldState {
    pub accounts: Storage<Address, Account>,
    pub storage: Storage<u256, u256>,
}

#[derive(Default)]
pub struct ExecutionContext {
    pub pc: usize,
    pub stack: Stack,
    pub stop: bool,
}

#[derive(Debug, Eq, PartialEq)]
struct InstructionOutput {
    cost: usize,
}

type InstructionResult = Result<InstructionOutput, Error>;

struct Machine {}

impl Machine {
    fn pop_or_fail(ctx: &mut ExecutionContext) -> Result<u256, Error> {
        if let Some(x) = ctx.stack.pop() { Ok(x) } else { Err(Error::EmptyStack) }
    }

    fn stop(_s: &mut WorldState, ctx: &mut ExecutionContext) -> InstructionResult {
        ctx.stop = true;
        Ok(InstructionOutput { cost: 0 })
    }

    fn add(_s: &mut WorldState, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(a.wrapping_add(b))?;
        Ok(InstructionOutput { cost: 3 })
    }
}

#[cfg(test)]
mod tests {
    use ethnum::U256;
    use super::*;

    impl ExecutionContext {
        fn with_stop(&mut self, stop: bool) {
            self.stop = stop;
        }

        fn with_stack<T: Into::<u256> + Copy>(&mut self, stack: Vec<T>) {
            self.stack = Stack::new();
            for i in (0..stack.len()).rev() { self.stack.push(stack[i].into()).unwrap(); }
        }
    }

    #[test]
    fn stop() {
        let context = &mut ExecutionContext::default();

        context.with_stop(false);
        assert_eq!(Machine::stop(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 0 }));
        assert!(context.stop);
    }

    #[test]
    fn add() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::add(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 16);

        context.with_stack(vec![U256::MAX, U256::ONE]);
        assert_eq!(Machine::add(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);
    }
}
