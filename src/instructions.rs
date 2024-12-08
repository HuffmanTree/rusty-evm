use std::cmp::min;
use ethnum::{u256, AsU256, U256};
use crate::errors::Error;
use crate::storage::Storage;
use crate::transaction::{Account, Address, Transaction};
use crate::transient::Transient;
use crate::utils::{Hash, IsNeg, NeededSizeInBytes, WrappingBigPow, WrappingSignedDiv, WrappingSignedRem};
use crate::memory::{Memory, ReadWriteOperation};

pub struct InstructionContext<'a> {
    pub accounts: &'a mut Storage<Address, Account>,
    pub caller: &'a Address,
    pub gas: &'a usize,
    pub memory: &'a mut Memory,
    pub pc: &'a mut usize,
    pub returndata: &'a mut Vec<u8>,
    pub stop_flag: &'a mut bool,
    pub revert_flag: &'a mut bool,
    pub storage: &'a mut Storage<u256, u256>,
    pub transaction: &'a Transaction,
    pub transient: &'a mut Transient,
}

type InstructionFunctionInput<const I: usize> = [u256; I];

#[derive(PartialEq, Eq, Debug)]
pub struct InstructionFunctionOutput<const O: usize> {
    pub cost: usize,
    pub result: [u256; O],
    pub jump: usize,
}

pub type InstructionFunction<const I: usize, const O: usize> = fn(&mut InstructionContext, InstructionFunctionInput<I>) -> Result<InstructionFunctionOutput<O>, Error>;

#[derive(Debug,PartialEq,Eq)]
pub struct InstructionOutput {
    pub cost: usize,
    pub jump: usize,
}

fn try_jump(code: &Vec<u8>, counter: u256) -> Result<usize, Error> {
    let counter: usize = match counter.try_into() {
        Ok(x) => x,
        _ => return Err(Error::InvalidJumpDest),
    };
    match code.get(counter) {
        Some(x) => if *x == 0x5B { Ok(counter) } else { Err(Error::InvalidJumpDest) },
        _ => Err(Error::InvalidJumpDest),
    }
}

fn push_n(pc: usize, code: &Vec<u8>, n: usize) -> InstructionFunctionOutput<1> {
    let mut res = U256::ZERO;
    for i in 0..n {
        res <<= 8;
        res |= u256::from(*code.get(pc + i + 1).unwrap_or(&0_u8));
    };
    InstructionFunctionOutput { cost: if n == 0 { 2 } else { 3 }, result: [res], jump: n + 1 }
}

fn dup_n<const I: usize, const O: usize>(input: [u256; I]) -> InstructionFunctionOutput<O> {
    let mut res = [U256::ZERO; O];

    res[0] = input[I - 1];
    for i in 0..I {
        res[i + 1] = input[i];
    }

    InstructionFunctionOutput { cost: 3, result: res, jump: 1 }
}

fn swap_n<const N: usize>(mut input: [u256; N]) -> InstructionFunctionOutput<N> {
    input.swap(0, N - 1);
    InstructionFunctionOutput { cost: 3, result: input, jump: 1 }
}

