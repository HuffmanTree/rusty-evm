use std::cmp::min;

use ethnum::{u256,U256};
use crate::storage::Storage;
use crate::transient::Transient;
use crate::utils::{NeededSizeInBytes,IsNeg,WrappingSignedDiv,WrappingSignedRem,WrappingBigPow};
use crate::memory::Memory;

pub struct TransitionContext<'a> {
    pub code: &'a Vec<u8>,
    pub gas: &'a usize,
    pub memory: &'a mut Memory,
    pub pc: &'a mut usize,
    pub stop_flag: &'a mut bool,
    pub storage: &'a mut Storage,
    pub transient: &'a mut Transient,
}

type TransitionFunctionInput<const I: usize> = [u256; I];

#[derive(PartialEq, Eq, Debug)]
pub struct TransitionFunctionOutput<const O: usize> {
    pub cost: usize,
    pub result: [u256; O],
    pub jump: usize,
}

pub type TransitionFunction<const I: usize, const O: usize> = fn(&mut TransitionContext, TransitionFunctionInput<I>) -> Result<TransitionFunctionOutput<O>, String>;

#[derive(Debug,PartialEq,Eq)]
pub struct TransitionOutput {
    pub cost: usize,
    pub jump: usize,
}

fn try_jump(code: &Vec<u8>, counter: u256) -> Result<usize, String> {
    let invalid_jumpdest: Result<usize, String> = Err("Invalid jump destination".to_string());
    let counter: usize = match counter.try_into() {
        Ok(x) => x,
        _ => return invalid_jumpdest,
    };
    match code.get(counter) {
        Some(x) => if *x == 0x5B { Ok(counter) } else { invalid_jumpdest },
        _ => invalid_jumpdest,
    }
}

