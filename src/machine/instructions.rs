use ethnum::{u256, AsU256, U256};
use crate::blockchain::WorldState;
use crate::blockchain::errors::Error;
use crate::machine::context::{CallContext, Log, TransactionContext};
use crate::machine::memory::ReadWriteOperation;
use crate::utils::{Hash, IsNeg, NeededSizeInBytes, WrappingBigPow, WrappingSignedDiv, WrappingSignedRem};

#[derive(Debug, Eq, PartialEq)]
pub struct InstructionOutput {
    pub cost: usize,
    pub jump: usize,
}

pub type InstructionResult = Result<InstructionOutput, Error>;

pub struct Instructions {}

impl Instructions {
    fn pop_or_fail<const N: usize>(cctx: &mut CallContext) -> Result<[u256; N], Error> {
        let mut res = [U256::ZERO; N];
        for i in 0..N {
            res[i] = if let Some(x) = cctx.stack.pop() { x } else { return Err(Error::EmptyStack) }
        }
        Ok(res)
    }

    fn push_rev_or_fail<const N: usize>(cctx: &mut CallContext, values: [u256; N]) -> Result<(), Error> {
        for i in (0..N).rev() {
            cctx.stack.push(values[i])?;
        }
        Ok(())
    }

    fn jump_or_fail(cctx: &mut CallContext, counter: u256) -> Result<(), Error> {
        let counter: usize = match counter.try_into() {
            Ok(x) => x,
            _ => return Err(Error::InvalidJumpDest),
        };
        match cctx.contract.code.get(counter) {
            Some(x) => if *x == 0x5B { cctx.pc = counter; Ok(()) } else { Err(Error::InvalidJumpDest) },
            _ => Err(Error::InvalidJumpDest),
        }
    }

    pub fn stop(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        cctx.stop = true;
        Ok(InstructionOutput { cost: 0, jump: 0 })
    }