pub static STOP: InstructionFunction<0, 0> = |context, []| { *context.stop_flag = true; Ok(InstructionFunctionOutput { cost: 0, result: [], jump: 0 }) };
pub static ADD: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [a.wrapping_add(b)], jump: 1 });
pub static MUL: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 5, result: [a.wrapping_mul(b)], jump: 1 });
pub static SUB: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [a.wrapping_sub(b)], jump: 1 });
pub static DIV: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_div(b) }], jump: 1 });
pub static SDIV: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_signed_div(b) }], jump: 1 });
pub static MOD: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_rem(b) }], jump: 1 });
pub static SMOD: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_signed_rem(b) }], jump: 1 });
pub static ADDMOD: InstructionFunction<3, 1> = |_, [a, b, n]| Ok(InstructionFunctionOutput { cost: 8, result: [if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_add(b.wrapping_rem(n)).wrapping_rem(n) }], jump: 1 });
pub static MULMOD: InstructionFunction<3, 1> = |_, [a, b, n]| Ok(InstructionFunctionOutput { cost: 8, result: [if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_mul(b.wrapping_rem(n)).wrapping_rem(n) }], jump: 1 });
pub static EXP: InstructionFunction<2, 1> = |_, [a, e]| Ok(InstructionFunctionOutput { cost: 10 + 50 * e.needed_size_in_bytes(), result: [a.wrapping_big_pow(e)], jump: 1 });
pub static SIGNEXTEND: InstructionFunction<2, 1> = |_, [b, x]| {
    let b: u32 = min(b, u256::from(30_u32)).try_into().unwrap();
    let mask = U256::ONE.wrapping_shl((b + 1).wrapping_shl(3));
    let sign_mask = mask.wrapping_shr(1);
    let size_mask = mask - 1;
    let value = x & size_mask;
    Ok(InstructionFunctionOutput { cost: 5, result: [if (value & sign_mask) != 0 { !size_mask | value } else { value }], jump: 1 })
};
pub static LT: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [if a < b { U256::ONE } else { U256::ZERO }], jump: 1 });
pub static GT: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [if a > b { U256::ONE } else { U256::ZERO }], jump: 1 });
pub static SLT: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [match (a.is_neg(), b.is_neg()) {
    (true, false) => { U256::ONE },
    (false, true) => { U256::ZERO },
    _ => if a < b { U256::ONE } else { U256::ZERO },
}], jump: 1 });
pub static SGT: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [match (a.is_neg(), b.is_neg()) {
    (true, false) => { U256::ZERO },
    (false, true) => { U256::ONE },
    _ => if a > b { U256::ONE } else { U256::ZERO },
}], jump: 1 });
pub static EQ: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [if a == b { U256::ONE } else { U256::ZERO }], jump: 1 });
pub static ISZERO: InstructionFunction<1, 1> = |_, [a]| Ok(InstructionFunctionOutput { cost: 3, result: [if a == U256::ZERO { U256::ONE } else { U256::ZERO }], jump: 1 });
pub static AND: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [a & b], jump: 1 });
pub static OR: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [a | b], jump: 1 });
pub static XOR: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [a ^ b], jump: 1 });
pub static NOT: InstructionFunction<1, 1> = |_, [a]| Ok(InstructionFunctionOutput { cost: 3, result: [!a], jump: 1 });
pub static BYTE: InstructionFunction<2, 1> = |_, [i, x]| Ok(InstructionFunctionOutput { cost: 3, result: [if i > 31 { U256::ZERO } else { (x >> (8 * (31 - i))) & 0xFF }], jump: 1 });
pub static SHL: InstructionFunction<2, 1> = |_, [shift, value]| Ok(InstructionFunctionOutput { cost: 3, result: [match TryInto::<u8>::try_into(shift) {
    Ok(shift) => value.wrapping_shl(shift.into()),
    _ => U256::ZERO,
}], jump: 1 });
pub static SHR: InstructionFunction<2, 1> = |_, [shift, value]| Ok(InstructionFunctionOutput { cost: 3, result: [match TryInto::<u8>::try_into(shift) {
    Ok(shift) => value.wrapping_shr(shift.into()),
    _ => U256::ZERO,
}], jump: 1 });
pub static SAR: InstructionFunction<2, 1> = |_, [shift, value]| Ok(InstructionFunctionOutput { cost: 3, result: [match (TryInto::<u8>::try_into(shift), value.is_neg()) {
    (Ok(shift), false) => value.wrapping_shr(shift.into()),
    (Ok(shift), true) => { if shift == 0 { value } else { !(U256::ONE.wrapping_shl((255 - shift + 1).into()) - 1) | value.wrapping_shr(shift.into()) } },
    (Err(_), false) => U256::ZERO,
    (Err(_), true) => U256::MAX,
}], jump: 1 });
pub static KECCAK256: InstructionFunction<2, 1> = |context, [offset, size]| {
    let ReadWriteOperation { size, extension_cost, result, .. } = context.memory.load(offset, size)?;
    Ok(InstructionFunctionOutput { cost: 30 + 6 * (size + 31) / 32 + extension_cost, result: [result.keccak256()], jump: 1 })
};
pub static ADDRESS: InstructionFunction<0, 1> = |context, []| Ok(InstructionFunctionOutput { cost: 2, result: [context.transaction.contract_address().0], jump: 1 });
pub static BALANCE: InstructionFunction<1, 1> = |context, [address]| {
    let account = context.accounts.load(address.try_into()?);
    Ok(InstructionFunctionOutput { cost: if account.warm { 100 } else { 2600 }, result: [account.value.balance], jump: 1 })
};
pub static ORIGIN: InstructionFunction<0, 1> = |context, []| Ok(InstructionFunctionOutput { cost: 2, result: [context.transaction.from.0], jump: 1 });
pub static CALLER: InstructionFunction<0, 1> = |context, []| Ok(InstructionFunctionOutput { cost: 2, result: [context.caller.0], jump: 1 });
pub static CALLVALUE: InstructionFunction<0, 1> = |context, []| Ok(InstructionFunctionOutput { cost: 2, result: [context.transaction.value], jump: 1 });
pub static CALLDATALOAD: InstructionFunction<1, 1> = |context, [offset]| Ok(InstructionFunctionOutput { cost: 3, result: [
    match TryInto::<usize>::try_into(offset) {
        Ok(offset) => {
            let mut res = U256::ZERO;
            for i in 0..32_usize {
                res <<= 8;
                res |= u256::from(*context.transaction.data.get(offset + i).unwrap_or(&0_u8));
            }
            res
        },
        Err(_) => U256::ZERO,
    }
], jump: 1 });
pub static CALLDATASIZE: InstructionFunction<0, 1> = |context, []| Ok(InstructionFunctionOutput { cost: 2, result: [context.transaction.data.len().as_u256()], jump: 1 });
pub static CALLDATACOPY: InstructionFunction<3, 0> = |context, [dest_offset, offset, size]| {
    let (calldata_offset, calldata_size): (usize, usize) = (offset.try_into().unwrap(), size.try_into().unwrap()); // TODO (fguerin - 7/12/2024) Handle calldata out of bounds
    let value = &context.transaction.data[calldata_offset..min(context.transaction.data.len(), calldata_offset + calldata_size)];
    let ReadWriteOperation { size, extension_cost, .. } = context.memory.store(dest_offset, size, value.to_vec())?;
    Ok(InstructionFunctionOutput { cost: 3 + 3 * (size + 31) / 32 + extension_cost, result: [], jump: 1 })
};
pub static CODESIZE: InstructionFunction<0, 1> = |context, []| Ok(InstructionFunctionOutput { cost: 2, result: [context.accounts.load(context.transaction.contract_address()).value.code.len().as_u256()], jump: 1 });
pub static CODECOPY: InstructionFunction<3, 0> = |context, [dest_offset, offset, size]| {
    let (code_offset, code_size): (usize, usize) = (offset.try_into().unwrap(), size.try_into().unwrap()); // TODO (fguerin - 7/12/2024) Handle code out of bounds
    let contract_account = context.accounts.load(context.transaction.contract_address());
    let value = &contract_account.value.code[code_offset..min(contract_account.value.code.len(), code_offset + code_size)];
    let ReadWriteOperation { size, extension_cost, .. } = context.memory.store(dest_offset, size, value.to_vec())?;
    Ok(InstructionFunctionOutput { cost: 3 + 3 * (size + 31) / 32 + extension_cost, result: [], jump: 1 })
};
pub static GASPRICE: InstructionFunction<0, 1> = |_, []| todo!();
pub static EXTCODESIZE: InstructionFunction<1, 1> = |_, [_address]| todo!();
pub static EXTCODECOPY: InstructionFunction<4, 0> = |_, [_address, _dest_offset, _offset, _size]| todo!();
pub static RETURNDATASIZE: InstructionFunction<0, 1> = |_, []| todo!();
pub static RETURNDATACOPY: InstructionFunction<3, 0> = |_, [_dest_offset, _offset, _size]| todo!();
pub static EXTCODEHASH: InstructionFunction<1, 1> = |_, [_address]| todo!();
pub static BLOCKHASH: InstructionFunction<1, 1> = |_, [_block]| todo!();
pub static COINBASE: InstructionFunction<0, 1> = |_, []| todo!();
pub static TIMESTAMP: InstructionFunction<0, 1> = |_, []| todo!();
pub static NUMBER: InstructionFunction<0, 1> = |_, []| todo!();
pub static PREVRANDAO: InstructionFunction<0, 1> = |_, []| todo!();
pub static GASLIMIT: InstructionFunction<0, 1> = |_, []| todo!();
pub static CHAINID: InstructionFunction<0, 1> = |_, []| todo!();
pub static SELFBALANCE: InstructionFunction<0, 1> = |_, []| todo!();
pub static BASEFEE: InstructionFunction<0, 1> = |_, []| todo!();
pub static BLOBHASH: InstructionFunction<1, 1> = |_, [_index]| todo!();
pub static BLOBBASEFEE: InstructionFunction<0, 1> = |_, []| todo!();
pub static POP: InstructionFunction<1, 0> = |_, [_x]| Ok(InstructionFunctionOutput { cost: 2, result: [], jump: 1 });
pub static MLOAD: InstructionFunction<1, 1> = |context, [offset]| {
    let ReadWriteOperation { extension_cost, result, .. } = context.memory.load_word(offset)?;
    Ok(InstructionFunctionOutput { cost: 3 + extension_cost, result: [result], jump: 1 })
};
pub static MSTORE: InstructionFunction<2, 0> = |context, [offset, value]| {
    let ReadWriteOperation { extension_cost, .. } = context.memory.store_word(offset, value)?;
    Ok(InstructionFunctionOutput { cost: 3 + extension_cost, result: [], jump: 1 })
};
pub static MSTORE8: InstructionFunction<2, 0> = |context, [offset, value]| {
    let ReadWriteOperation { extension_cost, .. } = context.memory.store_byte(offset, value)?;
    Ok(InstructionFunctionOutput { cost: 3 + extension_cost, result: [], jump: 1 })
};
pub static SLOAD: InstructionFunction<1, 1> = |context, [key]| {
    let res = context.storage.load(key);
    Ok(InstructionFunctionOutput { cost: if res.warm { 100 } else { 2100 }, result: [res.value], jump: 1 })
};
// TODO (fguerin - 17/11/2024) Add gas refund
pub static SSTORE: InstructionFunction<2, 0> = |context, [key, value]| {
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
    Ok(InstructionFunctionOutput { cost: base_cost + if warm { 0 } else { 2100 }, result: [], jump: 1 })
};
pub static JUMP: InstructionFunction<1, 0> = |context, [counter]| {
    *context.pc = try_jump(&context.transaction.data, counter)?;
    Ok(InstructionFunctionOutput { cost: 8, result: [], jump: 0 })
};
pub static JUMPI: InstructionFunction<2, 0> = |context, [counter, b]| {
    if b == U256::ZERO {
        Ok(InstructionFunctionOutput { cost: 10, result: [], jump: 1 })
    } else {
        *context.pc = try_jump(&context.transaction.data, counter)?;
        Ok(InstructionFunctionOutput { cost: 10, result: [], jump: 0 })
    }
};
pub static PC: InstructionFunction<0, 1> = |context, []| Ok(InstructionFunctionOutput { cost: 2, result: [u256::from(TryInto::<u64>::try_into(*context.pc).unwrap())], jump: 1 });
pub static MSIZE: InstructionFunction<0, 1> = |context, []| Ok(InstructionFunctionOutput { cost: 2, result: [u256::from(TryInto::<u64>::try_into(context.memory.size()).unwrap())], jump: 1 });
pub static GAS: InstructionFunction<0, 1> = |context, []| Ok(InstructionFunctionOutput { cost: 2, result: [if *context.gas >= 2 { u256::from(TryInto::<u64>::try_into(*context.gas - 2).unwrap()) } else { U256::ZERO }], jump: 1 });
pub static JUMPDEST: InstructionFunction<0, 0> = |_, []| Ok(InstructionFunctionOutput { cost: 1, result: [], jump: 1 });
pub static TLOAD: InstructionFunction<1, 1> = |context, [key]| Ok(InstructionFunctionOutput { cost: 100, result: [context.transient.load(key)], jump: 1 });
pub static TSTORE: InstructionFunction<2, 0> = |context, [key, value]| {
    context.transient.store(key, value);
    Ok(InstructionFunctionOutput { cost: 100, result: [], jump: 1 })
};
pub static MCOPY: InstructionFunction<3, 0> = |context, [dest_offset, offset, size]| {
    let value = context.memory.load(offset, size)?;
    let ReadWriteOperation { size, extension_cost, .. } = context.memory.store(dest_offset, size, value.result)?;
    Ok(InstructionFunctionOutput { cost: 3 + 3 * size / 32 + extension_cost, result: [], jump: 1 })
};
pub static PUSH0: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 0));
pub static PUSH1: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 1));
pub static PUSH2: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 2));
pub static PUSH3: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 3));
pub static PUSH4: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 4));
pub static PUSH5: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 5));
pub static PUSH6: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 6));
pub static PUSH7: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 7));
pub static PUSH8: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 8));
pub static PUSH9: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 9));
pub static PUSH10: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 10));
pub static PUSH11: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 11));
pub static PUSH12: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 12));
pub static PUSH13: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 13));
pub static PUSH14: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 14));
pub static PUSH15: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 15));
pub static PUSH16: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 16));
pub static PUSH17: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 17));
pub static PUSH18: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 18));
pub static PUSH19: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 19));
pub static PUSH20: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 20));
pub static PUSH21: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 21));
pub static PUSH22: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 22));
pub static PUSH23: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 23));
pub static PUSH24: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 24));
pub static PUSH25: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 25));
pub static PUSH26: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 26));
pub static PUSH27: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 27));
pub static PUSH28: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 28));
pub static PUSH29: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 29));
pub static PUSH30: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 30));
pub static PUSH31: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 31));
pub static PUSH32: InstructionFunction<0, 1> = |context, []| Ok(push_n(*context.pc, &context.transaction.data, 32));
pub static DUP1: InstructionFunction<1, 2> = |_, input| Ok(dup_n::<1, 2>(input));
pub static DUP2: InstructionFunction<2, 3> = |_, input| Ok(dup_n::<2, 3>(input));
pub static DUP3: InstructionFunction<3, 4> = |_, input| Ok(dup_n::<3, 4>(input));
pub static DUP4: InstructionFunction<4, 5> = |_, input| Ok(dup_n::<4, 5>(input));
pub static DUP5: InstructionFunction<5, 6> = |_, input| Ok(dup_n::<5, 6>(input));
pub static DUP6: InstructionFunction<6, 7> = |_, input| Ok(dup_n::<6, 7>(input));
pub static DUP7: InstructionFunction<7, 8> = |_, input| Ok(dup_n::<7, 8>(input));
pub static DUP8: InstructionFunction<8, 9> = |_, input| Ok(dup_n::<8, 9>(input));
pub static DUP9: InstructionFunction<9, 10> = |_, input| Ok(dup_n::<9, 10>(input));
pub static DUP10: InstructionFunction<10, 11> = |_, input| Ok(dup_n::<10, 11>(input));
pub static DUP11: InstructionFunction<11, 12> = |_, input| Ok(dup_n::<11, 12>(input));
pub static DUP12: InstructionFunction<12, 13> = |_, input| Ok(dup_n::<12, 13>(input));
pub static DUP13: InstructionFunction<13, 14> = |_, input| Ok(dup_n::<13, 14>(input));
pub static DUP14: InstructionFunction<14, 15> = |_, input| Ok(dup_n::<14, 15>(input));
pub static DUP15: InstructionFunction<15, 16> = |_, input| Ok(dup_n::<15, 16>(input));
pub static DUP16: InstructionFunction<16, 17> = |_, input| Ok(dup_n::<16, 17>(input));
pub static SWAP1: InstructionFunction<2, 2> = |_, input| Ok(swap_n::<2>(input));
pub static SWAP2: InstructionFunction<3, 3> = |_, input| Ok(swap_n::<3>(input));
pub static SWAP3: InstructionFunction<4, 4> = |_, input| Ok(swap_n::<4>(input));
pub static SWAP4: InstructionFunction<5, 5> = |_, input| Ok(swap_n::<5>(input));
pub static SWAP5: InstructionFunction<6, 6> = |_, input| Ok(swap_n::<6>(input));
pub static SWAP6: InstructionFunction<7, 7> = |_, input| Ok(swap_n::<7>(input));
pub static SWAP7: InstructionFunction<8, 8> = |_, input| Ok(swap_n::<8>(input));
pub static SWAP8: InstructionFunction<9, 9> = |_, input| Ok(swap_n::<9>(input));
pub static SWAP9: InstructionFunction<10, 10> = |_, input| Ok(swap_n::<10>(input));
pub static SWAP10: InstructionFunction<11, 11> = |_, input| Ok(swap_n::<11>(input));
pub static SWAP11: InstructionFunction<12, 12> = |_, input| Ok(swap_n::<12>(input));
pub static SWAP12: InstructionFunction<13, 13> = |_, input| Ok(swap_n::<13>(input));
pub static SWAP13: InstructionFunction<14, 14> = |_, input| Ok(swap_n::<14>(input));
pub static SWAP14: InstructionFunction<15, 15> = |_, input| Ok(swap_n::<15>(input));
pub static SWAP15: InstructionFunction<16, 16> = |_, input| Ok(swap_n::<16>(input));
pub static SWAP16: InstructionFunction<17, 17> = |_, input| Ok(swap_n::<17>(input));
pub static LOG0: InstructionFunction<2, 0> = |_, [_offset, _size, _topics @ ..]| todo!();
pub static LOG1: InstructionFunction<3, 0> = |_, [_offset, _size, _topics @ ..]| todo!();
pub static LOG2: InstructionFunction<4, 0> = |_, [_offset, _size, _topics @ ..]| todo!();
pub static LOG3: InstructionFunction<5, 0> = |_, [_offset, _size, _topics @ ..]| todo!();
pub static LOG4: InstructionFunction<6, 0> = |_, [_offset, _size, _topics @ ..]| todo!();
pub static CREATE: InstructionFunction<3, 1> = |_, [_value, _offset, _size]| todo!();
pub static CALL: InstructionFunction<7, 1> = |_, [_gas, _address, _value, _args_offset, _args_size, _ret_offset, _ret_size]| todo!();
pub static CALLCODE: InstructionFunction<7, 1> = |_, [_gas, _address, _value, _args_offset, _args_size, _ret_offset, _ret_size]| todo!();
pub static RETURN: InstructionFunction<2, 0> = |context, [offset, size]| {
    let ReadWriteOperation { extension_cost, result, .. } = context.memory.load(offset, size)?;
    *context.stop_flag = true;
    *context.returndata = result;
    Ok(InstructionFunctionOutput { cost: extension_cost, result: [], jump: 0 })
};
pub static DELEGATECALL: InstructionFunction<6, 1> = |_, [_gas, _address, _args_offset, _args_size, _ret_offset, _ret_size]| todo!();
pub static CREATE2: InstructionFunction<4, 1> = |_, [_value, _offset, _size, _salt]| todo!();
pub static STATICCALL: InstructionFunction<6, 1> = |_, [_gas, _address, _args_offset, _args_size, _ret_offset, _ret_size]| todo!();
pub static REVERT: InstructionFunction<2, 0> = |context, [offset, size]| {
    let ReadWriteOperation { extension_cost, result, .. } = context.memory.load(offset, size)?;
    *context.stop_flag = true;
    *context.revert_flag = true;
    *context.returndata = result;
    Ok(InstructionFunctionOutput { cost: extension_cost, result: [], jump: 0 })
};
pub static INVALID: InstructionFunction<0, 0> = |context, []| {
    *context.stop_flag = true;
    *context.revert_flag = true;
    *context.returndata = vec![];
    Ok(InstructionFunctionOutput { cost: *context.gas, result: [], jump: 0 })
};
pub static SELFDESTRUCT: InstructionFunction<1, 0> = |_, [_address]| todo!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{storage::StorageValue, transaction::Address};

    use super::*;
    use ethnum::{uint,u256};

    #[test]
    fn stop() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert!(!*context.stop_flag);
        assert_eq!(STOP(&mut context, []), Ok(InstructionFunctionOutput { cost: 0, result: [], jump: 0 }));
        assert!(*context.stop_flag);
    }

    #[test]
    fn add() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(ADD(&mut context, [uint!("10"), uint!("6")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("16")], jump: 1 }));
        assert_eq!(ADD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("1")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn mul() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(MUL(&mut context, [uint!("10"), uint!("6")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("60")], jump: 1 }));
        assert_eq!(MUL(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")], jump: 1 }));
    }

    #[test]
    fn sub() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(SUB(&mut context, [uint!("10"), uint!("6")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("4")], jump: 1 }));
        assert_eq!(SUB(&mut context, [uint!("0"), uint!("1")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")], jump: 1 }));
    }

    #[test]
    fn div() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(DIV(&mut context, [uint!("10"), uint!("6")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("1")], jump: 1 }));
        assert_eq!(DIV(&mut context, [uint!("6"), uint!("0")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("0")], jump: 1 })); // dividing by zero returns zero by convention
    }

    #[test]
    fn sdiv() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(SDIV(&mut context, [uint!("10"), uint!("6")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("1")], jump: 1 }));
        assert_eq!(SDIV(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("2")], jump: 1 }));
        assert_eq!(SDIV(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE"), uint!("1")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")], jump: 1 }));
        assert_eq!(SDIV(&mut context, [uint!("6"), uint!("0")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("0")], jump: 1 })); // dividing by zero returns zero by convention
    }

    #[test]
    fn r#mod() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(MOD(&mut context, [uint!("10"), uint!("6")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("4")], jump: 1 }));
        assert_eq!(MOD(&mut context, [uint!("6"), uint!("0")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("0")], jump: 1 })); // modulo zero returns zero by convention
    }

    #[test]
    fn smod() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(SMOD(&mut context, [uint!("10"), uint!("6")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("4")], jump: 1 }));
        assert_eq!(SMOD(&mut context, [uint!("3"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("1")], jump: 1 }));
        assert_eq!(SMOD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF8"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFD")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")], jump: 1 }));
        assert_eq!(SMOD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFD"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")], jump: 1 }));
        assert_eq!(SMOD(&mut context, [uint!("3"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("1")], jump: 1 }));
        assert_eq!(SMOD(&mut context, [uint!("6"), uint!("0")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("0")], jump: 1 })); // modulo zero returns zero by convention
    }

    #[test]
    fn addmod() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &100, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(ADDMOD(&mut context, [uint!("10"), uint!("10"), uint!("8")]), Ok(InstructionFunctionOutput { cost: 8, result: [uint!("4")], jump: 1 }));
        assert_eq!(ADDMOD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("2"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 8, result: [uint!("1")], jump: 1 }));
        assert_eq!(ADDMOD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFD"), uint!("2"), uint!("3")]), Ok(InstructionFunctionOutput { cost: 8, result: [uint!("0")], jump: 1 }));
        assert_eq!(ADDMOD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("1"), uint!("10")]), Ok(InstructionFunctionOutput { cost: 8, result: [uint!("6")], jump: 1 }));
        assert_eq!(ADDMOD(&mut context, [uint!("4"), uint!("6"), uint!("0")]), Ok(InstructionFunctionOutput { cost: 8, result: [uint!("0")], jump: 1 })); // modulo zero returns zero by convention
    }

    #[test]
    fn mulmod() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &100, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(MULMOD(&mut context, [uint!("10"), uint!("10"), uint!("8")]), Ok(InstructionFunctionOutput { cost: 8, result: [uint!("4")], jump: 1 }));
        assert_eq!(MULMOD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("12")]), Ok(InstructionFunctionOutput { cost: 8, result: [uint!("9")], jump: 1 }));
        assert_eq!(MULMOD(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFD"), uint!("2"), uint!("3")]), Ok(InstructionFunctionOutput { cost: 8, result: [uint!("2")], jump: 1 }));
        assert_eq!(MULMOD(&mut context, [uint!("4"), uint!("6"), uint!("0")]), Ok(InstructionFunctionOutput { cost: 8, result: [uint!("0")], jump: 1 })); // modulo zero returns zero by convention
    }

    #[test]
    fn exp() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &1400, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(EXP(&mut context, [uint!("10"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 60, result: [uint!("100")], jump: 1 }));
        assert_eq!(EXP(&mut context, [uint!("2"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 60, result: [uint!("4")], jump: 1 }));
        assert_eq!(EXP(&mut context, [uint!("5"), uint!("0")]), Ok(InstructionFunctionOutput { cost: 10, result: [uint!("1")], jump: 1 }));
        assert_eq!(EXP(&mut context, [uint!("2"), uint!("10")]), Ok(InstructionFunctionOutput { cost: 60, result: [uint!("1024")], jump: 1 }));
        assert_eq!(EXP(&mut context, [uint!("2"), uint!("260")]), Ok(InstructionFunctionOutput { cost: 110, result: [uint!("0")], jump: 1 }));
        assert_eq!(EXP(&mut context, [uint!("0xFFFFFFFFFFFFFFFF"), uint!("3")]), Ok(InstructionFunctionOutput { cost: 60, result: [uint!("0xFFFFFFFFFFFFFFFD0000000000000002FFFFFFFFFFFFFFFF")], jump: 1 }));
        assert_eq!(EXP(&mut context, [uint!("3"), uint!("0xFFFFFFFFFFFFFFF0")]), Ok(InstructionFunctionOutput { cost: 410, result: [uint!("0xE9377A20E36295B65EA7F55D4A333F73CF25A1BE32FEBCF9702BDE500F57B8C1")], jump: 1 }));
        assert_eq!(EXP(&mut context, [uint!("5"), uint!("0xFFFFFFFFFFFFFFF0FFFFFF")]), Ok(InstructionFunctionOutput { cost: 560, result: [uint!("0x49E63006C06484CE7E18DB842AD1771FC1C83AA03B09227A2EB3765958CCCCCD")], jump: 1 }));
    }

    #[test]
    fn signextend() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &200, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(SIGNEXTEND(&mut context, [uint!("0"), uint!("0x41")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("0x41")], jump: 1 }));
        assert_eq!(SIGNEXTEND(&mut context, [uint!("0"), uint!("0xEF41")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("0x41")], jump: 1 }));
        assert_eq!(SIGNEXTEND(&mut context, [uint!("1"), uint!("0xEF41")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEF41")], jump: 1 }));
        assert_eq!(SIGNEXTEND(&mut context, [uint!("2"), uint!("0xEF41")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("0xEF41")], jump: 1 }));
        assert_eq!(SIGNEXTEND(&mut context, [uint!("30"), uint!("0xEF41")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("0xEF41")], jump: 1 }));
        assert_eq!(SIGNEXTEND(&mut context, [uint!("31"), uint!("0xEF41")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("0xEF41")], jump: 1 }));
        assert_eq!(SIGNEXTEND(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0xEF41")]), Ok(InstructionFunctionOutput { cost: 5, result: [uint!("0xEF41")], jump: 1 }));
    }

    #[test]
    fn lt() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(LT(&mut context, [uint!("9"), uint!("10")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(LT(&mut context, [uint!("10"), uint!("10")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn gt() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(GT(&mut context, [uint!("10"), uint!("9")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(GT(&mut context, [uint!("10"), uint!("10")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn eq() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(EQ(&mut context, [uint!("10"), uint!("10")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(EQ(&mut context, [uint!("10"), uint!("3")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn iszero() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &20, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(ISZERO(&mut context, [uint!("0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(ISZERO(&mut context, [uint!("3")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn slt() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(SLT(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(SLT(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(SLT(&mut context, [uint!("0"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(SLT(&mut context, [uint!("1"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(SLT(&mut context, [uint!("10"), uint!("10")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn sgt() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(SGT(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(SGT(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(SGT(&mut context, [uint!("0"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(SGT(&mut context, [uint!("1"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(SGT(&mut context, [uint!("10"), uint!("10")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn and() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(AND(&mut context, [uint!("0xFF"), uint!("0xFF")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xFF")], jump: 1 }));
        assert_eq!(AND(&mut context, [uint!("0"), uint!("0xFF")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(AND(&mut context, [uint!("0xF0"), uint!("0xFF")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xF0")], jump: 1 }));
    }

    #[test]
    fn or() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(OR(&mut context, [uint!("0xFF"), uint!("0xFF")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xFF")], jump: 1 }));
        assert_eq!(OR(&mut context, [uint!("0"), uint!("0xFF")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xFF")], jump: 1 }));
        assert_eq!(OR(&mut context, [uint!("0xF0"), uint!("0xFF")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xFF")], jump: 1 }));
    }

    #[test]
    fn xor() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(XOR(&mut context, [uint!("0xFF"), uint!("0xFF")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(XOR(&mut context, [uint!("0"), uint!("0xFF")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xFF")], jump: 1 }));
        assert_eq!(XOR(&mut context, [uint!("0xF0"), uint!("0xFF")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x0F")], jump: 1 }));
    }

    #[test]
    fn not() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(NOT(&mut context, [uint!("0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")], jump: 1 }));
        assert_eq!(NOT(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(NOT(&mut context, [uint!("0xF0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0F")], jump: 1 }));
    }

    #[test]
    fn byte() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(BYTE(&mut context, [uint!("16"), uint!("0x0112233445566778899AABBCCDDEEFF0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(BYTE(&mut context, [uint!("31"), uint!("0x0112233445566778899AABBCCDDEEFF0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xF0")], jump: 1 }));
        assert_eq!(BYTE(&mut context, [uint!("15"), uint!("0x0112233445566778899AABBCCDDEEFF0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(BYTE(&mut context, [uint!("32"), uint!("0x0112233445566778899AABBCCDDEEFF0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(BYTE(&mut context, [uint!("28"), uint!("0x0112233445566778899AABBCCDDEEFF0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xCD")], jump: 1 }));
        assert_eq!(BYTE(&mut context, [uint!("19"), uint!("0x0112233445566778899AABBCCDDEEFF0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x34")], jump: 1 }));
    }

    #[test]
    fn shl() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(SHL(&mut context, [uint!("1"), uint!("1")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2")], jump: 1 }));
        assert_eq!(SHL(&mut context, [uint!("4"), uint!("0xFF00000000000000000000000000000000000000000000000000000000000000")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xF000000000000000000000000000000000000000000000000000000000000000")], jump: 1 }));
    }

    #[test]
    fn shr() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(SHR(&mut context, [uint!("1"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(SHR(&mut context, [uint!("4"), uint!("0xFF")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x0F")], jump: 1 }));
    }

    #[test]
    fn sar() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(SAR(&mut context, [uint!("1"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1")], jump: 1 }));
        assert_eq!(SAR(&mut context, [uint!("4"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")], jump: 1 }));
        assert_eq!(SAR(&mut context, [uint!("600"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")], jump: 1 }));
        assert_eq!(SAR(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")], jump: 1 }));
        assert_eq!(SAR(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0x0FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0")], jump: 1 }));
        assert_eq!(SAR(&mut context, [uint!("0"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")], jump: 1 }));
        assert_eq!(SAR(&mut context, [uint!("4"), uint!("0xEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFAB00")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFAB0")], jump: 1 }));
    }

    #[test]
    fn keccak256_1() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory(vec![0xFF, 0xFF, 0xFF, 0xFF]), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(KECCAK256(&mut context, [uint!("0"), uint!("4")]), Ok(InstructionFunctionOutput { cost: 36, result: [uint!("0x29045A592007D0C246EF02C2223570DA9522D0CF0F73282C79A1BC8F0BB2C238")], jump: 1 }));
    }

    #[test]
    fn keccak256_2() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory(vec![0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(KECCAK256(&mut context, [uint!("4"), uint!("40")]), Ok(InstructionFunctionOutput { cost: 46, result: [uint!("0xDAA77426C30C02A43D9FBA4E841A6556C524D47030762EB14DC4AF897E605D9B")], jump: 1 }));
    }

    #[test]
    fn address_1() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(uint!("0x327E1362BF1CA14B1685B19BE97994D6EEBF546B")), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(ADDRESS(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("0x327E1362BF1CA14B1685B19BE97994D6EEBF546B")], jump: 1 }));
    }

    #[test]
    fn address_2() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(uint!("0x004EC07D2329997267EC62B4166639513386F32E")), nonce: 0x8E, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(ADDRESS(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("0x8D7BB25141FF9C4C77E9E208B6BF4D1D3CA684B0")], jump: 1 }));
    }

    #[test]
    fn address_3() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(uint!("0x6AC7EA33F8831EA9DCC53393AAA88B25A785DBF0")), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(ADDRESS(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("0xCD234A471B72BA2F1CCF0A70FCABA648A5EECD8D")], jump: 1 }));
    }

    #[test]
    fn address_4() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(uint!("0x6AC7EA33F8831EA9DCC53393AAA88B25A785DBF0")), nonce: 1, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(ADDRESS(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("0x343C43A37D37DFF08AE8C4A11544C718ABB4FCF8")], jump: 1 }));
    }

    #[test]
    fn address_5() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(uint!("0x6AC7EA33F8831EA9DCC53393AAA88B25A785DBF0")), nonce: 2, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(ADDRESS(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")], jump: 1 }));
    }

    #[test]
    fn address_6() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(ADDRESS(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")], jump: 1 }));
    }

    #[test]
    fn balance() {
        let mut initial_accounts: HashMap::<Address, Account> = Default::default();
        initial_accounts.insert(Address(uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")), Account { balance: uint!("125985"), code: vec![] });
        let mut context = InstructionContext { accounts: &mut Storage::new(initial_accounts), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(BALANCE(&mut context, [uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]), Ok(InstructionFunctionOutput { cost: 2600, result: [uint!("125985")], jump: 1 }));
        assert_eq!(BALANCE(&mut context, [uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]), Ok(InstructionFunctionOutput { cost: 100, result: [uint!("125985")], jump: 1 }));
        assert_eq!(BALANCE(&mut context, [uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D9")]), Ok(InstructionFunctionOutput { cost: 2600, result: [uint!("0")], jump: 1 }));
        assert_eq!(BALANCE(&mut context, [uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D9")]), Ok(InstructionFunctionOutput { cost: 100, result: [uint!("0")], jump: 1 }));
        assert_eq!(BALANCE(&mut context, [uint!("0x109BBFED6889322E016E0A02EE459D306FC19545D9")]), Err(Error::InvalidAddress));
    }

    #[test]
    fn origin() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")), nonce: 0, to: Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(ORIGIN(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")], jump: 1 }));
    }

    #[test]
    fn caller() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Address(uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")), transaction: &Transaction { data: Default::default(), from: Address(uint!("0x13275B5C2C17FCA86DB556FF2C19CBED48D8D229")), nonce: 0, to: Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(CALLER(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")], jump: 1 }));
    }

    #[test]
    fn callvalue() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: uint!("42") }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(CALLVALUE(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("42")], jump: 1 }));
    }

    #[test]
    fn calldataload() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF], from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: uint!("42") }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(CALLDATALOAD(&mut context, [uint!("0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF")], jump: 1 }));
        assert_eq!(CALLDATALOAD(&mut context, [uint!("31")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0xFF00000000000000000000000000000000000000000000000000000000000000")], jump: 1 }));
    }

    #[test]
    fn calldatasize() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: vec![0xFF, 0xFF], from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: uint!("42") }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(CALLDATASIZE(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("2")], jump: 1 }));
    }

    #[test]
    fn calldatacopy() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF], from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: uint!("42") }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(CALLDATACOPY(&mut context, [uint!("0"), uint!("0"), uint!("32")]), Ok(InstructionFunctionOutput { cost: 11, result: [], jump: 1 }));
        assert_eq!(context.memory.0, vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);

        assert_eq!(CALLDATACOPY(&mut context, [uint!("0"), uint!("31"), uint!("8")]), Ok(InstructionFunctionOutput { cost: 6, result: [], jump: 1 }));
        assert_eq!(context.memory.0, vec![0xFF, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn codesize() {
        let mut initial_accounts = HashMap::<Address, Account>::new();
        initial_accounts.insert(Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), Account { balance: uint!("4"), code: vec![1, 5, 4, 11, 42] });
        let mut context = InstructionContext { accounts: &mut Storage::new(initial_accounts), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), gas: 0, value: uint!("42") }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(CODESIZE(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("5")], jump: 1 }));
    }

    #[test]
    fn codesize_no_code() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), gas: 0, value: uint!("42") }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(CODESIZE(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn codecopy() {
        let mut initial_accounts = HashMap::<Address, Account>::new();
        initial_accounts.insert(Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), Account { balance: uint!("4"), code: vec![0x7D, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F] });
        let mut context = InstructionContext { accounts: &mut Storage::new(initial_accounts), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(uint!("0xF0490D46185BEC962CAC93120B52389748E99C0C")), gas: 0, value: uint!("42") }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(CODECOPY(&mut context, [uint!("0"), uint!("0"), uint!("32")]), Ok(InstructionFunctionOutput { cost: 11, result: [], jump: 1 }));
        assert_eq!(context.memory.0, vec![0x7D, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]);

        assert_eq!(CODECOPY(&mut context, [uint!("0"), uint!("31"), uint!("8")]), Ok(InstructionFunctionOutput { cost: 6, result: [], jump: 1 }));
        assert_eq!(context.memory.0, vec![0x7F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]);
    }

    #[test]
    fn pop() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(POP(&mut context, [uint!("42")]), Ok(InstructionFunctionOutput { cost: 2, result: [], jump: 1 }));
    }

    #[test]
    fn mload_no_memory_extension() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory(vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69]), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(MLOAD(&mut context, [uint!("0")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369")], jump: 1 }));
        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(context.memory.0, vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69]);

        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory(vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69]), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(MLOAD(&mut context, [uint!("2")]), Ok(InstructionFunctionOutput { cost: 6, result: [uint!("0xB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A023690000")], jump: 1 }));
        assert_eq!(context.memory.0.len(), 64);
        assert_eq!(context.memory.0, vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory(vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69]), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(MLOAD(&mut context, [uint!("30")]), Ok(InstructionFunctionOutput { cost: 6, result: [uint!("0x2369000000000000000000000000000000000000000000000000000000000000")], jump: 1 }));
        assert_eq!(context.memory.0.len(), 64);
        assert_eq!(context.memory.0, vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory(vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69]), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(MLOAD(&mut context, [uint!("500")]), Ok(InstructionFunctionOutput { cost: 51, result: [uint!("0")], jump: 1 }));
        assert_eq!(context.memory.0.len(), 544);
        assert_eq!(context.memory.0, vec![0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0x34, 0x52, 0x51, 0x03, 0xF6, 0x7C, 0xF6, 0xE9, 0x4D, 0xBD, 0xB8, 0xBE, 0x31, 0x25, 0xA5, 0xDE, 0x53, 0xA0, 0x23, 0x69, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn mstore() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(context.memory.0.len(), 0);
        assert_eq!(MSTORE(&mut context, [uint!("0"), uint!("0xFF")]), Ok(InstructionFunctionOutput { cost: 6, result: [], jump: 1 }));
        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(context.memory.0, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF]);
        assert_eq!(MSTORE(&mut context, [uint!("1"), uint!("0xFF")]), Ok(InstructionFunctionOutput { cost: 6, result: [], jump: 1 }));
        assert_eq!(context.memory.0.len(), 64);
        assert_eq!(context.memory.0, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(context.memory.0.len(), 0);
        assert_eq!(MSTORE(&mut context, [uint!("3"), uint!("0xFF")]), Ok(InstructionFunctionOutput { cost: 9, result: [], jump: 1 }));
        assert_eq!(context.memory.0.len(), 64);
        assert_eq!(context.memory.0, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(context.memory.0.len(), 0);
        assert_eq!(MSTORE(&mut context, [uint!("500"), uint!("0xABFF")]), Ok(InstructionFunctionOutput { cost: 54, result: [], jump: 1 }));
        assert_eq!(context.memory.0.len(), 544);
        assert_eq!(context.memory.0, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xAB, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn mstore8() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(context.memory.0.len(), 0);
        assert_eq!(MSTORE8(&mut context, [uint!("0"), uint!("0xFFAB")]), Ok(InstructionFunctionOutput { cost: 6, result: [], jump: 1 }));
        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(context.memory.0, vec![0xAB, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(MSTORE8(&mut context, [uint!("31"), uint!("0xFFAB")]), Ok(InstructionFunctionOutput { cost: 3, result: [], jump: 1 }));
        assert_eq!(context.memory.0.len(), 32);
        assert_eq!(context.memory.0, vec![0xAB, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xAB]);
    }

    #[test]
    fn sload() {
        let mut initial_storage = HashMap::<u256, u256>::new();
        initial_storage.insert(uint!("42"), uint!("0xAB"));
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(initial_storage), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(SLOAD(&mut context, [uint!("42")]), Ok(InstructionFunctionOutput { cost: 2100, result: [uint!("0xAB")], jump: 1 }));
        assert_eq!(SLOAD(&mut context, [uint!("42")]), Ok(InstructionFunctionOutput { cost: 100, result: [uint!("0xAB")], jump: 1 }));
    }

    #[test]
    fn sstore() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(SSTORE(&mut context, [uint!("0"), uint!("0xFFFF")]), Ok(InstructionFunctionOutput { cost: 22100, result: [], jump: 1 })); // clean storage - no previous value - cold slot
        assert_eq!(context.storage.0.get(&uint!("0")), Some(&StorageValue { original_value: uint!("0"), value: uint!("0xFFFF"), warm: true }));
        assert_eq!(SSTORE(&mut context, [uint!("0"), uint!("0xFFFF")]), Ok(InstructionFunctionOutput { cost: 100, result: [], jump: 1 })); // dirty storage - same value - warn slot
        assert_eq!(context.storage.0.get(&uint!("0")), Some(&StorageValue { original_value: uint!("0"), value: uint!("0xFFFF"), warm: true }));
        assert_eq!(SSTORE(&mut context, [uint!("0"), uint!("0xFFF0")]), Ok(InstructionFunctionOutput { cost: 100, result: [], jump: 1 })); // dirty storage - different value - warn slot
        assert_eq!(context.storage.0.get(&uint!("0")), Some(&StorageValue { original_value: uint!("0"), value: uint!("0xFFF0"), warm: true }));

        let mut initial_storage = HashMap::<u256, u256>::new();
        initial_storage.insert(uint!("1"), uint!("55"));
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(initial_storage), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(SSTORE(&mut context, [uint!("1"), uint!("10")]), Ok(InstructionFunctionOutput { cost: 5000, result: [], jump: 1 })); // clean storage - different value - cold slot
        assert_eq!(context.storage.0.get(&uint!("1")), Some(&StorageValue { original_value: uint!("55"), value: uint!("10"), warm: true }));

        let mut initial_storage = HashMap::<u256, u256>::new();
        initial_storage.insert(uint!("1"), uint!("55"));
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(initial_storage), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(SSTORE(&mut context, [uint!("1"), uint!("55")]), Ok(InstructionFunctionOutput { cost: 2200, result: [], jump: 1 })); // clean storage - same value - cold slot
        assert_eq!(context.storage.0.get(&uint!("1")), Some(&StorageValue { original_value: uint!("55"), value: uint!("55"), warm: true }));
    }

    #[test]
    fn jump() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: vec![0_u8, 0_u8, 0x5B, 0_u8], from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(*context.pc, 0);
        assert_eq!(JUMP(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFF")]), Err(Error::InvalidJumpDest)); // not a usize
        assert_eq!(*context.pc, 0);
        assert_eq!(JUMP(&mut context, [uint!("0xFFFF")]), Err(Error::InvalidJumpDest)); // not in range
        assert_eq!(*context.pc, 0);
        assert_eq!(JUMP(&mut context, [uint!("1")]), Err(Error::InvalidJumpDest)); // not a valid destination
        assert_eq!(*context.pc, 0);
        assert_eq!(JUMP(&mut context, [uint!("2")]), Ok(InstructionFunctionOutput { cost: 8, result: [], jump: 0 }));
        assert_eq!(*context.pc, 2);
    }

    #[test]
    fn jumpi() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: vec![0_u8, 0_u8, 0x5B, 0_u8], from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(*context.pc, 0);
        assert_eq!(JUMPI(&mut context, [uint!("2"), uint!("0")]), Ok(InstructionFunctionOutput { cost: 10, result: [], jump: 1 })); // jump condition is false
        assert_eq!(*context.pc, 0);
        assert_eq!(JUMPI(&mut context, [uint!("0xFFFFFFFFFFFFFFFFFFFF"), uint!("1")]), Err(Error::InvalidJumpDest)); // not a usize
        assert_eq!(*context.pc, 0);
        assert_eq!(JUMPI(&mut context, [uint!("0xFFFF"), uint!("1")]), Err(Error::InvalidJumpDest)); // not in range
        assert_eq!(*context.pc, 0);
        assert_eq!(JUMPI(&mut context, [uint!("1"), uint!("1")]), Err(Error::InvalidJumpDest)); // not a valid destination
        assert_eq!(*context.pc, 0);
        assert_eq!(JUMPI(&mut context, [uint!("2"), uint!("1")]), Ok(InstructionFunctionOutput { cost: 10, result: [], jump: 0 }));
        assert_eq!(*context.pc, 2);
    }

    #[test]
    fn pc() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 30, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(PC(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("30")], jump: 1 }));
    }

    #[test]
    fn msize() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory(vec![0; 64]), pc: &mut 30, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(MSIZE(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("64")], jump: 1 }));
    }


    #[test]
    fn gas() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &5, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(GAS(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("3")], jump: 1 }));

        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &3, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(GAS(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("1")], jump: 1 }));

        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &1, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(GAS(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("0")], jump: 1 }));

        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &0, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(GAS(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn jumpdest() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &5, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(JUMPDEST(&mut context, []), Ok(InstructionFunctionOutput { cost: 1, result: [], jump: 1 }));

    }

    #[test]
    fn tload() {
        let mut initial_transient = HashMap::<u256, u256>::new();
        initial_transient.insert(uint!("42"), uint!("0xAB"));
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient(initial_transient), returndata: &mut Default::default() };

        assert_eq!(TLOAD(&mut context, [uint!("42")]), Ok(InstructionFunctionOutput { cost: 100, result: [uint!("0xAB")], jump: 1 }));
        assert_eq!(TLOAD(&mut context, [uint!("45")]), Ok(InstructionFunctionOutput { cost: 100, result: [uint!("0")], jump: 1 }));
    }

    #[test]
    fn tstore() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(TSTORE(&mut context, [uint!("1"), uint!("55")]), Ok(InstructionFunctionOutput { cost: 100, result: [], jump: 1 }));
        assert_eq!(context.transient.0.get(&uint!("1")), Some(&uint!("55")));
    }

    #[test]
    fn mcopy() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory(vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31]), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(MCOPY(&mut context, [uint!("0"), uint!("32"), uint!("32")]), Ok(InstructionFunctionOutput { cost: 6, result: [], jump: 1 }));
        assert_eq!(context.memory.0, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31]);

        assert_eq!(MCOPY(&mut context, [uint!("4"), uint!("8"), uint!("16")]), Ok(InstructionFunctionOutput { cost: 4, result: [], jump: 1 }));
        assert_eq!(context.memory.0, vec![0, 1, 2, 3, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31]);
    }

    #[test]
    fn push_n() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: vec![1, 0x59, 0x36, 0xD2, 0xA1, 0xC5, 0xC3, 0xAF, 0x2E, 0xEB, 0x31, 0x55, 0xB9, 0x6B, 0x30, 0x01, 0xA3, 0x47, 0xD6, 0xFE, 0x75, 0xE5, 0x18, 0x59, 0xEB, 0xBA, 0x81, 0x55, 0x13, 0x1A, 0x8E, 0x05, 0x56], from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(PUSH0(&mut context, []), Ok(InstructionFunctionOutput { cost: 2, result: [uint!("0")], jump: 1 }));
        assert_eq!(PUSH1(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x59")], jump: 2 }));
        assert_eq!(PUSH2(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936")], jump: 3 }));
        assert_eq!(PUSH3(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2")], jump: 4 }));
        assert_eq!(PUSH4(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1")], jump: 5 }));
        assert_eq!(PUSH5(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5")], jump: 6 }));
        assert_eq!(PUSH6(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3")], jump: 7 }));
        assert_eq!(PUSH7(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF")], jump: 8 }));
        assert_eq!(PUSH8(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2E")], jump: 9 }));
        assert_eq!(PUSH9(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB")], jump: 10 }));
        assert_eq!(PUSH10(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB31")], jump: 11 }));
        assert_eq!(PUSH11(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155")], jump: 12 }));
        assert_eq!(PUSH12(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B9")], jump: 13 }));
        assert_eq!(PUSH13(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B")], jump: 14 }));
        assert_eq!(PUSH14(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B30")], jump: 15 }));
        assert_eq!(PUSH15(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001")], jump: 16 }));
        assert_eq!(PUSH16(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A3")], jump: 17 }));
        assert_eq!(PUSH17(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347")], jump: 18 }));
        assert_eq!(PUSH18(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6")], jump: 19 }));
        assert_eq!(PUSH19(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE")], jump: 20 }));
        assert_eq!(PUSH20(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75")], jump: 21 }));
        assert_eq!(PUSH21(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E5")], jump: 22 }));
        assert_eq!(PUSH22(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E518")], jump: 23 }));
        assert_eq!(PUSH23(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859")], jump: 24 }));
        assert_eq!(PUSH24(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EB")], jump: 25 }));
        assert_eq!(PUSH25(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA")], jump: 26 }));
        assert_eq!(PUSH26(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA81")], jump: 27 }));
        assert_eq!(PUSH27(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155")], jump: 28 }));
        assert_eq!(PUSH28(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA815513")], jump: 29 }));
        assert_eq!(PUSH29(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155131A")], jump: 30 }));
        assert_eq!(PUSH30(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155131A8E")], jump: 31 }));
        assert_eq!(PUSH31(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155131A8E05")], jump: 32 }));
        assert_eq!(PUSH32(&mut context, []), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155131A8E0556")], jump: 33 }));
    }

    #[test]
    fn dup_n() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(DUP1(&mut context, [uint!("1")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("1")], jump: 1 }));
        assert_eq!(DUP2(&mut context, [uint!("0"), uint!("1")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("1")], jump: 1 }));
        assert_eq!(DUP3(&mut context, [uint!("0"), uint!("0"), uint!("1")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }));
        assert_eq!(
            DUP4(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            DUP5(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            DUP6(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            DUP7(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            DUP8(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            DUP9(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            DUP10(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            DUP11(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            DUP12(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            DUP13(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            DUP14(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            DUP15(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            DUP16(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
    }

    #[test]
    fn swap_n() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert_eq!(SWAP1(&mut context, [uint!("1"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("1")], jump: 1 }));
        assert_eq!(SWAP2(&mut context, [uint!("1"), uint!("0"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("1")], jump: 1 }));
        assert_eq!(SWAP3(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }));
        assert_eq!(
            SWAP4(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            SWAP5(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            SWAP6(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            SWAP7(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            SWAP8(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            SWAP9(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            SWAP10(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            SWAP11(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            SWAP12(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            SWAP13(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            SWAP14(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            SWAP15(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
        assert_eq!(
            SWAP16(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
            Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
        );
    }

    #[test]
    fn r#return() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory(vec![0xFF, 1]), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert!(!*context.stop_flag);
        assert_eq!(*context.returndata, vec![]);
        assert_eq!(RETURN(&mut context, [uint!("0"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 0, result: [], jump: 0 }));
        assert!(*context.stop_flag);
        assert_eq!(*context.returndata, vec![0xFF, 1]);
    }

    #[test]
    fn revert() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory(vec![0xFF, 1]), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert!(!*context.stop_flag);
        assert!(!*context.revert_flag);
        assert_eq!(*context.returndata, vec![]);
        assert_eq!(REVERT(&mut context, [uint!("0"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 0, result: [], jump: 0 }));
        assert!(*context.stop_flag);
        assert!(*context.revert_flag);
        assert_eq!(*context.returndata, vec![0xFF, 1]);
    }

    #[test]
    fn invalid() {
        let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory(vec![0xFF, 1]), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

        assert!(!*context.stop_flag);
        assert!(!*context.revert_flag);
        assert_eq!(*context.returndata, vec![]);
        assert_eq!(INVALID(&mut context, []), Ok(InstructionFunctionOutput { cost: 50, result: [], jump: 0 }));
        assert!(*context.stop_flag);
        assert!(*context.revert_flag);
        assert_eq!(*context.returndata, vec![]);
    }
}