pub static STOP: TransitionFunction<0, 0> = |context, []| { *context.stop_flag = true; Ok(TransitionFunctionOutput { cost: 0, result: [], jump: 0 }) };
pub static ADD: TransitionFunction<2, 1> = |_, [a, b]| Ok(TransitionFunctionOutput { cost: 3, result: [a.wrapping_add(b)], jump: 1 });
pub static MUL: TransitionFunction<2, 1> = |_, [a, b]| Ok(TransitionFunctionOutput { cost: 5, result: [a.wrapping_mul(b)], jump: 1 });
pub static SUB: TransitionFunction<2, 1> = |_, [a, b]| Ok(TransitionFunctionOutput { cost: 3, result: [a.wrapping_sub(b)], jump: 1 });
pub static DIV: TransitionFunction<2, 1> = |_, [a, b]| Ok(TransitionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_div(b) }], jump: 1 });
pub static SDIV: TransitionFunction<2, 1> = |_, [a, b]| Ok(TransitionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_signed_div(b) }], jump: 1 });
pub static MOD: TransitionFunction<2, 1> = |_, [a, b]| Ok(TransitionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_rem(b) }], jump: 1 });
pub static SMOD: TransitionFunction<2, 1> = |_, [a, b]| Ok(TransitionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_signed_rem(b) }], jump: 1 });
pub static ADDMOD: TransitionFunction<3, 1> = |_, [a, b, n]| Ok(TransitionFunctionOutput { cost: 8, result: [if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_add(b.wrapping_rem(n)).wrapping_rem(n) }], jump: 1 });
pub static MULMOD: TransitionFunction<3, 1> = |_, [a, b, n]| Ok(TransitionFunctionOutput { cost: 8, result: [if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_mul(b.wrapping_rem(n)).wrapping_rem(n) }], jump: 1 });
pub static EXP: TransitionFunction<2, 1> = |_, [a, e]| Ok(TransitionFunctionOutput { cost: 10 + 50 * e.needed_size_in_bytes(), result: [a.wrapping_big_pow(e)], jump: 1 });
pub static SIGNEXTEND: TransitionFunction<2, 1> = |_, [b, x]| {
    let b: u32 = min(b, u256::from(30_u32)).try_into().unwrap();
    let mask = U256::ONE.wrapping_shl((b + 1).wrapping_shl(3));
    let sign_mask = mask.wrapping_shr(1);
    let size_mask = mask - 1;
    let value = x & size_mask;
    Ok(TransitionFunctionOutput { cost: 5, result: [if (value & sign_mask) != 0 { !size_mask | value } else { value }], jump: 1 })
};
pub static LT: TransitionFunction<2, 1> = |_, [a, b]| Ok(TransitionFunctionOutput { cost: 3, result: [if a < b { U256::ONE } else { U256::ZERO }], jump: 1 });
pub static GT: TransitionFunction<2, 1> = |_, [a, b]| Ok(TransitionFunctionOutput { cost: 3, result: [if a > b { U256::ONE } else { U256::ZERO }], jump: 1 });
pub static SLT: TransitionFunction<2, 1> = |_, [a, b]| Ok(TransitionFunctionOutput { cost: 3, result: [match (a.is_neg(), b.is_neg()) {
    (true, false) => { U256::ONE },
    (false, true) => { U256::ZERO },
    _ => if a < b { U256::ONE } else { U256::ZERO },
}], jump: 1 });
pub static SGT: TransitionFunction<2, 1> = |_, [a, b]| Ok(TransitionFunctionOutput { cost: 3, result: [match (a.is_neg(), b.is_neg()) {
    (true, false) => { U256::ZERO },
    (false, true) => { U256::ONE },
    _ => if a > b { U256::ONE } else { U256::ZERO },
}], jump: 1 });
pub static EQ: TransitionFunction<2, 1> = |_, [a, b]| Ok(TransitionFunctionOutput { cost: 3, result: [if a == b { U256::ONE } else { U256::ZERO }], jump: 1 });
pub static ISZERO: TransitionFunction<1, 1> = |_, [a]| Ok(TransitionFunctionOutput { cost: 3, result: [if a == U256::ZERO { U256::ONE } else { U256::ZERO }], jump: 1 });
pub static AND: TransitionFunction<2, 1> = |_, [a, b]| Ok(TransitionFunctionOutput { cost: 3, result: [a & b], jump: 1 });
pub static OR: TransitionFunction<2, 1> = |_, [a, b]| Ok(TransitionFunctionOutput { cost: 3, result: [a | b], jump: 1 });
pub static XOR: TransitionFunction<2, 1> = |_, [a, b]| Ok(TransitionFunctionOutput { cost: 3, result: [a ^ b], jump: 1 });
pub static NOT: TransitionFunction<1, 1> = |_, [a]| Ok(TransitionFunctionOutput { cost: 3, result: [!a], jump: 1 });
pub static BYTE: TransitionFunction<2, 1> = |_, [i, x]| Ok(TransitionFunctionOutput { cost: 3, result: [if i > 31 { U256::ZERO } else { (x >> (8 * (31 - i))) & 0xFF }], jump: 1 });
pub static SHL: TransitionFunction<2, 1> = |_, [shift, value]| Ok(TransitionFunctionOutput { cost: 3, result: [match TryInto::<u8>::try_into(shift) {
    Ok(shift) => value.wrapping_shl(shift.into()),
    _ => U256::ZERO,
}], jump: 1 });
pub static SHR: TransitionFunction<2, 1> = |_, [shift, value]| Ok(TransitionFunctionOutput { cost: 3, result: [match TryInto::<u8>::try_into(shift) {
    Ok(shift) => value.wrapping_shr(shift.into()),
    _ => U256::ZERO,
}], jump: 1 });
pub static SAR: TransitionFunction<2, 1> = |_, [shift, value]| Ok(TransitionFunctionOutput { cost: 3, result: [match (TryInto::<u8>::try_into(shift), value.is_neg()) {
    (Ok(shift), false) => value.wrapping_shr(shift.into()),
    (Ok(shift), true) => { if shift == 0 { value } else { !(U256::ONE.wrapping_shl((255 - shift + 1).into()) - 1) | value.wrapping_shr(shift.into()) } },
    (Err(_), false) => U256::ZERO,
    (Err(_), true) => U256::MAX,
}], jump: 1 });
// (fguerin - 11/11/2024) Implement opcodes 0x20 - 0x4A
pub static POP: TransitionFunction<1, 0> = |_, [_x]| Ok(TransitionFunctionOutput { cost: 2, result: [], jump: 1 });
pub static MLOAD: TransitionFunction<1, 1> = |context, [offset]| {
    let (_, dynamic_cost, res) = context.memory.load_word(offset)?;
    Ok(TransitionFunctionOutput { cost: 3 + dynamic_cost, result: [res], jump: 1 })
};
pub static MSTORE: TransitionFunction<2, 0> = |context, [offset, value]| {
    let (_, dynamic_cost) = context.memory.store_word(offset, value)?;
    Ok(TransitionFunctionOutput { cost: 3 + dynamic_cost, result: [], jump: 1 })
};
pub static MSTORE8: TransitionFunction<2, 0> = |context, [offset, value]| {
    let (_, dynamic_cost) = context.memory.store_byte(offset, value)?;
    Ok(TransitionFunctionOutput { cost: 3 + dynamic_cost, result: [], jump: 1 })
};
pub static SLOAD: TransitionFunction<1, 1> = |context, [key]| {
    let res = context.storage.load(key);
    Ok(TransitionFunctionOutput { cost: if res.warm { 100 } else { 2100 }, result: [res.value], jump: 1 })
};
// TODO (fguerin - 17/11/2024) Add gas refund
pub static SSTORE: TransitionFunction<2, 0> = |context, [key, value]| {
    let (current_value, original_value, warm) = match context.storage.store(key, value) {
        Some(v) => (v.value, v.original_value, v.warm),
        None => (U256::ZERO, U256::ZERO, false),
    };
    let base_cost: usize =
        if value == current_value { 100 }         // the value does not change
        else if current_value == original_value { // the storage slot is clean ...
            if original_value == 0 { 20000 }      // ... and has not explicit value
            else { 2900 }                         // ... and has an explicit value
        }
        else { 100 };                             // the value changes and the storage slot is dirty
    Ok(TransitionFunctionOutput { cost: base_cost + if warm { 0 } else { 2100 }, result: [], jump: 1 })
};
pub static JUMP: TransitionFunction<1, 0> = |context, [counter]| {
    *context.pc = try_jump(context.code, counter)?;
    Ok(TransitionFunctionOutput { cost: 8, result: [], jump: 0 })
};
pub static JUMPI: TransitionFunction<2, 0> = |context, [counter, b]| {
    if b == U256::ZERO {
        Ok(TransitionFunctionOutput { cost: 10, result: [], jump: 1 })
    } else {
        *context.pc = try_jump(context.code, counter)?;
        Ok(TransitionFunctionOutput { cost: 10, result: [], jump: 0 })
    }
};
pub static PC: TransitionFunction<0, 1> = |context, []| Ok(TransitionFunctionOutput { cost: 2, result: [u256::from(TryInto::<u64>::try_into(*context.pc).unwrap())], jump: 1 });
pub static MSIZE: TransitionFunction<0, 1> = |context, []| Ok(TransitionFunctionOutput { cost: 2, result: [u256::from(TryInto::<u64>::try_into(context.memory.size()).unwrap())], jump: 1 });
pub static GAS: TransitionFunction<0, 1> = |context, []| Ok(TransitionFunctionOutput { cost: 2, result: [if *context.gas >= 2 { u256::from(TryInto::<u64>::try_into(*context.gas - 2).unwrap()) } else { U256::ZERO }], jump: 1 });
pub static JUMPDEST: TransitionFunction<0, 0> = |_, []| Ok(TransitionFunctionOutput { cost: 1, result: [], jump: 1 });
pub static TLOAD: TransitionFunction<1, 1> = |context, [key]| Ok(TransitionFunctionOutput { cost: 100, result: [context.transient.load(key)], jump: 1 });
pub static TSTORE: TransitionFunction<2, 0> = |context, [key, value]| {
    context.transient.store(key, value);
    Ok(TransitionFunctionOutput { cost: 100, result: [], jump: 1 })
};

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::storage::StorageValue;

    use super::*;
    use ethnum::{uint,u256};

    #[test]
    fn stop() {
        let mut context = TransitionContext { code: &Default::default(), gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert!(!*context.stop_flag);
        assert_eq!(STOP(&mut context, []), Ok(TransitionFunctionOutput { cost: 0, result: [], jump: 0 }));
        assert!(*context.stop_flag);
    }

    #[test]
    fn add() {
        let mut context = TransitionContext { code: &Default::default(), gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(ADD(&mut context, [uint!("10"), uint!("6")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("16")], jump: 1 }));
        assert_eq!(ADD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("1")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn mul() {
        let mut context = TransitionContext { code: &Default::default(), gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(MUL(&mut context, [uint!("10"), uint!("6")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("60")], jump: 1 }));
        assert_eq!(MUL(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("2")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")], jump: 1 }));
    }

    #[test]
    fn sub() {
        let mut context = TransitionContext { code: &Default::default(), gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(SUB(&mut context, [uint!("10"), uint!("6")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("4")], jump: 1 }));
        assert_eq!(SUB(&mut context, [uint!("0"), uint!("1")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")], jump: 1 }));
    }

    #[test]
    fn div() {
        let mut context = TransitionContext { code: &Default::default(), gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(DIV(&mut context, [uint!("10"), uint!("6")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("1")], jump: 1 }));
        assert_eq!(DIV(&mut context, [uint!("6"), uint!("0")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("0")], jump: 1 })); // dividing by zero returns zero by convention
    }

    #[test]
    fn sdiv() {
        let mut context = TransitionContext { code: &Default::default(), gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(SDIV(&mut context, [uint!("10"), uint!("6")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("1")], jump: 1 }));
        assert_eq!(SDIV(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("2")], jump: 1 }));
        assert_eq!(SDIV(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE"), uint!("1")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")], jump: 1 }));
        assert_eq!(SDIV(&mut context, [uint!("6"), uint!("0")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("0")], jump: 1 })); // dividing by zero returns zero by convention
    }

    #[test]
    fn r#mod() {
        let mut context = TransitionContext { code: &Default::default(), gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(MOD(&mut context, [uint!("10"), uint!("6")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("4")], jump: 1 }));
        assert_eq!(MOD(&mut context, [uint!("6"), uint!("0")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("0")], jump: 1 })); // modulo zero returns zero by convention
    }

    #[test]
    fn smod() {
        let mut context = TransitionContext { code: &Default::default(), gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(SMOD(&mut context, [uint!("10"), uint!("6")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("4")], jump: 1 }));
        assert_eq!(SMOD(&mut context, [uint!("3"), uint!("2")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("1")], jump: 1 }));
        assert_eq!(SMOD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF8"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFD")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")], jump: 1 }));
        assert_eq!(SMOD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFD"), uint!("2")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")], jump: 1 }));
        assert_eq!(SMOD(&mut context, [uint!("3"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("1")], jump: 1 }));
        assert_eq!(SMOD(&mut context, [uint!("6"), uint!("0")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("0")], jump: 1 })); // modulo zero returns zero by convention
    }

    #[test]
    fn addmod() {
        let mut context = TransitionContext { code: &Default::default(), gas: &100, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(ADDMOD(&mut context, [uint!("10"), uint!("10"), uint!("8")]), Ok(TransitionFunctionOutput { cost: 8, result: [uint!("4")], jump: 1 }));
        assert_eq!(ADDMOD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("2"), uint!("2")]), Ok(TransitionFunctionOutput { cost: 8, result: [uint!("1")], jump: 1 }));
        assert_eq!(ADDMOD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFD"), uint!("2"), uint!("3")]), Ok(TransitionFunctionOutput { cost: 8, result: [uint!("0")], jump: 1 }));
        assert_eq!(ADDMOD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("1"), uint!("10")]), Ok(TransitionFunctionOutput { cost: 8, result: [uint!("6")], jump: 1 }));
        assert_eq!(ADDMOD(&mut context, [uint!("4"), uint!("6"), uint!("0")]), Ok(TransitionFunctionOutput { cost: 8, result: [uint!("0")], jump: 1 })); // modulo zero returns zero by convention
    }

    #[test]
    fn mulmod() {
        let mut context = TransitionContext { code: &Default::default(), gas: &100, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(MULMOD(&mut context, [uint!("10"), uint!("10"), uint!("8")]), Ok(TransitionFunctionOutput { cost: 8, result: [uint!("4")], jump: 1 }));
        assert_eq!(MULMOD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("12")]), Ok(TransitionFunctionOutput { cost: 8, result: [uint!("9")], jump: 1 }));
        assert_eq!(MULMOD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFD"), uint!("2"), uint!("3")]), Ok(TransitionFunctionOutput { cost: 8, result: [uint!("2")], jump: 1 }));
        assert_eq!(MULMOD(&mut context, [uint!("4"), uint!("6"), uint!("0")]), Ok(TransitionFunctionOutput { cost: 8, result: [uint!("0")], jump: 1 })); // modulo zero returns zero by convention
    }

    #[test]
    fn exp() {
        let mut context = TransitionContext { code: &Default::default(), gas: &1400, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(EXP(&mut context, [uint!("10"), uint!("2")]), Ok(TransitionFunctionOutput { cost: 60, result: [uint!("100")], jump: 1 }));
        assert_eq!(EXP(&mut context, [uint!("2"), uint!("2")]), Ok(TransitionFunctionOutput { cost: 60, result: [uint!("4")], jump: 1 }));
        assert_eq!(EXP(&mut context, [uint!("5"), uint!("0")]), Ok(TransitionFunctionOutput { cost: 10, result: [uint!("1")], jump: 1 }));
        assert_eq!(EXP(&mut context, [uint!("2"), uint!("10")]), Ok(TransitionFunctionOutput { cost: 60, result: [uint!("1024")], jump: 1 }));
        assert_eq!(EXP(&mut context, [uint!("2"), uint!("260")]), Ok(TransitionFunctionOutput { cost: 110, result: [uint!("0")], jump: 1 }));
        assert_eq!(EXP(&mut context, [uint!("0xFFFFFFFFFFFFFFFF"), uint!("3")]), Ok(TransitionFunctionOutput { cost: 60, result: [uint!("0xFFFFFFFFFFFFFFFD0000000000000002FFFFFFFFFFFFFFFF")], jump: 1 }));
        assert_eq!(EXP(&mut context, [uint!("3"), uint!("0xFFFFFFFFFFFFFFF0")]), Ok(TransitionFunctionOutput { cost: 410, result: [uint!("0xE9377A20E36295B65EA7F55D4A333F73CF25A1BE32FEBCF9702BDE500F57B8C1")], jump: 1 }));
        assert_eq!(EXP(&mut context, [uint!("5"), uint!("0xFFFFFFFFFFFFFFF0FFFFFF")]), Ok(TransitionFunctionOutput { cost: 560, result: [uint!("0x49E63006C06484CE7E18DB842AD1771FC1C83AA03B09227A2EB3765958CCCCCD")], jump: 1 }));
    }

    #[test]
    fn signextend() {
        let mut context = TransitionContext { code: &Default::default(), gas: &200, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(SIGNEXTEND(&mut context, [uint!("0"), uint!("0x41")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("0x41")], jump: 1 }));
        assert_eq!(SIGNEXTEND(&mut context, [uint!("0"), uint!("0xEF41")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("0x41")], jump: 1 }));
        assert_eq!(SIGNEXTEND(&mut context, [uint!("1"), uint!("0xEF41")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEF41")], jump: 1 }));
        assert_eq!(SIGNEXTEND(&mut context, [uint!("2"), uint!("0xEF41")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("0xEF41")], jump: 1 }));
        assert_eq!(SIGNEXTEND(&mut context, [uint!("30"), uint!("0xEF41")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("0xEF41")], jump: 1 }));
        assert_eq!(SIGNEXTEND(&mut context, [uint!("31"), uint!("0xEF41")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("0xEF41")], jump: 1 }));
        assert_eq!(SIGNEXTEND(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0xEF41")]), Ok(TransitionFunctionOutput { cost: 5, result: [uint!("0xEF41")], jump: 1 }));
    }

    #[test]
    fn lt() {
        let mut context = TransitionContext { code: &Default::default(), gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(LT(&mut context, [uint!("9"), uint!("10")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(LT(&mut context, [uint!("10"), uint!("10")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn gt() {
        let mut context = TransitionContext { code: &Default::default(), gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(GT(&mut context, [uint!("10"), uint!("9")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(GT(&mut context, [uint!("10"), uint!("10")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn eq() {
        let mut context = TransitionContext { code: &Default::default(), gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(EQ(&mut context, [uint!("10"), uint!("10")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(EQ(&mut context, [uint!("10"), uint!("3")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn iszero() {
        let mut context = TransitionContext { code: &Default::default(), gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(ISZERO(&mut context, [uint!("0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(ISZERO(&mut context, [uint!("3")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn slt() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(SLT(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(SLT(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(SLT(&mut context, [uint!("0"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(SLT(&mut context, [uint!("1"), uint!("2")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(SLT(&mut context, [uint!("10"), uint!("10")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn sgt() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(SGT(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(SGT(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(SGT(&mut context, [uint!("0"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(SGT(&mut context, [uint!("1"), uint!("2")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(SGT(&mut context, [uint!("10"), uint!("10")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn and() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(AND(&mut context, [uint!("0xFF"), uint!("0xFF")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xFF")], jump: 1 }));
        assert_eq!(AND(&mut context, [uint!("0"), uint!("0xFF")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(AND(&mut context, [uint!("0xF0"), uint!("0xFF")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xF0")], jump: 1 }));
    }

    #[test]
    fn or() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(OR(&mut context, [uint!("0xFF"), uint!("0xFF")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xFF")], jump: 1 }));
        assert_eq!(OR(&mut context, [uint!("0"), uint!("0xFF")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xFF")], jump: 1 }));
        assert_eq!(OR(&mut context, [uint!("0xF0"), uint!("0xFF")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xFF")], jump: 1 }));
    }

    #[test]
    fn xor() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(XOR(&mut context, [uint!("0xFF"), uint!("0xFF")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(XOR(&mut context, [uint!("0"), uint!("0xFF")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xFF")], jump: 1 }));
        assert_eq!(XOR(&mut context, [uint!("0xF0"), uint!("0xFF")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0x0F")], jump: 1 }));
    }

    #[test]
    fn not() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(NOT(&mut context, [uint!("0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")], jump: 1 }));
        assert_eq!(NOT(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(NOT(&mut context, [uint!("0xF0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0F")], jump: 1 }));
    }

    #[test]
    fn byte() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(BYTE(&mut context, [uint!("16"), uint!("0x0112233445566778899AABBCCDDEEFF0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(BYTE(&mut context, [uint!("31"), uint!("0x0112233445566778899AABBCCDDEEFF0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xF0")], jump: 1 }));
        assert_eq!(BYTE(&mut context, [uint!("15"), uint!("0x0112233445566778899AABBCCDDEEFF0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(BYTE(&mut context, [uint!("32"), uint!("0x0112233445566778899AABBCCDDEEFF0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(BYTE(&mut context, [uint!("28"), uint!("0x0112233445566778899AABBCCDDEEFF0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xCD")], jump: 1 }));
        assert_eq!(BYTE(&mut context, [uint!("19"), uint!("0x0112233445566778899AABBCCDDEEFF0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0x34")], jump: 1 }));
    }

    #[test]
    fn shl() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(SHL(&mut context, [uint!("1"), uint!("1")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("2")], jump: 1 }));
        assert_eq!(SHL(&mut context, [uint!("4"), uint!("0xFF00000000000000000000000000000000000000000000000000000000000000")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xF000000000000000000000000000000000000000000000000000000000000000")], jump: 1 }));
    }

    #[test]
    fn shr() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(SHR(&mut context, [uint!("1"), uint!("2")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(SHR(&mut context, [uint!("4"), uint!("0xFF")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0x0F")], jump: 1 }));
    }

    #[test]
    fn sar() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(SAR(&mut context, [uint!("1"), uint!("2")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(SAR(&mut context, [uint!("4"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")], jump: 1 }));
        assert_eq!(SAR(&mut context, [uint!("600"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")], jump: 1 }));
        assert_eq!(SAR(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")], jump: 1 }));
        assert_eq!(SAR(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0x0FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(SAR(&mut context, [uint!("0"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")], jump: 1 }));
        assert_eq!(SAR(&mut context, [uint!("4"), uint!("0xEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFAB00")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0xFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFAB0")], jump: 1 }));
    }

    #[test]
    fn pop() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(POP(&mut context, [uint!("42")]), Ok(TransitionFunctionOutput { cost: 2, result: [], jump: 1 }));
    }

    #[test]
    fn mload_no_memory_extension() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory(vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69]), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(MLOAD(&mut context, [uint!("0")]), Ok(TransitionFunctionOutput { cost: 3, result: [uint!("0x4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369")], jump: 1 }));
        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(context.memory.0, vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69]);

        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory(vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69]), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(MLOAD(&mut context, [uint!("2")]), Ok(TransitionFunctionOutput { cost: 6, result: [uint!("0xB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A023690000")], jump: 1 }));
        assert_eq!(context.memory.0.len(), 64);
        assert_eq!(context.memory.0, vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory(vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69]), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(MLOAD(&mut context, [uint!("30")]), Ok(TransitionFunctionOutput { cost: 6, result: [uint!("0x2369000000000000000000000000000000000000000000000000000000000000")], jump: 1 }));
        assert_eq!(context.memory.0.len(), 64);
        assert_eq!(context.memory.0, vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory(vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69]), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(MLOAD(&mut context, [uint!("500")]), Ok(TransitionFunctionOutput { cost: 51, result: [uint!("0")], jump: 1 }));
        assert_eq!(context.memory.0.len(), 544);
        assert_eq!(context.memory.0, vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn mstore() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(context.memory.0.len(), 0);
        assert_eq!(MSTORE(&mut context, [uint!("0"), uint!("0xFF")]), Ok(TransitionFunctionOutput { cost: 6, result: [], jump: 1 }));
        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(context.memory.0, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF]);
        assert_eq!(MSTORE(&mut context, [uint!("1"), uint!("0xFF")]), Ok(TransitionFunctionOutput { cost: 6, result: [], jump: 1 }));
        assert_eq!(context.memory.0.len(), 64);
        assert_eq!(context.memory.0, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(context.memory.0.len(), 0);
        assert_eq!(MSTORE(&mut context, [uint!("3"), uint!("0xFF")]), Ok(TransitionFunctionOutput { cost: 9, result: [], jump: 1 }));
        assert_eq!(context.memory.0.len(), 64);
        assert_eq!(context.memory.0, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(context.memory.0.len(), 0);
        assert_eq!(MSTORE(&mut context, [uint!("500"), uint!("0xABFF")]), Ok(TransitionFunctionOutput { cost: 54, result: [], jump: 1 }));
        assert_eq!(context.memory.0.len(), 544);
        assert_eq!(context.memory.0, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xAB, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn mstore8() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(context.memory.0.len(), 0);
        assert_eq!(MSTORE8(&mut context, [uint!("0"), uint!("0xFFAB")]), Ok(TransitionFunctionOutput { cost: 6, result: [], jump: 1 }));
        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(context.memory.0, vec![0xAB, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(MSTORE8(&mut context, [uint!("31"), uint!("0xFFAB")]), Ok(TransitionFunctionOutput { cost: 3, result: [], jump: 1 }));
        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(context.memory.0, vec![0xAB, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xAB]);
    }

    #[test]
    fn sload() {
        let mut initial_storage = HashMap::<u256, u256>::new();
        initial_storage.insert(uint!("42"), uint!("0xAB"));
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(initial_storage), transient: &mut Transient::new() };

        assert_eq!(SLOAD(&mut context, [uint!("42")]), Ok(TransitionFunctionOutput { cost: 2100, result: [uint!("0xAB")], jump: 1 }));
        assert_eq!(SLOAD(&mut context, [uint!("42")]), Ok(TransitionFunctionOutput { cost: 100, result: [uint!("0xAB")], jump: 1 }));
    }

    #[test]
    fn sstore() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(SSTORE(&mut context, [uint!("0"), uint!("0xFFFF")]), Ok(TransitionFunctionOutput { cost: 22100, result: [], jump: 1 })); // clean storage - no previous value - cold slot
        assert_eq!(context.storage.0.get(&uint!("0")), Some(&StorageValue { original_value: uint!("0"), value: uint!("0xFFFF"), warm: true }));
        assert_eq!(SSTORE(&mut context, [uint!("0"), uint!("0xFFFF")]), Ok(TransitionFunctionOutput { cost: 100, result: [], jump: 1 })); // dirty storage - same value - warn slot
        assert_eq!(context.storage.0.get(&uint!("0")), Some(&StorageValue { original_value: uint!("0"), value: uint!("0xFFFF"), warm: true }));
        assert_eq!(SSTORE(&mut context, [uint!("0"), uint!("0xFFF0")]), Ok(TransitionFunctionOutput { cost: 100, result: [], jump: 1 })); // dirty storage - different value - warn slot
        assert_eq!(context.storage.0.get(&uint!("0")), Some(&StorageValue { original_value: uint!("0"), value: uint!("0xFFF0"), warm: true }));

        let mut initial_storage = HashMap::<u256, u256>::new();
        initial_storage.insert(uint!("1"), uint!("55"));
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(initial_storage), transient: &mut Transient::new() };

        assert_eq!(SSTORE(&mut context, [uint!("1"), uint!("10")]), Ok(TransitionFunctionOutput { cost: 5000, result: [], jump: 1 })); // clean storage - different value - cold slot
        assert_eq!(context.storage.0.get(&uint!("1")), Some(&StorageValue { original_value: uint!("55"), value: uint!("10"), warm: true }));

        let mut initial_storage = HashMap::<u256, u256>::new();
        initial_storage.insert(uint!("1"), uint!("55"));
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(initial_storage), transient: &mut Transient::new() };

        assert_eq!(SSTORE(&mut context, [uint!("1"), uint!("55")]), Ok(TransitionFunctionOutput { cost: 2200, result: [], jump: 1 })); // clean storage - same value - cold slot
        assert_eq!(context.storage.0.get(&uint!("1")), Some(&StorageValue { original_value: uint!("55"), value: uint!("55"), warm: true }));
    }

    #[test]
    fn jump() {
        let mut context = TransitionContext { code: &vec![0_u8, 0_u8, 0x5B, 0_u8], gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(*context.pc, 0);
        assert_eq!(JUMP(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFF")]), Err("Invalid jump destination".to_string())); // not a usize
        assert_eq!(*context.pc, 0);
        assert_eq!(JUMP(&mut context, [uint!("0xFFFF")]), Err("Invalid jump destination".to_string())); // not in range
        assert_eq!(*context.pc, 0);
        assert_eq!(JUMP(&mut context, [uint!("1")]), Err("Invalid jump destination".to_string())); // not a valid destination
        assert_eq!(*context.pc, 0);
        assert_eq!(JUMP(&mut context, [uint!("2")]), Ok(TransitionFunctionOutput { cost: 8, result: [], jump: 0 }));
        assert_eq!(*context.pc, 2);
    }

    #[test]
    fn jumpi() {
        let mut context = TransitionContext { code: &vec![0_u8, 0_u8, 0x5B, 0_u8], gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(*context.pc, 0);
        assert_eq!(JUMPI(&mut context, [uint!("2"), uint!("0")]), Ok(TransitionFunctionOutput { cost: 10, result: [], jump: 1 })); // jump condition is false
        assert_eq!(*context.pc, 0);
        assert_eq!(JUMPI(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFF"), uint!("1")]), Err("Invalid jump destination".to_string())); // not a usize
        assert_eq!(*context.pc, 0);
        assert_eq!(JUMPI(&mut context, [uint!("0xFFFF"), uint!("1")]), Err("Invalid jump destination".to_string())); // not in range
        assert_eq!(*context.pc, 0);
        assert_eq!(JUMPI(&mut context, [uint!("1"), uint!("1")]), Err("Invalid jump destination".to_string())); // not a valid destination
        assert_eq!(*context.pc, 0);
        assert_eq!(JUMPI(&mut context, [uint!("2"), uint!("1")]), Ok(TransitionFunctionOutput { cost: 10, result: [], jump: 0 }));
        assert_eq!(*context.pc, 2);
    }

    #[test]
    fn pc() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 30, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(PC(&mut context, []), Ok(TransitionFunctionOutput { cost: 2, result: [uint!("30")], jump: 1 }));
    }

    #[test]
    fn msize() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory(vec![0; 64]), pc: &mut 30, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(MSIZE(&mut context, []), Ok(TransitionFunctionOutput { cost: 2, result: [uint!("64")], jump: 1 }));
    }


    #[test]
    fn gas() {
        let mut context = TransitionContext { code: &Default::default(), gas: &5, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(GAS(&mut context, []), Ok(TransitionFunctionOutput { cost: 2, result: [uint!("3")], jump: 1 }));

        let mut context = TransitionContext { code: &Default::default(), gas: &3, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(GAS(&mut context, []), Ok(TransitionFunctionOutput { cost: 2, result: [uint!("1")], jump: 1 }));

        let mut context = TransitionContext { code: &Default::default(), gas: &1, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(GAS(&mut context, []), Ok(TransitionFunctionOutput { cost: 2, result: [uint!("0")], jump: 1 }));

        let mut context = TransitionContext { code: &Default::default(), gas: &0, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(GAS(&mut context, []), Ok(TransitionFunctionOutput { cost: 2, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn jumpdest() {
        let mut context = TransitionContext { code: &Default::default(), gas: &5, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(JUMPDEST(&mut context, []), Ok(TransitionFunctionOutput { cost: 1, result: [], jump: 1 }));

    }

    #[test]
    fn tload() {
        let mut initial_transient = HashMap::<u256, u256>::new();
        initial_transient.insert(uint!("42"), uint!("0xAB"));
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient(initial_transient) };

        assert_eq!(TLOAD(&mut context, [uint!("42")]), Ok(TransitionFunctionOutput { cost: 100, result: [uint!("0xAB")], jump: 1 }));
        assert_eq!(TLOAD(&mut context, [uint!("45")]), Ok(TransitionFunctionOutput { cost: 100, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn tstore() {
        let mut context = TransitionContext { code: &Default::default(), gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new() };

        assert_eq!(TSTORE(&mut context, [uint!("1"), uint!("55")]), Ok(TransitionFunctionOutput { cost: 100, result: [], jump: 1 }));
        assert_eq!(context.transient.0.get(&uint!("1")), Some(&uint!("55")));
    }
}