    pub fn add(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [a.wrapping_add(b)])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn mul(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [a.wrapping_mul(b)])?;
        Ok(InstructionOutput { cost: 5, jump: 1 })
    }

    pub fn sub(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [a.wrapping_sub(b)])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn div(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [if b == 0 { U256::ZERO } else { a.wrapping_div(b) }])?;
        Ok(InstructionOutput { cost: 5, jump: 1 })
    }

    pub fn sdiv(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [if b == 0 { U256::ZERO } else { a.wrapping_signed_div(b) }])?;
        Ok(InstructionOutput { cost: 5, jump: 1 })
    }

    pub fn r#mod(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [if b == 0 { U256::ZERO } else { a.wrapping_rem(b) }])?;
        Ok(InstructionOutput { cost: 5, jump: 1 })
    }

    pub fn smod(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [if b == 0 { U256::ZERO } else { a.wrapping_signed_rem(b) }])?;
        Ok(InstructionOutput { cost: 5, jump: 1 })
    }

    pub fn addmod(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b, n] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_add(b.wrapping_rem(n)).wrapping_rem(n) }])?;
        Ok(InstructionOutput { cost: 8, jump: 1 })
    }

    pub fn mulmod(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b, n] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_mul(b.wrapping_rem(n)).wrapping_rem(n) }])?;
        Ok(InstructionOutput { cost: 8, jump: 1 })
    }

    pub fn exp(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, e] = Instructions::pop_or_fail(cctx)?;
        let exponent_byte_size = e.needed_size_in_bytes();
        Instructions::push_rev_or_fail(cctx, [a.wrapping_big_pow(e)])?;
        Ok(InstructionOutput { cost: 10 + 50 * exponent_byte_size, jump: 1 })
    }

    pub fn signextend(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [b, x] = Instructions::pop_or_fail(cctx)?;
        let b: u32 = std::cmp::min(b, u256::from(30u32)).try_into().unwrap();
        let mask = U256::ONE.wrapping_shl((b + 1).wrapping_shl(3));
        let sign_mask = mask.wrapping_shr(1);
        let size_mask = mask - 1;
        let value = x & size_mask;
        Instructions::push_rev_or_fail(cctx, [if (value & sign_mask) != 0 { !size_mask | value } else { value }])?;
        Ok(InstructionOutput { cost: 5, jump: 1 })
    }

    pub fn lt(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [if a < b { U256::ONE } else { U256::ZERO }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn gt(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [if a > b { U256::ONE } else { U256::ZERO }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn slt(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [match (a.is_neg(), b.is_neg()) {
            (true, false) => { U256::ONE },
            (false, true) => { U256::ZERO },
            _ => if a < b { U256::ONE } else { U256::ZERO },
        }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn sgt(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [match (a.is_neg(), b.is_neg()) {
            (true, false) => { U256::ZERO },
            (false, true) => { U256::ONE },
            _ => if a > b { U256::ONE } else { U256::ZERO },
        }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn eq(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [if a == b { U256::ONE } else { U256::ZERO }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn iszero(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [if a == U256::ZERO { U256::ONE } else { U256::ZERO }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn and(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [a & b])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn or(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [a | b])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn xor(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [a ^ b])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn not(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [!a])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn byte(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [i, x] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [if i > 31 { U256::ZERO } else { (x >> (8 * (31 - i))) & 0xFF }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn shl(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [shift, value] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [match TryInto::<u8>::try_into(shift) {
            Ok(shift) => value.wrapping_shl(shift.into()),
            _ => U256::ZERO,
        }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn shr(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [shift, value] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [match TryInto::<u8>::try_into(shift) {
            Ok(shift) => value.wrapping_shr(shift.into()),
            _ => U256::ZERO,
        }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn sar(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [shift, value] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [match (TryInto::<u8>::try_into(shift), value.is_neg()) {
            (Ok(shift), false) => value.wrapping_shr(shift.into()),
            (Ok(shift), true) => { if shift == 0 { value } else { !(U256::ONE.wrapping_shl((255 - shift + 1).into()) - 1) | value.wrapping_shr(shift.into()) } },
            (Err(_), false) => U256::ZERO,
            (Err(_), true) => U256::MAX,
        }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn keccak256(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [offset, size] = Instructions::pop_or_fail(cctx)?;
        let ReadWriteOperation { size, extension_cost, result, .. } = cctx.memory.load(offset, size)?;
        Instructions::push_rev_or_fail(cctx, [result.keccak256()])?;
        Ok(InstructionOutput { cost: 30 + 6 * ((size + 31) >> 5) + extension_cost, jump: 1 })
    }

    pub fn address(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [cctx.contract.address.0])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn balance(s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [address] = Instructions::pop_or_fail(cctx)?;
        let account = s.accounts.load(address.try_into()?);
        Instructions::push_rev_or_fail(cctx, [account.value.balance])?;
        Ok(InstructionOutput { cost: if account.warm { 100 } else { 2600 }, jump: 1 })
    }

    pub fn origin(_s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [tctx.tx.from.0])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn caller(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [cctx.contract.caller.0])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn callvalue(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [cctx.contract.value])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn calldataload(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [offset] = Instructions::pop_or_fail(cctx)?;
        Instructions::push_rev_or_fail(cctx, [match TryInto::<usize>::try_into(offset) {
            Ok(offset) => {
                let mut res = U256::ZERO;
                for i in 0..32usize {
                    res <<= 8;
                    res |= u256::from(*cctx.contract.input.get(offset + i).unwrap_or(&0u8));
                }
                res
            },
            Err(_) => U256::ZERO,
        }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn calldatasize(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [cctx.contract.input.len().as_u256()])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn calldatacopy(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [dest_offset, offset, size] = Instructions::pop_or_fail(cctx)?;
        let (calldata_offset, calldata_size): (usize, usize) = (offset.try_into().unwrap(), size.try_into().unwrap()); // TODO (fguerin - 11/12/2024) Handle calldata out of bounds
        let value = &cctx.contract.input[calldata_offset..std::cmp::min(cctx.contract.input.len(), calldata_offset + calldata_size)];
        let ReadWriteOperation { size, extension_cost, .. } = cctx.memory.store(dest_offset, size, value.to_vec())?;
        Ok(InstructionOutput { cost: 3 + 3 * ((size + 31) >> 5) + extension_cost, jump: 1 })
    }

    pub fn codesize(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [cctx.contract.code.len().as_u256()])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn codecopy(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [dest_offset, offset, size] = Instructions::pop_or_fail(cctx)?;
        let (code_offset, code_size): (usize, usize) = (offset.try_into().unwrap(), size.try_into().unwrap()); // TODO (fguerin - 11/12/2024) Handle code out of bounds
        let value = &cctx.contract.code[code_offset..std::cmp::min(cctx.contract.code.len(), code_offset + code_size)];
        let ReadWriteOperation { size, extension_cost, .. } = cctx.memory.store(dest_offset, size, value.to_vec())?;
        Ok(InstructionOutput { cost: 3 + 3 * ((size + 31) >> 5) + extension_cost, jump: 1 })
    }

    pub fn gasprice(_s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [tctx.tx.gas_price.as_u256()])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn extcodesize(s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [address] = Instructions::pop_or_fail(cctx)?;
        let account = s.accounts.load(address.try_into()?);
        Instructions::push_rev_or_fail(cctx, [account.value.code.len().as_u256()])?;
        Ok(InstructionOutput { cost: if account.warm { 100 } else { 2600 }, jump: 1 })
    }

    pub fn extcodecopy(s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [address, dest_offset, offset, size] = Instructions::pop_or_fail(cctx)?;
        let account = s.accounts.load(address.try_into()?);
        let (code_offset, code_size): (usize, usize) = (offset.try_into().unwrap(), size.try_into().unwrap()); // TODO (fguerin - 13/12/2024) Handle code out of bounds
        let value = &account.value.code[code_offset..std::cmp::min(account.value.code.len(), code_offset + code_size)];
        let ReadWriteOperation { size, extension_cost, .. } = cctx.memory.store(dest_offset, size, value.to_vec())?;
        Ok(InstructionOutput { cost: 3 * ((size + 31) >> 5) + extension_cost + if account.warm { 100 } else { 2600 }, jump: 1 })
    }

    pub fn returndatasize(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [cctx.returndata.len().as_u256()])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn returndatacopy(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [dest_offset, offset, size] = Instructions::pop_or_fail(cctx)?;
        let (returndata_offset, returndata_size): (usize, usize) = (offset.try_into().unwrap(), size.try_into().unwrap()); // TODO (fguerin - 13/12/2024) Handle returndata out of bounds
        let value = &cctx.returndata[returndata_offset..std::cmp::min(cctx.returndata.len(), returndata_offset + returndata_size)];
        let ReadWriteOperation { size, extension_cost, .. } = cctx.memory.store(dest_offset, size, value.to_vec())?;
        Ok(InstructionOutput { cost: 3 + 3 * ((size + 31) >> 5) + extension_cost, jump: 1 })
    }

    pub fn extcodehash(s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        // TODO (fguerin - 22/02/2025) Implement other subtleties
        let [address] = Instructions::pop_or_fail(cctx)?;
        let account = s.accounts.load(address.try_into()?);
        Instructions::push_rev_or_fail(cctx, [account.value.code.keccak256()])?;
        Ok(InstructionOutput { cost: if account.warm { 100 } else { 2600 }, jump: 1 })
    }

    pub fn blockhash(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    pub fn coinbase(_s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [tctx.block.miner.0])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn timestamp(_s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [tctx.block.time])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn number(_s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [tctx.block.number])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn prevrandao(_s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [tctx.block.difficulty])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn gaslimit(_s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [tctx.block.gas_limit])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn chainid(s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [s.chain_id])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn selfbalance(s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        // TODO (fguerin - 13/12/2024) Test whether it should warm the storage
        let account = s.accounts.load(cctx.contract.address);
        Instructions::push_rev_or_fail(cctx, [account.value.balance])?;
        Ok(InstructionOutput { cost: 5, jump: 1 })
    }

    pub fn basefee(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    pub fn blobhash(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    pub fn blobbasefee(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    pub fn pop(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::pop_or_fail::<1>(cctx)?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn mload(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [offset] = Instructions::pop_or_fail(cctx)?;
        let ReadWriteOperation { extension_cost, result, .. } = cctx.memory.load_word(offset)?;
        Instructions::push_rev_or_fail(cctx, [result])?;
        Ok(InstructionOutput { cost: 3 + extension_cost, jump: 1 })
    }

    pub fn mstore(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [offset, value] = Instructions::pop_or_fail(cctx)?;
        let ReadWriteOperation { extension_cost, .. } = cctx.memory.store_word(offset, value)?;
        Ok(InstructionOutput { cost: 3 + extension_cost, jump: 1 })
    }

    pub fn mstore8(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [offset, value] = Instructions::pop_or_fail(cctx)?;
        let ReadWriteOperation { extension_cost, .. } = cctx.memory.store_byte(offset, value)?;
        Ok(InstructionOutput { cost: 3 + extension_cost, jump: 1 })
    }

    pub fn sload(s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [key] = Instructions::pop_or_fail(cctx)?;
        let storage = s.storage.entry(cctx.contract.address).or_default();
        let result = storage.load(key);
        Instructions::push_rev_or_fail(cctx, [result.value])?;
        Ok(InstructionOutput { cost: if result.warm { 100 } else { 2100 }, jump: 1 })
    }

    pub fn sstore(s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        // TODO (fguerin - 14/12/2024) Add gas refund
        let [key, value] = Instructions::pop_or_fail(cctx)?;
        let storage = s.storage.entry(cctx.contract.address).or_default();
        let (current_value, original_value, warm) = match storage.store(key, value) {
            Some(v) => (v.value, v.original_value, v.warm),
            None => (U256::ZERO, U256::ZERO, false),
        };
        let base_cost: usize =
            if value == current_value { 100 }     // the value does not change
        else if current_value == original_value { // the storage slot is clean ...
            if original_value == 0 { 20000 }      // ... and has not explicit value
            else { 2900 }                         // ... and has an explicit value
        }
        else { 100 };                             // the value changes and the storage slot is dirty
        Ok(InstructionOutput { cost: base_cost + if warm { 0 } else { 2100 }, jump: 1 })
    }

    pub fn jump(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [counter] = Instructions::pop_or_fail(cctx)?;
        Instructions::jump_or_fail(cctx, counter)?;
        Ok(InstructionOutput { cost: 8, jump: 0 })
    }

    pub fn jumpi(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [counter, b] = Instructions::pop_or_fail(cctx)?;
        let jump = match b {
            U256::ZERO => 1,
            _ => { Instructions::jump_or_fail(cctx, counter)?; 0 },
        };
        Ok(InstructionOutput { cost: 10, jump })
    }

    pub fn pc(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [cctx.pc.as_u256()])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn msize(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [cctx.memory.size().as_u256()])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn gas(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Instructions::push_rev_or_fail(cctx, [if cctx.contract.gas < 2 { U256::ZERO } else { (cctx.contract.gas - 2).as_u256() }])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    pub fn jumpdest(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        Ok(InstructionOutput { cost: 1, jump: 1 })
    }

    pub fn tload(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [key] = Instructions::pop_or_fail(cctx)?;
        let value = cctx.transient.load(key);
        Instructions::push_rev_or_fail(cctx, [value])?;
        Ok(InstructionOutput { cost: 100, jump: 1 })
    }

    pub fn tstore(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [key, value] = Instructions::pop_or_fail(cctx)?;
        cctx.transient.store(key, value);
        Ok(InstructionOutput { cost: 100, jump: 1 })
    }

    pub fn mcopy(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [dest_offset, offset, size] = Instructions::pop_or_fail(cctx)?;
        let ReadWriteOperation { result, extension_cost: read_extension_cost, .. } = cctx.memory.load(offset, size)?;
        let ReadWriteOperation { size, extension_cost: write_extension_cost, .. } = cctx.memory.store(dest_offset, size, result)?;
        Ok(InstructionOutput { cost: 3 + 3 * ((size + 31) >> 5) + read_extension_cost + write_extension_cost, jump: 1 })
    }

    pub fn push<const N: usize>(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let mut res = U256::ZERO;
        for i in 0..N {
            res <<= 8;
            res |= u256::from(*cctx.contract.code.get(cctx.pc + i + 1).unwrap_or(&0u8));
        };
        Instructions::push_rev_or_fail(cctx, [res])?;
        Ok(InstructionOutput { cost: if N == 0 { 2 } else { 3 }, jump: N + 1 })
    }

    pub fn dup<const N: usize>(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let values = Instructions::pop_or_fail::<N>(cctx)?;
        Instructions::push_rev_or_fail(cctx, values)?;
        Instructions::push_rev_or_fail(cctx, [values[N - 1]])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn swap<const N: usize>(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let mut values = Instructions::pop_or_fail::<N>(cctx)?;
        values.swap(0, N - 1);
        Instructions::push_rev_or_fail(cctx, values)?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    pub fn log<const N: usize>(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [offset, size] = Instructions::pop_or_fail(cctx)?;
        let topics = Instructions::pop_or_fail::<N>(cctx)?;
        let ReadWriteOperation { result: data, extension_cost, size, .. } = cctx.memory.load(offset, size)?;
        cctx.contract.logs.push(Log {
            data,
            topics: [
                topics.get(0).cloned(),
                topics.get(1).cloned(),
                topics.get(2).cloned(),
                topics.get(3).cloned(),
            ],
        });
        Ok(InstructionOutput { cost: 375 * (N + 1) + (size << 3) + extension_cost, jump: 1 })
    }

    pub fn create(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    pub fn call(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    pub fn callcode(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    pub fn r#return(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        cctx.stop = true;
        let [offset, size] = Instructions::pop_or_fail(cctx)?;
        let ReadWriteOperation { result: data, extension_cost, .. } = cctx.memory.load(offset, size)?;
        cctx.r#return = data;
        Ok(InstructionOutput { cost: extension_cost, jump: 0 })
    }

    pub fn delegatecall(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    pub fn create2(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    pub fn staticcall(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    pub fn revert(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        cctx.stop = true;
        cctx.revert = true;
        let [offset, size] = Instructions::pop_or_fail(cctx)?;
        let ReadWriteOperation { result: data, extension_cost, .. } = cctx.memory.load(offset, size)?;
        cctx.r#return = data;
        Ok(InstructionOutput { cost: extension_cost, jump: 0 })
    }

    pub fn invalid(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        cctx.stop = true;
        cctx.revert = true;
        Ok(InstructionOutput { cost: cctx.contract.gas, jump: 0 })
    }

    pub fn selfdestruct(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }
}

#[cfg(test)]
mod tests {
    use ethnum::uint;
    use super::*;
    use crate::blockchain::primitives::{Account, Address, Block, Transaction};
    use crate::blockchain::storage::StorageValue;
    use crate::machine::context::CallContextContract;
    use crate::machine::memory::Memory;
    use crate::machine::stack::Stack;

    impl CallContext {
        fn with_stop(&mut self, stop: bool) {
            self.stop = stop;
        }

        fn with_pc(&mut self, pc: usize) {
            self.pc = pc;
        }

        fn with_stack<T: Into::<u256> + Copy>(&mut self, stack: Vec<T>) {
            self.stack = Stack::new();
            for i in (0..stack.len()).rev() { self.stack.push(stack[i].into()).unwrap(); }
        }

        fn with_memory(&mut self, memory: &str) {
            self.memory = Memory(hex::decode(memory).unwrap());
        }


        fn with_transient<T: Into::<u256> + Copy>(&mut self, transient: &[(T, T)]) {
            self.transient = Default::default();
            for (key, value) in transient {
                self.transient.0.insert(Into::<u256>::into(*key), Into::<u256>::into(*value));
            }
        }

        fn with_contract(&mut self, contract: CallContextContract) {
            self.contract = contract;
        }

        fn with_returndata(&mut self, memory: &str) {
            self.returndata = hex::decode(memory).unwrap();
        }
    }

    impl TransactionContext {
        fn with_transaction(&mut self, tx: Transaction) {
            self.tx = tx;
        }

        fn with_block(&mut self, block: Block) {
            self.block = block;
        }
    }

    impl WorldState {
        fn with_chain_id<T: Into::<u256>>(&mut self, chain_id: T) {
            self.chain_id = chain_id.into();
        }

        fn with_accounts(&mut self, accounts: &[(Address, Account)]) {
            self.accounts = Default::default();
            for (address, account) in accounts {
                self.accounts.0.insert(*address, StorageValue {
                    original_value: account.clone(),
                    value: account.clone(),
                    warm: false,
                });
            }
        }

        fn with_storage<T: Into::<u256> + Copy>(&mut self, storage: &[(Address, &[(T, T)])]) {
            self.storage = Default::default();
            for (address, store) in storage {
                self.storage.insert(*address, Default::default());
                let s = self.storage.get_mut(address).unwrap();
                for (key, value) in *store {
                    s.0.insert(Into::<u256>::into(*key), StorageValue {
                        original_value: Into::<u256>::into(*value),
                        value: Into::<u256>::into(*value),
                        warm: false,
                    });
                }
            }
        }
    }

    #[test]
    fn stop() {
        let cctx = &mut CallContext::default();

        cctx.with_stop(false);
        assert_eq!(Instructions::stop(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 0, jump: 0 }));
        assert!(cctx.stop);
    }

    #[test]
    fn add() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 6]);
        assert_eq!(Instructions::add(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [16]);

        cctx.with_stack(vec![U256::MAX, U256::ONE]);
        assert_eq!(Instructions::add(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn mul() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 6]);
        assert_eq!(Instructions::mul(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [60]);

        cctx.with_stack(vec![U256::MAX, uint!("2")]);
        assert_eq!(Instructions::mul(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [U256::MAX - 1]);
    }

    #[test]
    fn sub() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 6]);
        assert_eq!(Instructions::sub(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [4]);

        cctx.with_stack(vec![0u8, 1]);
        assert_eq!(Instructions::sub(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [U256::MAX]);
    }

    
    #[test]
    fn div() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 6]);
        assert_eq!(Instructions::div(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![6u8, 0]);
        assert_eq!(Instructions::div(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]); // dividing by zero returns zero by convention
    }

    #[test]
    fn sdiv() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 6]);
        assert_eq!(Instructions::sdiv(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![U256::MAX - 1, U256::MAX]);
        assert_eq!(Instructions::sdiv(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2]);

        cctx.with_stack(vec![U256::MAX - 1, U256::ONE]);
        assert_eq!(Instructions::sdiv(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [U256::MAX - 1]);

        cctx.with_stack(vec![6u8, 0]);
        assert_eq!(Instructions::sdiv(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]); // dividing by zero returns zero by convention
    }

    #[test]
    fn r#mod() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 6]);
        assert_eq!(Instructions::r#mod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [4]);

        cctx.with_stack(vec![10u8, 3]);
        assert_eq!(Instructions::r#mod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![6u8, 0]);
        assert_eq!(Instructions::r#mod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]); // modulo zero returns zero by convention
    }

    #[test]
    fn smod() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 6]);
        assert_eq!(Instructions::smod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [4]);

        cctx.with_stack(vec![3u8, 2]);
        assert_eq!(Instructions::smod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![U256::MAX - 7, U256::MAX - 2]);
        assert_eq!(Instructions::smod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [U256::MAX - 1]);

        cctx.with_stack(vec![U256::MAX - 2, uint!("2")]);
        assert_eq!(Instructions::smod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [U256::MAX]);

        cctx.with_stack(vec![uint!("3"), U256::MAX - 1]);
        assert_eq!(Instructions::smod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![6u8, 0]);
        assert_eq!(Instructions::smod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]); // modulo zero returns zero by convention
    }
   
    #[test]
    fn addmod() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 10, 8]);
        assert_eq!(Instructions::addmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [4]);

        cctx.with_stack(vec![U256::MAX, uint!("2"), uint!("2")]);
        assert_eq!(Instructions::addmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![U256::MAX - 2, uint!("2"), uint!("3")]);
        assert_eq!(Instructions::addmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![U256::MAX, uint!("1"), uint!("10")]);
        assert_eq!(Instructions::addmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [6]);

        cctx.with_stack(vec![4u8, 6, 0]);
        assert_eq!(Instructions::addmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]); // modulo zero returns zero by convention
    }
    
    #[test]
    fn mulmod() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 10, 8]);
        assert_eq!(Instructions::mulmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [4]);

        cctx.with_stack(vec![U256::MAX, U256::MAX, uint!("12")]);
        assert_eq!(Instructions::mulmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [9]);

        cctx.with_stack(vec![U256::MAX - 2, uint!("2"), uint!("3")]);
        assert_eq!(Instructions::mulmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2]);

        cctx.with_stack(vec![4u8, 6, 0]);
        assert_eq!(Instructions::mulmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);
    }


    #[test]
    fn exp() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 2]);
        assert_eq!(Instructions::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 60, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [100]);

        cctx.with_stack(vec![2u8, 2]);
        assert_eq!(Instructions::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 60, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [4]);

        cctx.with_stack(vec![5u8, 0]);
        assert_eq!(Instructions::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 10, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![2u8, 10]);
        assert_eq!(Instructions::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 60, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1024]);

        cctx.with_stack(vec![2u16, 260]);
        assert_eq!(Instructions::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 110, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("0xFFFFFFFFFFFFFFFF"), uint!("3")]);
        assert_eq!(Instructions::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 60, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xFFFFFFFFFFFFFFFD0000000000000002FFFFFFFFFFFFFFFF")]);

        cctx.with_stack(vec![uint!("3"), uint!("0xFFFFFFFFFFFFFFF0")]);
        assert_eq!(Instructions::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 410, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xE9377A20E36295B65EA7F55D4A333F73CF25A1BE32FEBCF9702BDE500F57B8C1")]);

        cctx.with_stack(vec![uint!("5"), uint!("0xFFFFFFFFFFFFFFF0FFFFFF")]);
        assert_eq!(Instructions::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 560, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x49E63006C06484CE7E18DB842AD1771FC1C83AA03B09227A2EB3765958CCCCCD")]);
    }

    #[test]
    fn signextend() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![0u8, 0x41]);
        assert_eq!(Instructions::signextend(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0x41]);

        cctx.with_stack(vec![0u16, 0xEF41]);
        assert_eq!(Instructions::signextend(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0x41]);

        cctx.with_stack(vec![1u16, 0xEF41]);
        assert_eq!(Instructions::signextend(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEF41")]);

        cctx.with_stack(vec![2u16, 0xEF41]);
        assert_eq!(Instructions::signextend(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0xEF41]);

        cctx.with_stack(vec![30u16, 0xEF41]);
        assert_eq!(Instructions::signextend(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0xEF41]);

        cctx.with_stack(vec![31u16, 0xEF41]);
        assert_eq!(Instructions::signextend(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0xEF41]);

        cctx.with_stack(vec![uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0xEF41")]);
        assert_eq!(Instructions::signextend(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0xEF41]);
    }


    #[test]
    fn lt() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![9u8, 10]);
        assert_eq!(Instructions::lt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![10u8, 10]);
        assert_eq!(Instructions::lt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn gt() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 9]);
        assert_eq!(Instructions::gt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![10u8, 10]);
        assert_eq!(Instructions::gt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn eq() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 10]);
        assert_eq!(Instructions::eq(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![10u8, 3]);
        assert_eq!(Instructions::eq(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn iszero() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![0u8]);
        assert_eq!(Instructions::iszero(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![3u8]);
        assert_eq!(Instructions::iszero(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn slt() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![U256::MAX, U256::ONE]);
        assert_eq!(Instructions::slt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![U256::MAX, U256::MAX - 1]);
        assert_eq!(Instructions::slt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![U256::ZERO, U256::MAX]);
        assert_eq!(Instructions::slt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![1u8, 2]);
        assert_eq!(Instructions::slt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![10u8, 10]);
        assert_eq!(Instructions::slt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn sgt() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![U256::MAX, U256::ZERO]);
        assert_eq!(Instructions::sgt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![U256::MAX, U256::MAX - 1]);
        assert_eq!(Instructions::sgt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![U256::ZERO, U256::MAX]);
        assert_eq!(Instructions::sgt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![1u8, 2]);
        assert_eq!(Instructions::sgt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![10u8, 10]);
        assert_eq!(Instructions::sgt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn and() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![0xFFu8, 0xFF]);
        assert_eq!(Instructions::and(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0xFF]);

        cctx.with_stack(vec![0u8, 0xFF]);
        assert_eq!(Instructions::and(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![0xF0u8, 0xFF]);
        assert_eq!(Instructions::and(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0xF0]);
    }

    #[test]
    fn or() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![0xFFu8, 0xFF]);
        assert_eq!(Instructions::or(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0xFF]);

        cctx.with_stack(vec![0u8, 0xFF]);
        assert_eq!(Instructions::or(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0xFF]);

        cctx.with_stack(vec![0xF0u8, 0xFF]);
        assert_eq!(Instructions::or(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0xFF]);
    }

    #[test]
    fn xor() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![0xFFu8, 0xFF]);
        assert_eq!(Instructions::xor(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![0u8, 0xFF]);
        assert_eq!(Instructions::xor(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0xFF]);

        cctx.with_stack(vec![0xF0u8, 0xFF]);
        assert_eq!(Instructions::xor(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0x0F]);
    }

    #[test]
    fn not() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![0u8]);
        assert_eq!(Instructions::not(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [U256::MAX]);

        cctx.with_stack(vec![U256::MAX]);
        assert_eq!(Instructions::not(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![0xF0u8]);
        assert_eq!(Instructions::not(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0F")]);
    }

    #[test]
    fn byte() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![uint!("16"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Instructions::byte(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![uint!("31"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Instructions::byte(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0xF0]);

        cctx.with_stack(vec![uint!("15"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Instructions::byte(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("32"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Instructions::byte(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("28"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Instructions::byte(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0xCD]);

        cctx.with_stack(vec![uint!("19"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Instructions::byte(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0x34]);
    }

    #[test]
    fn shl() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![1u8, 1]);
        assert_eq!(Instructions::shl(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2]);

        cctx.with_stack(vec![uint!("4"), uint!("0xFF00000000000000000000000000000000000000000000000000000000000000")]);
        assert_eq!(Instructions::shl(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xF000000000000000000000000000000000000000000000000000000000000000")]);
    }

    #[test]
    fn shr() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![1u8, 2]);
        assert_eq!(Instructions::shr(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![4u8, 0xFFu8]);
        assert_eq!(Instructions::shr(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0x0F]);
    }

    #[test]
    fn sar() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![1u8, 2]);
        assert_eq!(Instructions::sar(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![uint!("4"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);
        assert_eq!(Instructions::sar(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [U256::MAX]);

        cctx.with_stack(vec![uint!("600"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);
        assert_eq!(Instructions::sar(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [U256::MAX]);

        cctx.with_stack(vec![U256::MAX, uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);
        assert_eq!(Instructions::sar(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [U256::MAX]);

        cctx.with_stack(vec![U256::MAX, uint!("0x0FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);
        assert_eq!(Instructions::sar(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("0"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);
        assert_eq!(Instructions::sar(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);

        cctx.with_stack(vec![uint!("4"), uint!("0xEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFAB00")]);
        assert_eq!(Instructions::sar(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFAB0")]);
    }

    #[test]
    fn keccak256() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![0u8, 4]);
        cctx.with_memory("FFFFFFFF00000000000000000000000000000000000000000000000000000000");
        assert_eq!(Instructions::keccak256(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 36, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x29045A592007D0C246EF02C2223570DA9522D0CF0F73282C79A1BC8F0BB2C238")]);

        cctx.with_stack(vec![4u8, 40]);
        cctx.with_memory("FFFFFFFF00000000000000000000000000000000000000000000000000000000");
        assert_eq!(Instructions::keccak256(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 45, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xDAA77426C30C02A43D9FBA4E841A6556C524D47030762EB14DC4AF897E605D9B")]);
    }

    #[test]
    fn address() {
        let cctx = &mut CallContext::default();

        cctx.with_contract(CallContextContract {
            address: Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")),
            caller: Address(U256::ZERO),
            code: vec![],
            gas: 0,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });
        assert_eq!(Instructions::address(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")]);
    }

    #[test]
    fn balance() {
        let state = &mut WorldState::default();
        let cctx = &mut CallContext::default();

        state.with_accounts(&[(Address(uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")), Account { balance: uint!("125985"), code: vec![] })]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]);
        assert_eq!(Instructions::balance(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2600, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [125985]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]);
        assert_eq!(Instructions::balance(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [125985]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D9")]);
        assert_eq!(Instructions::balance(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2600, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D9")]);
        assert_eq!(Instructions::balance(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("0x109BBFED6889322E016E0A02EE459D306FC19545D9")]);
        assert_eq!(Instructions::balance(state, &TransactionContext::default(), cctx), Err(Error::InvalidAddress));
    }

    #[test]
    fn origin() {
        let cctx = &mut CallContext::default();
        let tctx = &mut TransactionContext::default();

        tctx.with_transaction(Transaction {
            data: vec![],
            from: Address(uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")),
            gas: 0,
            gas_price: 0,
            nonce: 0,
            to: Address::default(),
            value: U256::ZERO,
        });

        assert_eq!(Instructions::origin(&mut WorldState::default(), &tctx, cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]);
    }

    #[test]
    fn caller() {
        let cctx = &mut CallContext::default();

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")),
            code: vec![],
            gas: 0,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });
        assert_eq!(Instructions::caller(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")]);
    }

    #[test]
    fn callvalue() {
        let cctx = &mut CallContext::default();

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: vec![],
            gas: 0,
            input: vec![],
            logs: vec![],
            value: uint!("42"),
        });
        assert_eq!(Instructions::callvalue(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [42]);
    }

    #[test]
    fn calldataload() {
        let cctx = &mut CallContext::default();

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: vec![],
            gas: 0,
            input: hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap(),
            logs: vec![],
            value: U256::ZERO,
        });

        cctx.with_stack(vec![0u8]);
        assert_eq!(Instructions::calldataload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [U256::MAX]);

        cctx.with_stack(vec![31u8]);
        assert_eq!(Instructions::calldataload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xFF00000000000000000000000000000000000000000000000000000000000000")]);
    }

    #[test]
    fn calldatasize() {
        let cctx = &mut CallContext::default();

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: vec![],
            gas: 0,
            input: hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap(),
            logs: vec![],
            value: U256::ZERO,
        });

        assert_eq!(Instructions::calldatasize(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [30]);
    }

    #[test]
    fn calldatacopy() {
        let cctx = &mut CallContext::default();

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: vec![],
            gas: 0,
            input: hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap(),
            logs: vec![],
            value: U256::ZERO,
        });

        cctx.with_stack(vec![0u8, 0, 32]);
        assert_eq!(Instructions::calldatacopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 9, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap());

        cctx.with_stack(vec![0u8, 31, 8]);
        assert_eq!(Instructions::calldatacopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("FF00000000000000FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap());
    }

    #[test]
    fn codesize() {
        let cctx = &mut CallContext::default();

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap(),
            gas: 0,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });

        assert_eq!(Instructions::codesize(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [30]);
    }

    #[test]
    fn codecopy() {
        let cctx = &mut CallContext::default();

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap(),
            gas: 0,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });

        cctx.with_stack(vec![0u8, 0, 32]);
        assert_eq!(Instructions::codecopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 9, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap());

        cctx.with_stack(vec![0u8, 31, 8]);
        assert_eq!(Instructions::codecopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("FF00000000000000FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap());
    }

    #[test]
    fn gasprice() {
        let cctx = &mut CallContext::default();
        let tctx = &mut TransactionContext::default();

        tctx.with_transaction(Transaction {
            data: vec![],
            from: Address(U256::ZERO),
            gas: 0,
            gas_price: 15,
            nonce: 0,
            to: Address::default(),
            value: U256::ZERO,
        });

        assert_eq!(Instructions::gasprice(&mut WorldState::default(), &tctx, cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [15]);
    }

    #[test]
    fn extcodesize() {
        let state = &mut WorldState::default();
        let cctx = &mut CallContext::default();

        state.with_accounts(&[(Address(uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")), Account { balance: U256::ZERO, code: hex::decode("FF0F4C").unwrap() })]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]);
        assert_eq!(Instructions::extcodesize(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2600, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [3]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]);
        assert_eq!(Instructions::extcodesize(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [3]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D9")]);
        assert_eq!(Instructions::extcodesize(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2600, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D9")]);
        assert_eq!(Instructions::extcodesize(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("0x109BBFED6889322E016E0A02EE459D306FC19545D9")]);
        assert_eq!(Instructions::extcodesize(state, &TransactionContext::default(), cctx), Err(Error::InvalidAddress));
    }

    #[test]
    fn extcodecopy() {
        let state = &mut WorldState::default();
        let cctx = &mut CallContext::default();

        state.with_accounts(&[(Address(uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")), Account { balance: U256::ZERO, code: hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap() })]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8"), U256::ZERO, U256::ZERO, uint!("32")]);
        assert_eq!(Instructions::extcodecopy(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2606, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap());

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8"), U256::ZERO, uint!("31"), uint!("8")]);
        assert_eq!(Instructions::extcodecopy(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 103, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("FF00000000000000FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap());
    }

    #[test]
    fn returndatasize() {
        let cctx = &mut CallContext::default();

        cctx.with_returndata("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF");

        assert_eq!(Instructions::returndatasize(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [32]);
    }

    #[test]
    fn returndatacopy() {
        let cctx = &mut CallContext::default();

        cctx.with_returndata("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF");

        cctx.with_stack(vec![0u8, 0, 32]);
        assert_eq!(Instructions::returndatacopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 9, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap());

        cctx.with_stack(vec![32u8, 31, 1]);
        assert_eq!(Instructions::returndatacopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 9, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000000000000000000000000000000000000000000000000000000000").unwrap());
    }

    #[test]
    fn extcodehash() {
        let state = &mut WorldState::default();
        let cctx = &mut CallContext::default();

        state.with_accounts(&[
            (Address(uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")), Account { balance: uint!("125985"), code: vec![] }),
            (Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")), Account { balance: uint!("125985"), code: vec![0xF0, 0xBD, 0x5A, 0x61, 0x9C, 0xAD, 0x26, 0x29] }),
        ]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]);

        assert_eq!(Instructions::extcodehash(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2600, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xC5D2460186F7233C927E7DB2DCC703C0E500B653CA82273B7BFAD8045D85A470")]);

        cctx.with_stack(vec![uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")]);

        assert_eq!(Instructions::extcodehash(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2600, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xE88A3F7420AC15E5F28B6260FF05BF4700AA744BC3C0C3F801C9EBC65AC260CA")]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]);

        assert_eq!(Instructions::extcodehash(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xC5D2460186F7233C927E7DB2DCC703C0E500B653CA82273B7BFAD8045D85A470")]);
    }

    #[test]
    fn coinbase() {
        let cctx = &mut CallContext::default();
        let tctx = &mut TransactionContext::default();

        tctx.with_block(Block {
            difficulty: U256::ZERO,
            gas_limit: U256::ZERO,
            miner: Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")),
            number: U256::ZERO,
            time: U256::ZERO,
        });

        assert_eq!(Instructions::coinbase(&mut WorldState::default(), tctx, cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")]);
    }

    #[test]
    fn timestamp() {
        let cctx = &mut CallContext::default();
        let tctx = &mut TransactionContext::default();

        tctx.with_block(Block {
            difficulty: U256::ZERO,
            gas_limit: U256::ZERO,
            miner: Address(U256::ZERO),
            number: U256::ZERO,
            time: uint!("1734036363"),
        });

        assert_eq!(Instructions::timestamp(&mut WorldState::default(), tctx, cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1734036363]);
    }

    #[test]
    fn number() {
        let cctx = &mut CallContext::default();
        let tctx = &mut TransactionContext::default();

        tctx.with_block(Block {
            difficulty: U256::ZERO,
            gas_limit: U256::ZERO,
            miner: Address(U256::ZERO),
            number: uint!("50"),
            time: U256::ZERO,
        });

        assert_eq!(Instructions::number(&mut WorldState::default(), tctx, cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [50]);
    }

    #[test]
    fn prevrandao() {
        let cctx = &mut CallContext::default();
        let tctx = &mut TransactionContext::default();

        tctx.with_block(Block {
            difficulty: uint!("50"),
            gas_limit: U256::ZERO,
            miner: Address(U256::ZERO),
            number: U256::ZERO,
            time: U256::ZERO,
        });

        assert_eq!(Instructions::prevrandao(&mut WorldState::default(), tctx, cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [50]);
    }

    #[test]
    fn gaslimit() {
        let cctx = &mut CallContext::default();
        let tctx = &mut TransactionContext::default();

        tctx.with_block(Block {
            difficulty: U256::ZERO,
            gas_limit: uint!("50"),
            miner: Address(U256::ZERO),
            number: U256::ZERO,
            time: U256::ZERO,
        });

        assert_eq!(Instructions::gaslimit(&mut WorldState::default(), tctx, cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [50]);
    }

    #[test]
    fn chainid() {
        let s = &mut WorldState::default();
        let cctx = &mut CallContext::default();

        s.with_chain_id(5u8);

        assert_eq!(Instructions::chainid(s, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [5]);
    }

    #[test]
    fn selfbalance() {
        let state = &mut WorldState::default();
        let cctx = &mut CallContext::default();

        state.with_accounts(&[(Address(uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")), Account { balance: uint!("125985"), code: vec![] })]);
        cctx.with_contract(CallContextContract {
            address: Address(uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")),
            caller: Address(U256::ZERO),
            code: vec![],
            gas: 0,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });

        assert_eq!(Instructions::selfbalance(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [125985]);

        assert_eq!(Instructions::selfbalance(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [125985]);
    }

    #[test]
    fn pop() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![30u8, 42]);
        assert_eq!(Instructions::pop(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [42]);
    }

    #[test]
    fn mload() {
        let cctx = &mut CallContext::default();

        cctx.with_memory("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369");
        cctx.with_stack(vec![0u8]);
        assert_eq!(Instructions::mload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369")]);
        assert_eq!(cctx.memory.0, hex::decode("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369").unwrap());

        cctx.with_memory("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369");
        cctx.with_stack(vec![2u8]);
        assert_eq!(Instructions::mload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0xB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A023690000")]);
        assert_eq!(cctx.memory.0, hex::decode("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A023690000000000000000000000000000000000000000000000000000000000000000").unwrap());

        cctx.with_memory("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369");
        cctx.with_stack(vec![30u8]);
        assert_eq!(Instructions::mload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x2369000000000000000000000000000000000000000000000000000000000000")]);
        assert_eq!(cctx.memory.0, hex::decode("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A023690000000000000000000000000000000000000000000000000000000000000000").unwrap());

        cctx.with_memory("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369");
        cctx.with_stack(vec![500u16]);
        assert_eq!(Instructions::mload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 51, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);
        assert_eq!(cctx.memory.0, hex::decode("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A023690000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap());
    }

    #[test]
    fn mstore() {
        let cctx = &mut CallContext::default();

        cctx.with_memory("");
        cctx.with_stack(vec![0u8, 0xFF]);
        assert_eq!(Instructions::mstore(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("00000000000000000000000000000000000000000000000000000000000000FF").unwrap());
        cctx.with_stack(vec![1u8, 0xFF]);
        assert_eq!(Instructions::mstore(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("0000000000000000000000000000000000000000000000000000000000000000FF00000000000000000000000000000000000000000000000000000000000000").unwrap());

        cctx.with_memory("");
        cctx.with_stack(vec![3u8, 0xFF]);
        assert_eq!(Instructions::mstore(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 9, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("00000000000000000000000000000000000000000000000000000000000000000000FF0000000000000000000000000000000000000000000000000000000000").unwrap());

        cctx.with_memory("");
        cctx.with_stack(vec![500u16, 0xABFF]);
        assert_eq!(Instructions::mstore(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 54, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ABFF000000000000000000000000").unwrap());
    }

    #[test]
    fn mstore8() {
        let cctx = &mut CallContext::default();

        cctx.with_memory("");
        cctx.with_stack(vec![0u16, 0xFFAB]);
        assert_eq!(Instructions::mstore8(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("AB00000000000000000000000000000000000000000000000000000000000000").unwrap());
        cctx.with_stack(vec![31u16, 0xFFAB]);
        assert_eq!(Instructions::mstore8(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("AB000000000000000000000000000000000000000000000000000000000000AB").unwrap());
    }

    #[test]
    fn sload() {
        let state = &mut WorldState::default();
        let cctx = &mut CallContext::default();

        state.with_storage(&[(Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")), &[(42u8, 0xAB)])]);
        cctx.with_contract(CallContextContract {
            address: Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")),
            caller: Address(U256::ZERO),
            code: vec![],
            gas: 0,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });

        cctx.with_stack(vec![42u8]);
        assert_eq!(Instructions::sload(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2100, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0xAB]);
        cctx.with_stack(vec![42u8]);
        assert_eq!(Instructions::sload(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0xAB]);

        cctx.with_stack(vec![40u8]);
        assert_eq!(Instructions::sload(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2100, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);
        cctx.with_stack(vec![40u8]);
        assert_eq!(Instructions::sload(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn sstore() {
        let state = &mut WorldState::default();
        let cctx = &mut CallContext::default();

        cctx.with_contract(CallContextContract {
            address: Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")),
            caller: Address(U256::ZERO),
            gas: 0,
            code: vec![],
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });

        cctx.with_stack(vec![0u16, 0xFFFF]);
        assert_eq!(Instructions::sstore(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 22100, jump: 1 })); // clean storage - no previous value - cold slot
        assert_eq!(state.storage.get(&Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91"))).unwrap().0.get(&uint!("0")), Some(&StorageValue {
            original_value: uint!("0"),
            value: uint!("0xFFFF"),
            warm: true,
        }));
        cctx.with_stack(vec![0u16, 0xFFFF]);
        assert_eq!(Instructions::sstore(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 })); // dirty storage - same value - warn slot
        assert_eq!(state.storage.get(&Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91"))).unwrap().0.get(&uint!("0")), Some(&StorageValue {
            original_value: uint!("0"),
            value: uint!("0xFFFF"),
            warm: true,
        }));
        cctx.with_stack(vec![0u16, 0xFFF0]);
        assert_eq!(Instructions::sstore(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 })); // dirty storage - different value - warn slot
        assert_eq!(state.storage.get(&Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91"))).unwrap().0.get(&uint!("0")), Some(&StorageValue {
            original_value: uint!("0"),
            value: uint!("0xFFF0"),
            warm: true,
        }));

        state.with_storage(&[(Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")), &[(1u8, 55)])]);

        cctx.with_stack(vec![1u16, 10]);
        assert_eq!(Instructions::sstore(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5000, jump: 1 })); // clean storage - different value - cold slot
        assert_eq!(state.storage.get(&Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91"))).unwrap().0.get(&uint!("1")), Some(&StorageValue {
            original_value: uint!("55"),
            value: uint!("10"),
            warm: true,
        }));

        state.with_storage(&[(Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")), &[(1u8, 55)])]);

        cctx.with_stack(vec![1u16, 55]);
        assert_eq!(Instructions::sstore(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2200, jump: 1 })); // clean storage - same value - cold slot
        assert_eq!(state.storage.get(&Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91"))).unwrap().0.get(&uint!("1")), Some(&StorageValue {
            original_value: uint!("55"),
            value: uint!("55"),
            warm: true,
        }));
    }

        #[test]
    fn jump() {
        let cctx = &mut CallContext::default();

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: hex::decode("00005B00").unwrap(),
            gas: 0,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![uint!("0xFFFFFFFFFFFFFFFFFFFF")]);
        assert_eq!(Instructions::jump(&mut WorldState::default(), &TransactionContext::default(), cctx), Err(Error::InvalidJumpDest)); // not a usize
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![0xFFFFu16]);
        assert_eq!(Instructions::jump(&mut WorldState::default(), &TransactionContext::default(), cctx), Err(Error::InvalidJumpDest)); // not in range
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![1u8]);
        assert_eq!(Instructions::jump(&mut WorldState::default(), &TransactionContext::default(), cctx), Err(Error::InvalidJumpDest)); // not a valid destination
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![2u8]);
        assert_eq!(Instructions::jump(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 0 }));
        assert_eq!(cctx.pc, 2);
    }

    #[test]
    fn jumpi() {
        let cctx = &mut CallContext::default();

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: hex::decode("00005B00").unwrap(),
            gas: 0,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![2u8, 0]);
        assert_eq!(Instructions::jumpi(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 10, jump: 1 }));
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![uint!("0xFFFFFFFFFFFFFFFFFFFF"), uint!("1")]);
        assert_eq!(Instructions::jumpi(&mut WorldState::default(), &TransactionContext::default(), cctx), Err(Error::InvalidJumpDest)); // not a usize
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![0xFFFFu16, 1]);
        assert_eq!(Instructions::jumpi(&mut WorldState::default(), &TransactionContext::default(), cctx), Err(Error::InvalidJumpDest)); // not in range
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![1u8, 1]);
        assert_eq!(Instructions::jumpi(&mut WorldState::default(), &TransactionContext::default(), cctx), Err(Error::InvalidJumpDest)); // not a valid destination
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![2u8, 1]);
        assert_eq!(Instructions::jumpi(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 10, jump: 0 }));
        assert_eq!(cctx.pc, 2);
    }

    #[test]
    fn pc() {
        let cctx = &mut CallContext::default();

        cctx.with_pc(30);

        assert_eq!(Instructions::pc(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [30]);
    }

    #[test]
    fn msize() {
        let cctx = &mut CallContext::default();

        cctx.with_memory("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369");

        assert_eq!(Instructions::msize(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [32]);
    }

    #[test]
    fn gas() {
        let cctx = &mut CallContext::default();

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: vec![],
            gas: 5,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });

        assert_eq!(Instructions::gas(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [3]);


        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: vec![],
            gas: 3,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });

        assert_eq!(Instructions::gas(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: vec![],
            gas: 1,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });

        assert_eq!(Instructions::gas(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: vec![],
            gas: 0,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });

        assert_eq!(Instructions::gas(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn jumpdest() {
        assert_eq!(Instructions::jumpdest(&mut WorldState::default(), &TransactionContext::default(), &mut CallContext::default()), Ok(InstructionOutput { cost: 1, jump: 1 }));
    }

    #[test]
    fn tload() {
        let cctx = &mut CallContext::default();

        cctx.with_transient(&[(42u8, 0xAB)]);

        cctx.with_stack(vec![42u8]);
        assert_eq!(Instructions::tload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0xAB]);

        cctx.with_stack(vec![45u8]);
        assert_eq!(Instructions::tload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn tstore() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![1u8, 55]);
        assert_eq!(Instructions::tstore(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(cctx.transient.0.get(&uint!("1")), Some(&uint!("55")));
    }

    #[test]
    fn mcopy() {
        let cctx = &mut CallContext::default();

        cctx.with_memory("00000000000000000000000000000000000000000000000000000000000000000001020304050607080910111213141516171819202122232425262728293031");

        cctx.with_stack(vec![0u8, 32, 32]);
        assert_eq!(Instructions::mcopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("00010203040506070809101112131415161718192021222324252627282930310001020304050607080910111213141516171819202122232425262728293031").unwrap());

        cctx.with_stack(vec![4u8, 8, 16]);
        assert_eq!(Instructions::mcopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("00010203080910111213141516171819202122232021222324252627282930310001020304050607080910111213141516171819202122232425262728293031").unwrap());

        cctx.with_memory("000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F");

        cctx.with_stack(vec![100u8, 4, 40]);
        assert_eq!(Instructions::mcopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 21, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F0000000000000000000000000000000000000000000000000000000000000000").unwrap());
    }

    #[test]
    fn push() {
        let cctx = &mut CallContext::default();

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: hex::decode("015936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155131A8E0556").unwrap(),
            gas: 0,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });

        assert_eq!(Instructions::push::<0>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0")]);

        assert_eq!(Instructions::push::<1>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 2 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x59")]);

        assert_eq!(Instructions::push::<2>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 3 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936")]);

        assert_eq!(Instructions::push::<3>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 4 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2")]);

        assert_eq!(Instructions::push::<4>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 5 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1")]);

        assert_eq!(Instructions::push::<5>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 6 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5")]);

        assert_eq!(Instructions::push::<6>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 7 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3")]);

        assert_eq!(Instructions::push::<7>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 8 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF")]);

        assert_eq!(Instructions::push::<8>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 9 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2E")]);

        assert_eq!(Instructions::push::<9>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 10 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB")]);

        assert_eq!(Instructions::push::<10>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 11 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB31")]);

        assert_eq!(Instructions::push::<11>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 12 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155")]);

        assert_eq!(Instructions::push::<12>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 13 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B9")]);

        assert_eq!(Instructions::push::<13>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 14 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B")]);

        assert_eq!(Instructions::push::<14>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 15 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B30")]);

        assert_eq!(Instructions::push::<15>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 16 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001")]);

        assert_eq!(Instructions::push::<16>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 17 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A3")]);

        assert_eq!(Instructions::push::<17>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 18 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347")]);

        assert_eq!(Instructions::push::<18>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 19 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6")]);

        assert_eq!(Instructions::push::<19>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 20 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE")]);

        assert_eq!(Instructions::push::<20>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 21 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75")]);

        assert_eq!(Instructions::push::<21>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 22 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E5")]);

        assert_eq!(Instructions::push::<22>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 23 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E518")]);

        assert_eq!(Instructions::push::<23>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 24 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859")]);

        assert_eq!(Instructions::push::<24>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 25 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EB")]);

        assert_eq!(Instructions::push::<25>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 26 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA")]);

        assert_eq!(Instructions::push::<26>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 27 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA81")]);

        assert_eq!(Instructions::push::<27>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 28 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155")]);

        assert_eq!(Instructions::push::<28>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 29 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA815513")]);

        assert_eq!(Instructions::push::<29>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 30 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155131A")]);

        assert_eq!(Instructions::push::<30>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 31 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155131A8E")]);

        assert_eq!(Instructions::push::<31>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 32 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155131A8E05")]);

        assert_eq!(Instructions::push::<32>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 33 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155131A8E0556")]);
    }

    #[test]
    fn dup() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![1u8]);
        assert_eq!(Instructions::dup::<1>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 1]);
    
        cctx.with_stack(vec![0u8, 1]);
        assert_eq!(Instructions::dup::<2>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 1]);
        assert_eq!(Instructions::dup::<3>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 1]);
        assert_eq!(Instructions::dup::<4>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 1]);
        assert_eq!(Instructions::dup::<5>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 1]);
        assert_eq!(Instructions::dup::<6>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Instructions::dup::<7>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Instructions::dup::<8>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Instructions::dup::<9>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Instructions::dup::<10>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Instructions::dup::<11>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Instructions::dup::<12>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Instructions::dup::<13>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Instructions::dup::<14>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Instructions::dup::<15>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Instructions::dup::<16>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn swap() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![1u8, 2]);
        assert_eq!(Instructions::swap::<2>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 1]);

        cctx.with_stack(vec![1u8, 0, 2]);
        assert_eq!(Instructions::swap::<3>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 2]);
        assert_eq!(Instructions::swap::<4>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 2]);
        assert_eq!(Instructions::swap::<5>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 2]);
        assert_eq!(Instructions::swap::<6>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Instructions::swap::<7>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Instructions::swap::<8>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Instructions::swap::<9>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Instructions::swap::<10>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Instructions::swap::<11>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Instructions::swap::<12>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Instructions::swap::<13>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Instructions::swap::<14>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Instructions::swap::<15>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Instructions::swap::<16>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Instructions::swap::<17>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Instructions::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    }

        #[test]
    fn log() {
        let cctx = &mut CallContext::default();

        cctx.with_memory("000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F");

        cctx.with_stack(vec![4u8, 16]);
        assert_eq!(Instructions::log::<0>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 503, jump: 1 }));
        assert_eq!(cctx.contract.logs, vec![Log {
            data: hex::decode("0405060708090A0B0C0D0E0F10111213").unwrap(),
            topics: [None, None, None, None],
        }]);

        cctx.with_stack(vec![14u8, 8, 42]);
        assert_eq!(Instructions::log::<1>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 814, jump: 1 }));
        assert_eq!(cctx.contract.logs, vec![Log {
            data: hex::decode("0405060708090A0B0C0D0E0F10111213").unwrap(),
            topics: [None, None, None, None],
        }, Log {
            data: hex::decode("0E0F101112131415").unwrap(),
            topics: [Some(uint!("42")), None, None, None],
        }]);

        cctx.with_stack(vec![18u8, 20, 50, 52]);
        assert_eq!(Instructions::log::<2>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 1288, jump: 1 }));
        assert_eq!(cctx.contract.logs, vec![Log {
            data: hex::decode("0405060708090A0B0C0D0E0F10111213").unwrap(),
            topics: [None, None, None, None],
        }, Log {
            data: hex::decode("0E0F101112131415").unwrap(),
            topics: [Some(uint!("42")), None, None, None],
        }, Log {
            data: hex::decode("12131415161718191A1B1C1D1E1F000000000000").unwrap(),
            topics: [Some(uint!("50")), Some(uint!("52")), None, None],
        }]);
    }

    #[test]
    fn r#return() {
        let cctx = &mut CallContext::default();

        cctx.with_memory("FF01");
        cctx.with_stack(vec![0u8, 2]);

        assert!(!cctx.stop);
        assert_eq!(*cctx.returndata, vec![]);
        assert_eq!(Instructions::r#return(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 0, jump: 0 }));
        assert!(cctx.stop);
        assert_eq!(cctx.r#return, vec![0xFF, 1]);
    }

    #[test]
    fn revert() {
        let cctx = &mut CallContext::default();

        cctx.with_memory("FF01");
        cctx.with_stack(vec![0u8, 2]);

        assert!(!cctx.stop);
        assert!(!cctx.revert);
        assert_eq!(*cctx.returndata, vec![]);
        assert_eq!(Instructions::revert(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 0, jump: 0 }));
        assert!(cctx.stop);
        assert!(cctx.revert);
        assert_eq!(cctx.r#return, vec![0xFF, 1]);
    }

    #[test]
    fn invalid() {
        let cctx = &mut CallContext::default();

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: vec![],
            gas: 25,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });

        assert!(!cctx.stop);
        assert!(!cctx.revert);
        assert_eq!(*cctx.returndata, vec![]);
        assert_eq!(Instructions::invalid(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 25, jump: 0 }));
        assert!(cctx.stop);
        assert!(cctx.revert);
    }
}
