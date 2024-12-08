#![allow(dead_code)]

use ethnum::{u256, U256};
use std::cmp::min;
use crate::{errors::Error, memory::{Memory, ReadWriteOperation}, stack::Stack, storage::Storage, transaction::{Account, Address, Transaction}, utils::{Hash, IsNeg, NeededSizeInBytes, WrappingBigPow, WrappingSignedDiv, WrappingSignedRem}};

#[derive(Default)]
pub struct WorldState {
    pub accounts: Storage<Address, Account>,
    pub storage: Storage<u256, u256>,
}

#[derive(Default)]
pub struct ExecutionContextContract {
    pub address: Address,
    pub caller: Address,
}

#[derive(Default)]
pub struct ExecutionContext {
    pub contract: ExecutionContextContract,
    pub memory: Memory,
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

    fn stop(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        ctx.stop = true;
        Ok(InstructionOutput { cost: 0 })
    }

    fn add(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(a.wrapping_add(b))?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn mul(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(a.wrapping_mul(b))?;
        Ok(InstructionOutput { cost: 5 })
    }

    fn sub(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(a.wrapping_sub(b))?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn div(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if b == 0 { U256::ZERO } else { a.wrapping_div(b) })?;
        Ok(InstructionOutput { cost: 5 })
    }

    fn sdiv(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if b == 0 { U256::ZERO } else { a.wrapping_signed_div(b) })?;
        Ok(InstructionOutput { cost: 5 })
    }

