#![allow(dead_code)]

use ethnum::{u256, U256};
use std::cmp::min;
use crate::{errors::Error, stack::Stack, storage::Storage, transaction::{Account, Address}, utils::{NeededSizeInBytes, WrappingBigPow, WrappingSignedDiv, WrappingSignedRem}};

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

    fn mul(_s: &mut WorldState, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(a.wrapping_mul(b))?;
        Ok(InstructionOutput { cost: 5 })
    }

    fn sub(_s: &mut WorldState, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(a.wrapping_sub(b))?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn div(_s: &mut WorldState, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if b == 0 { U256::ZERO } else { a.wrapping_div(b) })?;
        Ok(InstructionOutput { cost: 5 })
    }

    fn sdiv(_s: &mut WorldState, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if b == 0 { U256::ZERO } else { a.wrapping_signed_div(b) })?;
        Ok(InstructionOutput { cost: 5 })
    }

    fn r#mod(_s: &mut WorldState, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if b == 0 { U256::ZERO } else { a.wrapping_rem(b) })?;
        Ok(InstructionOutput { cost: 5 })
    }

    fn smod(_s: &mut WorldState, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if b == 0 { U256::ZERO } else { a.wrapping_signed_rem(b) })?;
        Ok(InstructionOutput { cost: 5 })
    }

    fn addmod(_s: &mut WorldState, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b, n) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_add(b.wrapping_rem(n)).wrapping_rem(n) })?;
        Ok(InstructionOutput { cost: 8 })
    }

    fn mulmod(_s: &mut WorldState, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b, n) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_mul(b.wrapping_rem(n)).wrapping_rem(n) })?;
        Ok(InstructionOutput { cost: 8 })
    }

    fn exp(_s: &mut WorldState, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, e) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        let exponent_byte_size = e.needed_size_in_bytes();
        ctx.stack.push(a.wrapping_big_pow(e))?;
        Ok(InstructionOutput { cost: 10 + 50 * exponent_byte_size })
    }

    fn signextend(_s: &mut WorldState, ctx: &mut ExecutionContext) -> InstructionResult {
        let (b, x) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        let b: u32 = min(b, u256::from(30u32)).try_into().unwrap();
        let mask = U256::ONE.wrapping_shl((b + 1).wrapping_shl(3));
        let sign_mask = mask.wrapping_shr(1);
        let size_mask = mask - 1;
        let value = x & size_mask;
        ctx.stack.push(if (value & sign_mask) != 0 { !size_mask | value } else { value })?;
        Ok(InstructionOutput { cost: 5 })
    }
}

#[cfg(test)]
mod tests {
    use ethnum::{uint, U256};
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

    #[test]
    fn mul() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::mul(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 60);

        context.with_stack(vec![U256::MAX, uint!("2")]);
        assert_eq!(Machine::mul(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), U256::MAX - 1);
    }

    #[test]
    fn sub() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::sub(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 4);

        context.with_stack(vec![0u8, 1]);
        assert_eq!(Machine::sub(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), U256::MAX);
    }

    
    #[test]
    fn div() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::div(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![6u8, 0]);
        assert_eq!(Machine::div(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0); // dividing by zero returns zero by convention
    }

    #[test]
    fn sdiv() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::sdiv(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![U256::MAX - 1, U256::MAX]);
        assert_eq!(Machine::sdiv(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 2);

        context.with_stack(vec![U256::MAX - 1, U256::ONE]);
        assert_eq!(Machine::sdiv(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), U256::MAX - 1);

        context.with_stack(vec![6u8, 0]);
        assert_eq!(Machine::sdiv(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0); // dividing by zero returns zero by convention
    }

    #[test]
    fn r#mod() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::r#mod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 4);

        context.with_stack(vec![10u8, 3]);
        assert_eq!(Machine::r#mod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![6u8, 0]);
        assert_eq!(Machine::r#mod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0); // modulo zero returns zero by convention
    }

    #[test]
    fn smod() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::smod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 4);

        context.with_stack(vec![3u8, 2]);
        assert_eq!(Machine::smod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![U256::MAX - 7, U256::MAX - 2]);
        assert_eq!(Machine::smod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), U256::MAX - 1);

        context.with_stack(vec![U256::MAX - 2, uint!("2")]);
        assert_eq!(Machine::smod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), U256::MAX);

        context.with_stack(vec![uint!("3"), U256::MAX - 1]);
        assert_eq!(Machine::smod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![6u8, 0]);
        assert_eq!(Machine::smod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0); // modulo zero returns zero by convention
    }
   
    #[test]
    fn addmod() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 10, 8]);
        assert_eq!(Machine::addmod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 4);

        context.with_stack(vec![U256::MAX, uint!("2"), uint!("2")]);
        assert_eq!(Machine::addmod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![U256::MAX - 2, uint!("2"), uint!("3")]);
        assert_eq!(Machine::addmod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![U256::MAX, uint!("1"), uint!("10")]);
        assert_eq!(Machine::addmod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 6);

        context.with_stack(vec![4u8, 6, 0]);
        assert_eq!(Machine::addmod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0); // modulo zero returns zero by convention
    }
    
    #[test]
    fn mulmod() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 10, 8]);
        assert_eq!(Machine::mulmod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 4);

        context.with_stack(vec![U256::MAX, U256::MAX, uint!("12")]);
        assert_eq!(Machine::mulmod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 9);

        context.with_stack(vec![U256::MAX - 2, uint!("2"), uint!("3")]);
        assert_eq!(Machine::mulmod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 2);

        context.with_stack(vec![4u8, 6, 0]);
        assert_eq!(Machine::mulmod(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);
    }


    #[test]
    fn exp() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 2]);
        assert_eq!(Machine::exp(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 60 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 100);

        context.with_stack(vec![2u8, 2]);
        assert_eq!(Machine::exp(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 60 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 4);

        context.with_stack(vec![5u8, 0]);
        assert_eq!(Machine::exp(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 10 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![2u8, 10]);
        assert_eq!(Machine::exp(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 60 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1024);

        context.with_stack(vec![2u16, 260]);
        assert_eq!(Machine::exp(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 110 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![uint!("0xFFFFFFFFFFFFFFFF"), uint!("3")]);
        assert_eq!(Machine::exp(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 60 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0xFFFFFFFFFFFFFFFD0000000000000002FFFFFFFFFFFFFFFF"));

        context.with_stack(vec![uint!("3"), uint!("0xFFFFFFFFFFFFFFF0")]);
        assert_eq!(Machine::exp(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 410 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0xE9377A20E36295B65EA7F55D4A333F73CF25A1BE32FEBCF9702BDE500F57B8C1"));

        context.with_stack(vec![uint!("5"), uint!("0xFFFFFFFFFFFFFFF0FFFFFF")]);
        assert_eq!(Machine::exp(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 560 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0x49E63006C06484CE7E18DB842AD1771FC1C83AA03B09227A2EB3765958CCCCCD"));
    }

    #[test]
    fn signextend() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![0u8, 0x41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0x41);

        context.with_stack(vec![0u16, 0xEF41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0x41);

        context.with_stack(vec![1u16, 0xEF41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEF41"));

        context.with_stack(vec![2u16, 0xEF41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xEF41);

        context.with_stack(vec![30u16, 0xEF41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xEF41);

        context.with_stack(vec![31u16, 0xEF41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xEF41);

        context.with_stack(vec![uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0xEF41")]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xEF41);
    }
}
