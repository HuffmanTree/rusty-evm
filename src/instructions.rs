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

pub static STOP: InstructionFunction<0, 0> = |context, []| { *context.stop_flag = true; Ok(InstructionFunctionOutput { cost: 0, result: [], jump: 0 }) }; // DONE
pub static ADD: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [a.wrapping_add(b)], jump: 1 }); // DONE
pub static MUL: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 5, result: [a.wrapping_mul(b)], jump: 1 }); // DONE
pub static SUB: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [a.wrapping_sub(b)], jump: 1 }); // DONE
pub static DIV: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_div(b) }], jump: 1 }); // DONE
pub static SDIV: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_signed_div(b) }], jump: 1 }); // DONE
pub static MOD: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_rem(b) }], jump: 1 }); // DONE
pub static SMOD: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 5, result: [if b == 0 { U256::ZERO } else { a.wrapping_signed_rem(b) }], jump: 1 }); // DONE
pub static ADDMOD: InstructionFunction<3, 1> = |_, [a, b, n]| Ok(InstructionFunctionOutput { cost: 8, result: [if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_add(b.wrapping_rem(n)).wrapping_rem(n) }], jump: 1 }); // DONE
pub static MULMOD: InstructionFunction<3, 1> = |_, [a, b, n]| Ok(InstructionFunctionOutput { cost: 8, result: [if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_mul(b.wrapping_rem(n)).wrapping_rem(n) }], jump: 1 }); // DONE
pub static EXP: InstructionFunction<2, 1> = |_, [a, e]| Ok(InstructionFunctionOutput { cost: 10 + 50 * e.needed_size_in_bytes(), result: [a.wrapping_big_pow(e)], jump: 1 }); // DONE
pub static SIGNEXTEND: InstructionFunction<2, 1> = |_, [b, x]| { // DONE
    let b: u32 = min(b, u256::from(30_u32)).try_into().unwrap();
    let mask = U256::ONE.wrapping_shl((b + 1).wrapping_shl(3));
    let sign_mask = mask.wrapping_shr(1);
    let size_mask = mask - 1;
    let value = x & size_mask;
    Ok(InstructionFunctionOutput { cost: 5, result: [if (value & sign_mask) != 0 { !size_mask | value } else { value }], jump: 1 })
};
pub static LT: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [if a < b { U256::ONE } else { U256::ZERO }], jump: 1 }); // DONE
pub static GT: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [if a > b { U256::ONE } else { U256::ZERO }], jump: 1 }); // DONE
pub static SLT: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [match (a.is_neg(), b.is_neg()) { // DONE
    (true, false) => { U256::ONE },
    (false, true) => { U256::ZERO },
    _ => if a < b { U256::ONE } else { U256::ZERO },
}], jump: 1 });
pub static SGT: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [match (a.is_neg(), b.is_neg()) { // DONE
    (true, false) => { U256::ZERO },
    (false, true) => { U256::ONE },
    _ => if a > b { U256::ONE } else { U256::ZERO },
}], jump: 1 });
pub static EQ: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [if a == b { U256::ONE } else { U256::ZERO }], jump: 1 }); // DONE
pub static ISZERO: InstructionFunction<1, 1> = |_, [a]| Ok(InstructionFunctionOutput { cost: 3, result: [if a == U256::ZERO { U256::ONE } else { U256::ZERO }], jump: 1 }); // DONE
pub static AND: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [a & b], jump: 1 }); // DONE
pub static OR: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [a | b], jump: 1 }); // DONE
pub static XOR: InstructionFunction<2, 1> = |_, [a, b]| Ok(InstructionFunctionOutput { cost: 3, result: [a ^ b], jump: 1 }); // DONE
pub static NOT: InstructionFunction<1, 1> = |_, [a]| Ok(InstructionFunctionOutput { cost: 3, result: [!a], jump: 1 }); // DONE
pub static BYTE: InstructionFunction<2, 1> = |_, [i, x]| Ok(InstructionFunctionOutput { cost: 3, result: [if i > 31 { U256::ZERO } else { (x >> (8 * (31 - i))) & 0xFF }], jump: 1 }); // DONE
pub static SHL: InstructionFunction<2, 1> = |_, [shift, value]| Ok(InstructionFunctionOutput { cost: 3, result: [match TryInto::<u8>::try_into(shift) { // DONE
    Ok(shift) => value.wrapping_shl(shift.into()),
    _ => U256::ZERO,
}], jump: 1 });
pub static SHR: InstructionFunction<2, 1> = |_, [shift, value]| Ok(InstructionFunctionOutput { cost: 3, result: [match TryInto::<u8>::try_into(shift) { // DONE
    Ok(shift) => value.wrapping_shr(shift.into()),
    _ => U256::ZERO,
}], jump: 1 });
pub static SAR: InstructionFunction<2, 1> = |_, [shift, value]| Ok(InstructionFunctionOutput { cost: 3, result: [match (TryInto::<u8>::try_into(shift), value.is_neg()) { // DONE
    (Ok(shift), false) => value.wrapping_shr(shift.into()),
    (Ok(shift), true) => { if shift == 0 { value } else { !(U256::ONE.wrapping_shl((255 - shift + 1).into()) - 1) | value.wrapping_shr(shift.into()) } },
    (Err(_), false) => U256::ZERO,
    (Err(_), true) => U256::MAX,
}], jump: 1 });
pub static KECCAK256: InstructionFunction<2, 1> = |context, [offset, size]| { // DONE
    let ReadWriteOperation { size, extension_cost, result, .. } = context.memory.load(offset, size)?;
    Ok(InstructionFunctionOutput { cost: 30 + 6 * (size + 31) / 32 + extension_cost, result: [result.keccak256()], jump: 1 })
};
pub static ADDRESS: InstructionFunction<0, 1> = |context, []| Ok(InstructionFunctionOutput { cost: 2, result: [context.transaction.contract_address().0], jump: 1 }); // DONE
pub static BALANCE: InstructionFunction<1, 1> = |context, [address]| { // DONE
    let account = context.accounts.load(address.try_into()?);
    Ok(InstructionFunctionOutput { cost: if account.warm { 100 } else { 2600 }, result: [account.value.balance], jump: 1 })
};
pub static ORIGIN: InstructionFunction<0, 1> = |context, []| Ok(InstructionFunctionOutput { cost: 2, result: [context.transaction.from.0], jump: 1 }); // DONE
pub static CALLER: InstructionFunction<0, 1> = |context, []| Ok(InstructionFunctionOutput { cost: 2, result: [context.caller.0], jump: 1 }); // DONE
pub static CALLVALUE: InstructionFunction<0, 1> = |context, []| Ok(InstructionFunctionOutput { cost: 2, result: [context.transaction.value], jump: 1 }); // DONE
pub static CALLDATALOAD: InstructionFunction<1, 1> = |context, [offset]| Ok(InstructionFunctionOutput { cost: 3, result: [ // DONE
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
pub static CALLDATASIZE: InstructionFunction<0, 1> = |context, []| Ok(InstructionFunctionOutput { cost: 2, result: [context.transaction.data.len().as_u256()], jump: 1 }); // DONE
pub static CALLDATACOPY: InstructionFunction<3, 0> = |context, [dest_offset, offset, size]| { // DONE
    let (calldata_offset, calldata_size): (usize, usize) = (offset.try_into().unwrap(), size.try_into().unwrap()); // TODO (fguerin - 7/12/2024) Handle calldata out of bounds
    let value = &context.transaction.data[calldata_offset..min(context.transaction.data.len(), calldata_offset + calldata_size)];
    let ReadWriteOperation { size, extension_cost, .. } = context.memory.store(dest_offset, size, value.to_vec())?;
    Ok(InstructionFunctionOutput { cost: 3 + 3 * (size + 31) / 32 + extension_cost, result: [], jump: 1 })
};
pub static CODESIZE: InstructionFunction<0, 1> = |context, []| Ok(InstructionFunctionOutput { cost: 2, result: [context.accounts.load(context.transaction.contract_address()).value.code.len().as_u256()], jump: 1 }); // DONE
pub static CODECOPY: InstructionFunction<3, 0> = |context, [dest_offset, offset, size]| { // DONE
    let (code_offset, code_size): (usize, usize) = (offset.try_into().unwrap(), size.try_into().unwrap()); // TODO (fguerin - 7/12/2024) Handle code out of bounds
    let contract_account = context.accounts.load(context.transaction.contract_address());
    let value = &contract_account.value.code[code_offset..min(contract_account.value.code.len(), code_offset + code_size)];
    let ReadWriteOperation { size, extension_cost, .. } = context.memory.store(dest_offset, size, value.to_vec())?;
    Ok(InstructionFunctionOutput { cost: 3 + 3 * (size + 31) / 32 + extension_cost, result: [], jump: 1 })
};
pub static GASPRICE: InstructionFunction<0, 1> = |_, []| todo!(); // DONE
pub static EXTCODESIZE: InstructionFunction<1, 1> = |_, [_address]| todo!(); // DONE
pub static EXTCODECOPY: InstructionFunction<4, 0> = |_, [_address, _dest_offset, _offset, _size]| todo!(); // DONE
pub static RETURNDATASIZE: InstructionFunction<0, 1> = |_, []| todo!(); // DONE
pub static RETURNDATACOPY: InstructionFunction<3, 0> = |_, [_dest_offset, _offset, _size]| todo!(); // DONE
pub static EXTCODEHASH: InstructionFunction<1, 1> = |_, [_address]| todo!(); // TODO
pub static BLOCKHASH: InstructionFunction<1, 1> = |_, [_block]| todo!(); // TODO
pub static COINBASE: InstructionFunction<0, 1> = |_, []| todo!(); // DONE
pub static TIMESTAMP: InstructionFunction<0, 1> = |_, []| todo!(); // DONE
pub static NUMBER: InstructionFunction<0, 1> = |_, []| todo!(); // DONE
pub static PREVRANDAO: InstructionFunction<0, 1> = |_, []| todo!(); // DONE
pub static GASLIMIT: InstructionFunction<0, 1> = |_, []| todo!(); // DONE
pub static CHAINID: InstructionFunction<0, 1> = |_, []| todo!(); // DONE
pub static SELFBALANCE: InstructionFunction<0, 1> = |_, []| todo!(); // DONE
pub static BASEFEE: InstructionFunction<0, 1> = |_, []| todo!(); // TODO
pub static BLOBHASH: InstructionFunction<1, 1> = |_, [_index]| todo!(); // TODO
pub static BLOBBASEFEE: InstructionFunction<0, 1> = |_, []| todo!(); // TODO
pub static POP: InstructionFunction<1, 0> = |_, [_x]| Ok(InstructionFunctionOutput { cost: 2, result: [], jump: 1 }); // DONE
pub static MLOAD: InstructionFunction<1, 1> = |context, [offset]| { // DONE
    let ReadWriteOperation { extension_cost, result, .. } = context.memory.load_word(offset)?;
    Ok(InstructionFunctionOutput { cost: 3 + extension_cost, result: [result], jump: 1 })
};
pub static MSTORE: InstructionFunction<2, 0> = |context, [offset, value]| { // DONE
    let ReadWriteOperation { extension_cost, .. } = context.memory.store_word(offset, value)?;
    Ok(InstructionFunctionOutput { cost: 3 + extension_cost, result: [], jump: 1 })
};
pub static MSTORE8: InstructionFunction<2, 0> = |context, [offset, value]| { // DONE
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

//     #[test]
//     fn dup_n() {
//         let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

//         assert_eq!(DUP1(&mut context, [uint!("1")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("1")], jump: 1 }));
//         assert_eq!(DUP2(&mut context, [uint!("0"), uint!("1")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("1")], jump: 1 }));
//         assert_eq!(DUP3(&mut context, [uint!("0"), uint!("0"), uint!("1")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }));
//         assert_eq!(
//             DUP4(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             DUP5(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             DUP6(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             DUP7(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             DUP8(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             DUP9(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             DUP10(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             DUP11(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             DUP12(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             DUP13(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             DUP14(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             DUP15(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             DUP16(&mut context, [uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//     }

//     #[test]
//     fn swap_n() {
//         let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory::new(), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

//         assert_eq!(SWAP1(&mut context, [uint!("1"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("1")], jump: 1 }));
//         assert_eq!(SWAP2(&mut context, [uint!("1"), uint!("0"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("1")], jump: 1 }));
//         assert_eq!(SWAP3(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }));
//         assert_eq!(
//             SWAP4(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             SWAP5(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             SWAP6(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             SWAP7(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             SWAP8(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             SWAP9(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             SWAP10(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             SWAP11(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             SWAP12(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             SWAP13(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             SWAP14(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             SWAP15(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//         assert_eq!(
//             SWAP16(&mut context, [uint!("1"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("2")]),
//             Ok(InstructionFunctionOutput { cost: 3, result: [uint!("2"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("0"), uint!("1")], jump: 1 }),
//         );
//     }

//     #[test]
//     fn r#return() {
//         let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory(vec![0xFF, 1]), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

//         assert!(!*context.stop_flag);
//         assert_eq!(*context.returndata, vec![]);
//         assert_eq!(RETURN(&mut context, [uint!("0"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 0, result: [], jump: 0 }));
//         assert!(*context.stop_flag);
//         assert_eq!(*context.returndata, vec![0xFF, 1]);
//     }

//     #[test]
//     fn revert() {
//         let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory(vec![0xFF, 1]), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

//         assert!(!*context.stop_flag);
//         assert!(!*context.revert_flag);
//         assert_eq!(*context.returndata, vec![]);
//         assert_eq!(REVERT(&mut context, [uint!("0"), uint!("2")]), Ok(InstructionFunctionOutput { cost: 0, result: [], jump: 0 }));
//         assert!(*context.stop_flag);
//         assert!(*context.revert_flag);
//         assert_eq!(*context.returndata, vec![0xFF, 1]);
//     }

//     #[test]
//     fn invalid() {
//         let mut context = InstructionContext { accounts: &mut Default::default(), caller: &Default::default(), transaction: &Transaction { data: Default::default(), from: Address(U256::ZERO), nonce: 0, to: Address(U256::ZERO), gas: 0, value: U256::ZERO }, gas: &50, memory: &mut Memory(vec![0xFF, 1]), pc: &mut 0, stop_flag: &mut false, revert_flag: &mut false, storage: &mut Storage::new(Default::default()), transient: &mut Transient::new(), returndata: &mut Default::default() };

//         assert!(!*context.stop_flag);
//         assert!(!*context.revert_flag);
//         assert_eq!(*context.returndata, vec![]);
//         assert_eq!(INVALID(&mut context, []), Ok(InstructionFunctionOutput { cost: 50, result: [], jump: 0 }));
//         assert!(*context.stop_flag);
//         assert!(*context.revert_flag);
//         assert_eq!(*context.returndata, vec![]);
//     }
// }
