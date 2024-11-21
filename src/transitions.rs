use std::cmp::min;

use ethnum::{u256,U256};
use crate::storage::Storage;
use crate::utils::{NeededSizeInBytes,IsNeg,WrappingSignedDiv,WrappingSignedRem,WrappingBigPow};
use crate::memory::Memory;

pub struct TransitionContext<'a> {
    pub code: &'a Vec<u8>,
    pub memory: &'a mut Memory,
    pub pc: &'a mut usize,
    pub stop_flag: &'a mut bool,
    pub storage: &'a mut Storage,
}

type TransitionFunctionInput<const I: usize> = [u256; I];

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