    fn r#mod(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if b == 0 { U256::ZERO } else { a.wrapping_rem(b) })?;
        Ok(InstructionOutput { cost: 5 })
    }

    fn smod(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if b == 0 { U256::ZERO } else { a.wrapping_signed_rem(b) })?;
        Ok(InstructionOutput { cost: 5 })
    }

    fn addmod(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b, n) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_add(b.wrapping_rem(n)).wrapping_rem(n) })?;
        Ok(InstructionOutput { cost: 8 })
    }

    fn mulmod(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b, n) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_mul(b.wrapping_rem(n)).wrapping_rem(n) })?;
        Ok(InstructionOutput { cost: 8 })
    }

    fn exp(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, e) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        let exponent_byte_size = e.needed_size_in_bytes();
        ctx.stack.push(a.wrapping_big_pow(e))?;
        Ok(InstructionOutput { cost: 10 + 50 * exponent_byte_size })
    }

    fn signextend(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (b, x) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        let b: u32 = min(b, u256::from(30u32)).try_into().unwrap();
        let mask = U256::ONE.wrapping_shl((b + 1).wrapping_shl(3));
        let sign_mask = mask.wrapping_shr(1);
        let size_mask = mask - 1;
        let value = x & size_mask;
        ctx.stack.push(if (value & sign_mask) != 0 { !size_mask | value } else { value })?;
        Ok(InstructionOutput { cost: 5 })
    }

    fn lt(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if a < b { U256::ONE } else { U256::ZERO })?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn gt(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if a > b { U256::ONE } else { U256::ZERO })?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn slt(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(match (a.is_neg(), b.is_neg()) {
            (true, false) => { U256::ONE },
            (false, true) => { U256::ZERO },
            _ => if a < b { U256::ONE } else { U256::ZERO },
        })?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn sgt(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(match (a.is_neg(), b.is_neg()) {
            (true, false) => { U256::ZERO },
            (false, true) => { U256::ONE },
            _ => if a > b { U256::ONE } else { U256::ZERO },
        })?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn eq(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if a == b { U256::ONE } else { U256::ZERO })?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn iszero(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let a = Machine::pop_or_fail(ctx)?;
        ctx.stack.push(if a == U256::ZERO { U256::ONE } else { U256::ZERO })?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn and(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(a & b)?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn or(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(a | b)?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn xor(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (a, b) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(a ^ b)?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn not(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let a = Machine::pop_or_fail(ctx)?;
        ctx.stack.push(!a)?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn byte(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (i, x) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(if i > 31 { U256::ZERO } else { (x >> (8 * (31 - i))) & 0xFF })?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn shl(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (shift, value) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(match TryInto::<u8>::try_into(shift) {
            Ok(shift) => value.wrapping_shl(shift.into()),
            _ => U256::ZERO,
        })?;
        Ok(InstructionOutput { cost: 3 })
   }

    fn shr(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (shift, value) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(match TryInto::<u8>::try_into(shift) {
            Ok(shift) => value.wrapping_shr(shift.into()),
            _ => U256::ZERO,
        })?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn sar(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (shift, value) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        ctx.stack.push(match (TryInto::<u8>::try_into(shift), value.is_neg()) {
            (Ok(shift), false) => value.wrapping_shr(shift.into()),
            (Ok(shift), true) => { if shift == 0 { value } else { !(U256::ONE.wrapping_shl((255 - shift + 1).into()) - 1) | value.wrapping_shr(shift.into()) } },
            (Err(_), false) => U256::ZERO,
            (Err(_), true) => U256::MAX,
        })?;
        Ok(InstructionOutput { cost: 3 })
    }

    fn keccak256(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let (offset, size) = (Machine::pop_or_fail(ctx)?, Machine::pop_or_fail(ctx)?);
        let ReadWriteOperation { size, extension_cost, result, .. } = ctx.memory.load(offset, size)?;
        ctx.stack.push(result.keccak256())?;
        Ok(InstructionOutput { cost: 30 + 6 * ((size + 31) >> 5) + extension_cost })
    }

    fn address(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        ctx.stack.push(ctx.contract.address.0)?;
        Ok(InstructionOutput { cost: 2 })
    }

    fn balance(s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        let address = Machine::pop_or_fail(ctx)?;
        let account = s.accounts.load(address.try_into()?);
        ctx.stack.push(account.value.balance)?;
        Ok(InstructionOutput { cost: if account.warm { 100 } else { 2600 } })
    }

    fn origin(_s: &mut WorldState, tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        ctx.stack.push(tx.from.0)?;
        Ok(InstructionOutput { cost: 2 })
    }

    fn caller(_s: &mut WorldState, _tx: &Transaction, ctx: &mut ExecutionContext) -> InstructionResult {
        ctx.stack.push(ctx.contract.caller.0)?;
        Ok(InstructionOutput { cost: 2 })
    }
}

#[cfg(test)]
mod tests {
    use ethnum::{uint, U256};
    use crate::storage::StorageValue;
    use super::*;

    impl ExecutionContext {
        fn with_stop(&mut self, stop: bool) {
            self.stop = stop;
        }

        fn with_stack<T: Into::<u256> + Copy>(&mut self, stack: Vec<T>) {
            self.stack = Stack::new();
            for i in (0..stack.len()).rev() { self.stack.push(stack[i].into()).unwrap(); }
        }

        fn with_memory(&mut self, memory: &str) {
            self.memory = Memory(hex::decode(memory).unwrap());
        }

        fn with_contract(&mut self, contract: ExecutionContextContract) {
            self.contract = contract;
        }
    }

    impl WorldState {
        fn with_accounts(&mut self, accounts: &[(Address, Account)]) {
            self.accounts = Storage::<Address, Account>::default();
            for (address, account) in accounts {
                self.accounts.0.insert(*address, StorageValue {
                    original_value: account.clone(),
                    value: account.clone(),
                    warm: false,
                });
            }
        }
    }

    #[test]
    fn stop() {
        let context = &mut ExecutionContext::default();

        context.with_stop(false);
        assert_eq!(Machine::stop(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 0 }));
        assert!(context.stop);
    }

    #[test]
    fn add() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::add(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 16);

        context.with_stack(vec![U256::MAX, U256::ONE]);
        assert_eq!(Machine::add(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);
    }

    #[test]
    fn mul() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::mul(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 60);

        context.with_stack(vec![U256::MAX, uint!("2")]);
        assert_eq!(Machine::mul(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), U256::MAX - 1);
    }

    #[test]
    fn sub() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::sub(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 4);

        context.with_stack(vec![0u8, 1]);
        assert_eq!(Machine::sub(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), U256::MAX);
    }

    
    #[test]
    fn div() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::div(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![6u8, 0]);
        assert_eq!(Machine::div(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0); // dividing by zero returns zero by convention
    }

    #[test]
    fn sdiv() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::sdiv(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![U256::MAX - 1, U256::MAX]);
        assert_eq!(Machine::sdiv(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 2);

        context.with_stack(vec![U256::MAX - 1, U256::ONE]);
        assert_eq!(Machine::sdiv(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), U256::MAX - 1);

        context.with_stack(vec![6u8, 0]);
        assert_eq!(Machine::sdiv(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0); // dividing by zero returns zero by convention
    }

    #[test]
    fn r#mod() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::r#mod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 4);

        context.with_stack(vec![10u8, 3]);
        assert_eq!(Machine::r#mod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![6u8, 0]);
        assert_eq!(Machine::r#mod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0); // modulo zero returns zero by convention
    }

    #[test]
    fn smod() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::smod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 4);

        context.with_stack(vec![3u8, 2]);
        assert_eq!(Machine::smod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![U256::MAX - 7, U256::MAX - 2]);
        assert_eq!(Machine::smod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), U256::MAX - 1);

        context.with_stack(vec![U256::MAX - 2, uint!("2")]);
        assert_eq!(Machine::smod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), U256::MAX);

        context.with_stack(vec![uint!("3"), U256::MAX - 1]);
        assert_eq!(Machine::smod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![6u8, 0]);
        assert_eq!(Machine::smod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0); // modulo zero returns zero by convention
    }
   
    #[test]
    fn addmod() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 10, 8]);
        assert_eq!(Machine::addmod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 4);

        context.with_stack(vec![U256::MAX, uint!("2"), uint!("2")]);
        assert_eq!(Machine::addmod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![U256::MAX - 2, uint!("2"), uint!("3")]);
        assert_eq!(Machine::addmod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![U256::MAX, uint!("1"), uint!("10")]);
        assert_eq!(Machine::addmod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 6);

        context.with_stack(vec![4u8, 6, 0]);
        assert_eq!(Machine::addmod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0); // modulo zero returns zero by convention
    }
    
    #[test]
    fn mulmod() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 10, 8]);
        assert_eq!(Machine::mulmod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 4);

        context.with_stack(vec![U256::MAX, U256::MAX, uint!("12")]);
        assert_eq!(Machine::mulmod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 9);

        context.with_stack(vec![U256::MAX - 2, uint!("2"), uint!("3")]);
        assert_eq!(Machine::mulmod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 2);

        context.with_stack(vec![4u8, 6, 0]);
        assert_eq!(Machine::mulmod(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 8 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);
    }


    #[test]
    fn exp() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 2]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 60 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 100);

        context.with_stack(vec![2u8, 2]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 60 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 4);

        context.with_stack(vec![5u8, 0]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 10 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![2u8, 10]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 60 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1024);

        context.with_stack(vec![2u16, 260]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 110 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![uint!("0xFFFFFFFFFFFFFFFF"), uint!("3")]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 60 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0xFFFFFFFFFFFFFFFD0000000000000002FFFFFFFFFFFFFFFF"));

        context.with_stack(vec![uint!("3"), uint!("0xFFFFFFFFFFFFFFF0")]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 410 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0xE9377A20E36295B65EA7F55D4A333F73CF25A1BE32FEBCF9702BDE500F57B8C1"));

        context.with_stack(vec![uint!("5"), uint!("0xFFFFFFFFFFFFFFF0FFFFFF")]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 560 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0x49E63006C06484CE7E18DB842AD1771FC1C83AA03B09227A2EB3765958CCCCCD"));
    }

    #[test]
    fn signextend() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![0u8, 0x41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0x41);

        context.with_stack(vec![0u16, 0xEF41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0x41);

        context.with_stack(vec![1u16, 0xEF41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEF41"));

        context.with_stack(vec![2u16, 0xEF41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xEF41);

        context.with_stack(vec![30u16, 0xEF41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xEF41);

        context.with_stack(vec![31u16, 0xEF41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xEF41);

        context.with_stack(vec![uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0xEF41")]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 5 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xEF41);
    }


    #[test]
    fn lt() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![9u8, 10]);
        assert_eq!(Machine::lt(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![10u8, 10]);
        assert_eq!(Machine::lt(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);
    }

    #[test]
    fn gt() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 9]);
        assert_eq!(Machine::gt(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![10u8, 10]);
        assert_eq!(Machine::gt(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);
    }

    #[test]
    fn eq() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![10u8, 10]);
        assert_eq!(Machine::eq(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![10u8, 3]);
        assert_eq!(Machine::eq(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);
    }

    #[test]
    fn iszero() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![0u8]);
        assert_eq!(Machine::iszero(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![3u8]);
        assert_eq!(Machine::iszero(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);
    }

    #[test]
    fn slt() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![U256::MAX, U256::ONE]);
        assert_eq!(Machine::slt(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![U256::MAX, U256::MAX - 1]);
        assert_eq!(Machine::slt(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![U256::ZERO, U256::MAX]);
        assert_eq!(Machine::slt(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![1u8, 2]);
        assert_eq!(Machine::slt(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![10u8, 10]);
        assert_eq!(Machine::slt(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);
    }

    #[test]
    fn sgt() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![U256::MAX, U256::ZERO]);
        assert_eq!(Machine::sgt(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![U256::MAX, U256::MAX - 1]);
        assert_eq!(Machine::sgt(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![U256::ZERO, U256::MAX]);
        assert_eq!(Machine::sgt(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![1u8, 2]);
        assert_eq!(Machine::sgt(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![10u8, 10]);
        assert_eq!(Machine::sgt(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);
    }

    #[test]
    fn and() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![0xFFu8, 0xFF]);
        assert_eq!(Machine::and(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xFF);

        context.with_stack(vec![0u8, 0xFF]);
        assert_eq!(Machine::and(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![0xF0u8, 0xFF]);
        assert_eq!(Machine::and(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xF0);
    }

    #[test]
    fn or() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![0xFFu8, 0xFF]);
        assert_eq!(Machine::or(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xFF);

        context.with_stack(vec![0u8, 0xFF]);
        assert_eq!(Machine::or(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xFF);

        context.with_stack(vec![0xF0u8, 0xFF]);
        assert_eq!(Machine::or(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xFF);
    }

    #[test]
    fn xor() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![0xFFu8, 0xFF]);
        assert_eq!(Machine::xor(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![0u8, 0xFF]);
        assert_eq!(Machine::xor(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xFF);

        context.with_stack(vec![0xF0u8, 0xFF]);
        assert_eq!(Machine::xor(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0x0F);
    }

    #[test]
    fn not() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![0u8]);
        assert_eq!(Machine::not(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), U256::MAX);

        context.with_stack(vec![U256::MAX]);
        assert_eq!(Machine::not(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![0xF0u8]);
        assert_eq!(Machine::not(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0F"));
    }

    #[test]
    fn byte() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![uint!("16"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Machine::byte(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![uint!("31"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Machine::byte(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xF0);

        context.with_stack(vec![uint!("15"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Machine::byte(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![uint!("32"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Machine::byte(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![uint!("28"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Machine::byte(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0xCD);

        context.with_stack(vec![uint!("19"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Machine::byte(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0x34);
    }

    #[test]
    fn shl() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![1u8, 1]);
        assert_eq!(Machine::shl(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 2);

        context.with_stack(vec![uint!("4"), uint!("0xFF00000000000000000000000000000000000000000000000000000000000000")]);
        assert_eq!(Machine::shl(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0xF000000000000000000000000000000000000000000000000000000000000000"));
    }

    #[test]
    fn shr() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![1u8, 2]);
        assert_eq!(Machine::shr(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![4u8, 0xFFu8]);
        assert_eq!(Machine::shr(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0x0F);
    }

    #[test]
    fn sar() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![1u8, 2]);
        assert_eq!(Machine::sar(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 1);

        context.with_stack(vec![uint!("4"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);
        assert_eq!(Machine::sar(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), U256::MAX);

        context.with_stack(vec![uint!("600"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);
        assert_eq!(Machine::sar(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), U256::MAX);

        context.with_stack(vec![U256::MAX, uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);
        assert_eq!(Machine::sar(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), U256::MAX);

        context.with_stack(vec![U256::MAX, uint!("0x0FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);
        assert_eq!(Machine::sar(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![uint!("0"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);
        assert_eq!(Machine::sar(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0"));

        context.with_stack(vec![uint!("4"), uint!("0xEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFAB00")]);
        assert_eq!(Machine::sar(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 3 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0xFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFAB0"));
    }

    #[test]
    fn keccak256() {
        let context = &mut ExecutionContext::default();

        context.with_stack(vec![0u8, 4]);
        context.with_memory("FFFFFFFF00000000000000000000000000000000000000000000000000000000");
        assert_eq!(Machine::keccak256(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 36 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0x29045A592007D0C246EF02C2223570DA9522D0CF0F73282C79A1BC8F0BB2C238"));

        context.with_stack(vec![4u8, 40]);
        context.with_memory("FFFFFFFF00000000000000000000000000000000000000000000000000000000");
        assert_eq!(Machine::keccak256(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 45 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0xDAA77426C30C02A43D9FBA4E841A6556C524D47030762EB14DC4AF897E605D9B"));
    }

    #[test]
    fn address() {
        let context = &mut ExecutionContext::default();

        context.with_contract(ExecutionContextContract {
            address: Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")),
            caller: Address(U256::ZERO),
        });
        assert_eq!(Machine::address(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 2 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91"));
    }

    #[test]
    fn balance() {
        let state = &mut WorldState::default();
        let context = &mut ExecutionContext::default();

        state.with_accounts(&[(Address(uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")), Account { balance: uint!("125985"), code: vec![] })]);

        context.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]);
        assert_eq!(Machine::balance(state, &Transaction::default(), context), Ok(InstructionOutput { cost: 2600 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 125985);

        context.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]);
        assert_eq!(Machine::balance(state, &Transaction::default(), context), Ok(InstructionOutput { cost: 100 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 125985);

        context.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D9")]);
        assert_eq!(Machine::balance(state, &Transaction::default(), context), Ok(InstructionOutput { cost: 2600 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D9")]);
        assert_eq!(Machine::balance(state, &Transaction::default(), context), Ok(InstructionOutput { cost: 100 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), 0);

        context.with_stack(vec![uint!("0x109BBFED6889322E016E0A02EE459D306FC19545D9")]);
        assert_eq!(Machine::balance(state, &Transaction::default(), context), Err(Error::InvalidAddress));
    }

    #[test]
    fn origin() {
        let context = &mut ExecutionContext::default();
        let tx = Transaction { data: vec![], from: Address(uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")), gas: 0, nonce: 0, to: Address(U256::ZERO), value: U256::ZERO };

        assert_eq!(Machine::origin(&mut WorldState::default(), &tx, context), Ok(InstructionOutput { cost: 2 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8"));

    }

    #[test]
    fn caller() {
        let context = &mut ExecutionContext::default();

        context.with_contract(ExecutionContextContract {
            address: Address(U256::ZERO),
            caller: Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")),
        });
        assert_eq!(Machine::caller(&mut WorldState::default(), &Transaction::default(), context), Ok(InstructionOutput { cost: 2 }));
        assert_eq!(Machine::pop_or_fail(context).unwrap(), uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91"));
    }
}
