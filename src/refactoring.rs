use ethnum::{u256, AsU256, U256};
use std::{cmp::min, collections::HashMap};
use crate::{errors::Error, memory::{Memory, ReadWriteOperation}, stack::Stack, storage::Storage, transaction::{Account, Address, Block, Transaction}, transient::Transient, utils::{Hash, IsNeg, NeededSizeInBytes, WrappingBigPow, WrappingSignedDiv, WrappingSignedRem}};

#[derive(Default)]
pub struct WorldState {
    pub accounts: Storage<Address, Account>,
    pub chain_id: u256,
    pub storage: HashMap<Address, Storage<u256, u256>>,
}

#[derive(Default, Clone, Debug, Eq, PartialEq)]
pub struct Log {
    data: Vec<u8>,
    topics: [Option<U256>; 4],
}

#[derive(Default)]
pub struct CallContextContract {
    pub address: Address,
    pub caller: Address,
    pub code: Vec<u8>,
    pub gas: usize,
    pub input: Vec<u8>,
    pub logs: Vec<Log>,
    pub value: u256,
}

#[derive(Default)]
pub struct TransactionContext {
    pub block: Block,
    pub tx: Transaction,
}

#[derive(Default)]
pub struct CallContext {
    pub contract: CallContextContract,
    pub memory: Memory,
    pub pc: usize,
    pub r#return: Vec<u8>,
    pub returndata: Vec<u8>,
    pub revert: bool,
    pub stack: Stack,
    pub stop: bool,
    pub transient: Transient,
}

impl CallContext {
    fn from_transaction(s: &mut WorldState, tx: &Transaction) -> Self {
        let contract_address = tx.contract_address();
        let contract_input = &tx.data;
        let contract = CallContextContract {
            address: contract_address,
            caller: tx.from,
            code: if tx.is_contract_creation() { contract_input.clone() } else { s.accounts.load(contract_address).value.code },
            gas: tx.gas,
            input: contract_input.clone(),
            logs: Vec::default(),
            value: tx.value,
        };
        Self {
            contract,
            memory: Memory::new(),
            pc: 0,
            r#return: Vec::default(),
            returndata: Vec::default(),
            revert: false,
            stack: Stack::new(),
            stop: false,
            transient: Transient::new(),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct InstructionOutput {
    cost: usize,
    jump: usize,
}

type InstructionResult = Result<InstructionOutput, Error>;
type InstructionFn = fn (&mut WorldState, &TransactionContext, &mut CallContext) -> InstructionResult;

#[derive(Default, Debug, Eq, PartialEq)]
pub struct ExecutionOutput {
    pub data: Vec<u8>,
    pub remaining_gas: usize,
    pub revert: bool,
}
pub type ExecutionResult = Result<ExecutionOutput, Error>;

pub struct Machine {}

impl Machine {
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

    fn stop(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        cctx.stop = true;
        Ok(InstructionOutput { cost: 0, jump: 0 })
    }

    fn add(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [a.wrapping_add(b)])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn mul(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [a.wrapping_mul(b)])?;
        Ok(InstructionOutput { cost: 5, jump: 1 })
    }

    fn sub(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [a.wrapping_sub(b)])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn div(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [if b == 0 { U256::ZERO } else { a.wrapping_div(b) }])?;
        Ok(InstructionOutput { cost: 5, jump: 1 })
    }

    fn sdiv(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [if b == 0 { U256::ZERO } else { a.wrapping_signed_div(b) }])?;
        Ok(InstructionOutput { cost: 5, jump: 1 })
    }

    fn r#mod(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [if b == 0 { U256::ZERO } else { a.wrapping_rem(b) }])?;
        Ok(InstructionOutput { cost: 5, jump: 1 })
    }

    fn smod(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [if b == 0 { U256::ZERO } else { a.wrapping_signed_rem(b) }])?;
        Ok(InstructionOutput { cost: 5, jump: 1 })
    }

    fn addmod(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b, n] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_add(b.wrapping_rem(n)).wrapping_rem(n) }])?;
        Ok(InstructionOutput { cost: 8, jump: 1 })
    }

    fn mulmod(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b, n] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [if n == 0 { U256::ZERO } else { a.wrapping_rem(n).wrapping_mul(b.wrapping_rem(n)).wrapping_rem(n) }])?;
        Ok(InstructionOutput { cost: 8, jump: 1 })
    }

    fn exp(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, e] = Machine::pop_or_fail(cctx)?;
        let exponent_byte_size = e.needed_size_in_bytes();
        Machine::push_rev_or_fail(cctx, [a.wrapping_big_pow(e)])?;
        Ok(InstructionOutput { cost: 10 + 50 * exponent_byte_size, jump: 1 })
    }

    fn signextend(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [b, x] = Machine::pop_or_fail(cctx)?;
        let b: u32 = min(b, u256::from(30u32)).try_into().unwrap();
        let mask = U256::ONE.wrapping_shl((b + 1).wrapping_shl(3));
        let sign_mask = mask.wrapping_shr(1);
        let size_mask = mask - 1;
        let value = x & size_mask;
        Machine::push_rev_or_fail(cctx, [if (value & sign_mask) != 0 { !size_mask | value } else { value }])?;
        Ok(InstructionOutput { cost: 5, jump: 1 })
    }

    fn lt(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [if a < b { U256::ONE } else { U256::ZERO }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn gt(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [if a > b { U256::ONE } else { U256::ZERO }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn slt(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [match (a.is_neg(), b.is_neg()) {
            (true, false) => { U256::ONE },
            (false, true) => { U256::ZERO },
            _ => if a < b { U256::ONE } else { U256::ZERO },
        }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn sgt(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [match (a.is_neg(), b.is_neg()) {
            (true, false) => { U256::ZERO },
            (false, true) => { U256::ONE },
            _ => if a > b { U256::ONE } else { U256::ZERO },
        }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn eq(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [if a == b { U256::ONE } else { U256::ZERO }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn iszero(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [if a == U256::ZERO { U256::ONE } else { U256::ZERO }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn and(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [a & b])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn or(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [a | b])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn xor(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a, b] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [a ^ b])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn not(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [a] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [!a])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn byte(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [i, x] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [if i > 31 { U256::ZERO } else { (x >> (8 * (31 - i))) & 0xFF }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn shl(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [shift, value] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [match TryInto::<u8>::try_into(shift) {
            Ok(shift) => value.wrapping_shl(shift.into()),
            _ => U256::ZERO,
        }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
   }

    fn shr(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [shift, value] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [match TryInto::<u8>::try_into(shift) {
            Ok(shift) => value.wrapping_shr(shift.into()),
            _ => U256::ZERO,
        }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn sar(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [shift, value] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [match (TryInto::<u8>::try_into(shift), value.is_neg()) {
            (Ok(shift), false) => value.wrapping_shr(shift.into()),
            (Ok(shift), true) => { if shift == 0 { value } else { !(U256::ONE.wrapping_shl((255 - shift + 1).into()) - 1) | value.wrapping_shr(shift.into()) } },
            (Err(_), false) => U256::ZERO,
            (Err(_), true) => U256::MAX,
        }])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn keccak256(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [offset, size] = Machine::pop_or_fail(cctx)?;
        let ReadWriteOperation { size, extension_cost, result, .. } = cctx.memory.load(offset, size)?;
        Machine::push_rev_or_fail(cctx, [result.keccak256()])?;
        Ok(InstructionOutput { cost: 30 + 6 * ((size + 31) >> 5) + extension_cost, jump: 1 })
    }

    fn address(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [cctx.contract.address.0])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn balance(s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [address] = Machine::pop_or_fail(cctx)?;
        let account = s.accounts.load(address.try_into()?);
        Machine::push_rev_or_fail(cctx, [account.value.balance])?;
        Ok(InstructionOutput { cost: if account.warm { 100 } else { 2600 }, jump: 1 })
    }

    fn origin(_s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [tctx.tx.from.0])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn caller(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [cctx.contract.caller.0])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn callvalue(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [cctx.contract.value])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn calldataload(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [offset] = Machine::pop_or_fail(cctx)?;
        Machine::push_rev_or_fail(cctx, [match TryInto::<usize>::try_into(offset) {
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

    fn calldatasize(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [cctx.contract.input.len().as_u256()])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn calldatacopy(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [dest_offset, offset, size] = Machine::pop_or_fail(cctx)?;
        let (calldata_offset, calldata_size): (usize, usize) = (offset.try_into().unwrap(), size.try_into().unwrap()); // TODO (fguerin - 11/12/2024) Handle calldata out of bounds
        let value = &cctx.contract.input[calldata_offset..min(cctx.contract.input.len(), calldata_offset + calldata_size)];
        let ReadWriteOperation { size, extension_cost, .. } = cctx.memory.store(dest_offset, size, value.to_vec())?;
        Ok(InstructionOutput { cost: 3 + 3 * ((size + 31) >> 5) + extension_cost, jump: 1 })
    }

    fn codesize(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [cctx.contract.code.len().as_u256()])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn codecopy(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [dest_offset, offset, size] = Machine::pop_or_fail(cctx)?;
        let (code_offset, code_size): (usize, usize) = (offset.try_into().unwrap(), size.try_into().unwrap()); // TODO (fguerin - 11/12/2024) Handle code out of bounds
        let value = &cctx.contract.code[code_offset..min(cctx.contract.code.len(), code_offset + code_size)];
        let ReadWriteOperation { size, extension_cost, .. } = cctx.memory.store(dest_offset, size, value.to_vec())?;
        Ok(InstructionOutput { cost: 3 + 3 * ((size + 31) >> 5) + extension_cost, jump: 1 })
    }

    fn gasprice(_s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [tctx.tx.gas_price.as_u256()])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn extcodesize(s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [address] = Machine::pop_or_fail(cctx)?;
        let account = s.accounts.load(address.try_into()?);
        Machine::push_rev_or_fail(cctx, [account.value.code.len().as_u256()])?;
        Ok(InstructionOutput { cost: if account.warm { 100 } else { 2600 }, jump: 1 })
    }

    fn extcodecopy(s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [address, dest_offset, offset, size] = Machine::pop_or_fail(cctx)?;
        let account = s.accounts.load(address.try_into()?);
        let (code_offset, code_size): (usize, usize) = (offset.try_into().unwrap(), size.try_into().unwrap()); // TODO (fguerin - 13/12/2024) Handle code out of bounds
        let value = &account.value.code[code_offset..min(account.value.code.len(), code_offset + code_size)];
        let ReadWriteOperation { size, extension_cost, .. } = cctx.memory.store(dest_offset, size, value.to_vec())?;
        Ok(InstructionOutput { cost: 3 * ((size + 31) >> 5) + extension_cost + if account.warm { 100 } else { 2600 }, jump: 1 })
    }

    fn returndatasize(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [cctx.returndata.len().as_u256()])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn returndatacopy(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [dest_offset, offset, size] = Machine::pop_or_fail(cctx)?;
        let (returndata_offset, returndata_size): (usize, usize) = (offset.try_into().unwrap(), size.try_into().unwrap()); // TODO (fguerin - 13/12/2024) Handle returndata out of bounds
        let value = &cctx.returndata[returndata_offset..min(cctx.returndata.len(), returndata_offset + returndata_size)];
        let ReadWriteOperation { size, extension_cost, .. } = cctx.memory.store(dest_offset, size, value.to_vec())?;
        Ok(InstructionOutput { cost: 3 + 3 * ((size + 31) >> 5) + extension_cost, jump: 1 })
    }

    fn extcodehash(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    fn blockhash(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    fn coinbase(_s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [tctx.block.miner.0])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn timestamp(_s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [tctx.block.time])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn number(_s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [tctx.block.number])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn prevrandao(_s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [tctx.block.difficulty])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn gaslimit(_s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [tctx.block.gas_limit])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn chainid(s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [s.chain_id])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn selfbalance(s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
         // TODO (fguerin - 13/12/2024) Test whether it should warm the storage
        let account = s.accounts.load(cctx.contract.address);
        Machine::push_rev_or_fail(cctx, [account.value.balance])?;
        Ok(InstructionOutput { cost: 5, jump: 1 })
    }

    fn basefee(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    fn blobhash(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    fn blobbasefee(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    fn pop(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::pop_or_fail::<1>(cctx)?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn mload(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [offset] = Machine::pop_or_fail(cctx)?;
        let ReadWriteOperation { extension_cost, result, .. } = cctx.memory.load_word(offset)?;
        Machine::push_rev_or_fail(cctx, [result])?;
        Ok(InstructionOutput { cost: 3 + extension_cost, jump: 1 })
    }

    fn mstore(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [offset, value] = Machine::pop_or_fail(cctx)?;
        let ReadWriteOperation { extension_cost, .. } = cctx.memory.store_word(offset, value)?;
        Ok(InstructionOutput { cost: 3 + extension_cost, jump: 1 })
    }

    fn mstore8(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [offset, value] = Machine::pop_or_fail(cctx)?;
        let ReadWriteOperation { extension_cost, .. } = cctx.memory.store_byte(offset, value)?;
        Ok(InstructionOutput { cost: 3 + extension_cost, jump: 1 })
    }

    fn sload(s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [key] = Machine::pop_or_fail(cctx)?;
        let storage = s.storage.entry(cctx.contract.address).or_default();
        let result = storage.load(key);
        Machine::push_rev_or_fail(cctx, [result.value])?;
        Ok(InstructionOutput { cost: if result.warm { 100 } else { 2100 }, jump: 1 })
    }

    fn sstore(s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        // TODO (fguerin - 14/12/2024) Add gas refund
        let [key, value] = Machine::pop_or_fail(cctx)?;
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

    fn jump(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [counter] = Machine::pop_or_fail(cctx)?;
        Machine::jump_or_fail(cctx, counter)?;
        Ok(InstructionOutput { cost: 8, jump: 0 })
    }

    fn jumpi(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [counter, b] = Machine::pop_or_fail(cctx)?;
        let jump = match b {
            U256::ZERO => 1,
            _ => { Machine::jump_or_fail(cctx, counter)?; 0 },
        };
        Ok(InstructionOutput { cost: 10, jump })
    }

    fn pc(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [cctx.pc.as_u256()])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn msize(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [cctx.memory.size().as_u256()])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn gas(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        Machine::push_rev_or_fail(cctx, [if cctx.contract.gas < 2 { U256::ZERO } else { (cctx.contract.gas - 2).as_u256() }])?;
        Ok(InstructionOutput { cost: 2, jump: 1 })
    }

    fn jumpdest(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        Ok(InstructionOutput { cost: 1, jump: 1 })
    }

    fn tload(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [key] = Machine::pop_or_fail(cctx)?;
        let value = cctx.transient.load(key);
        Machine::push_rev_or_fail(cctx, [value])?;
        Ok(InstructionOutput { cost: 100, jump: 1 })
    }

    fn tstore(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [key, value] = Machine::pop_or_fail(cctx)?;
        cctx.transient.store(key, value);
        Ok(InstructionOutput { cost: 100, jump: 1 })
    }

    fn mcopy(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [dest_offset, offset, size] = Machine::pop_or_fail(cctx)?;
        let ReadWriteOperation { result, extension_cost: read_extension_cost, .. } = cctx.memory.load(offset, size)?;
        let ReadWriteOperation { size, extension_cost: write_extension_cost, .. } = cctx.memory.store(dest_offset, size, result)?;
        Ok(InstructionOutput { cost: 3 + 3 * ((size + 31) >> 5) + read_extension_cost + write_extension_cost, jump: 1 })
    }

    fn push<const N: usize>(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let mut res = U256::ZERO;
        for i in 0..N {
            res <<= 8;
            res |= u256::from(*cctx.contract.code.get(cctx.pc + i + 1).unwrap_or(&0u8));
        };
        Machine::push_rev_or_fail(cctx, [res])?;
        Ok(InstructionOutput { cost: if N == 0 { 2 } else { 3 }, jump: N + 1 })
    }

    fn dup<const N: usize>(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let values = Machine::pop_or_fail::<N>(cctx)?;
        Machine::push_rev_or_fail(cctx, values)?;
        Machine::push_rev_or_fail(cctx, [values[N - 1]])?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn swap<const N: usize>(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let mut values = Machine::pop_or_fail::<N>(cctx)?;
        values.swap(0, N - 1);
        Machine::push_rev_or_fail(cctx, values)?;
        Ok(InstructionOutput { cost: 3, jump: 1 })
    }

    fn log<const N: usize>(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        let [offset, size] = Machine::pop_or_fail(cctx)?;
        let topics = Machine::pop_or_fail::<N>(cctx)?;
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

    fn create(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    fn call(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    fn callcode(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    fn r#return(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        cctx.stop = true;
        let [offset, size] = Machine::pop_or_fail(cctx)?;
        let ReadWriteOperation { result: data, extension_cost, .. } = cctx.memory.load(offset, size)?;
        cctx.r#return = data;
        Ok(InstructionOutput { cost: extension_cost, jump: 0 })
    }

    fn delegatecall(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    fn create2(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    fn staticcall(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    fn revert(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        cctx.stop = true;
        cctx.revert = true;
        let [offset, size] = Machine::pop_or_fail(cctx)?;
        let ReadWriteOperation { result: data, extension_cost, .. } = cctx.memory.load(offset, size)?;
        cctx.r#return = data;
        Ok(InstructionOutput { cost: extension_cost, jump: 0 })
    }

    fn invalid(_s: &mut WorldState, _tctx: &TransactionContext, cctx: &mut CallContext) -> InstructionResult {
        cctx.stop = true;
        cctx.revert = true;
        Ok(InstructionOutput { cost: cctx.contract.gas, jump: 0 })
    }

    fn selfdestruct(_s: &mut WorldState, _tctx: &TransactionContext, _cctx: &mut CallContext) -> InstructionResult {
        todo!();
    }

    fn next_instruction(cctx: &CallContext) -> InstructionFn {
        match *cctx.contract.code.get(cctx.pc).unwrap_or(&0) {
            0x00 => Machine::stop,
            0x01 => Machine::add,
            0x02 => Machine::mul,
            0x03 => Machine::sub,
            0x04 => Machine::div,
            0x05 => Machine::sdiv,
            0x06 => Machine::r#mod,
            0x07 => Machine::smod,
            0x08 => Machine::addmod,
            0x09 => Machine::mulmod,
            0x0A => Machine::exp,
            0x0B => Machine::signextend,
            0x10 => Machine::lt,
            0x11 => Machine::gt,
            0x12 => Machine::slt,
            0x13 => Machine::sgt,
            0x14 => Machine::eq,
            0x15 => Machine::iszero,
            0x16 => Machine::and,
            0x17 => Machine::or,
            0x18 => Machine::xor,
            0x19 => Machine::not,
            0x1A => Machine::byte,
            0x1B => Machine::shl,
            0x1C => Machine::shr,
            0x1D => Machine::sar,
            0x20 => Machine::keccak256,
            0x30 => Machine::address,
            0x31 => Machine::balance,
            0x32 => Machine::origin,
            0x33 => Machine::caller,
            0x34 => Machine::callvalue,
            0x35 => Machine::calldataload,
            0x36 => Machine::calldatasize,
            0x37 => Machine::calldatacopy,
            0x38 => Machine::codesize,
            0x39 => Machine::codecopy,
            0x3A => Machine::gasprice,
            0x3B => Machine::extcodesize,
            0x3C => Machine::extcodecopy,
            0x3D => Machine::returndatasize,
            0x3E => Machine::returndatacopy,
            0x3F => Machine::extcodehash,
            0x40 => Machine::blockhash,
            0x41 => Machine::coinbase,
            0x42 => Machine::timestamp,
            0x43 => Machine::number,
            0x44 => Machine::prevrandao,
            0x45 => Machine::gaslimit,
            0x46 => Machine::chainid,
            0x47 => Machine::selfbalance,
            0x48 => Machine::basefee,
            0x49 => Machine::blobhash,
            0x4A => Machine::blobbasefee,
            0x50 => Machine::pop,
            0x51 => Machine::mload,
            0x52 => Machine::mstore,
            0x53 => Machine::mstore8,
            0x54 => Machine::sload,
            0x55 => Machine::sstore,
            0x56 => Machine::jump,
            0x57 => Machine::jumpi,
            0x58 => Machine::pc,
            0x59 => Machine::msize,
            0x5A => Machine::gas,
            0x5B => Machine::jumpdest,
            0x5C => Machine::tload,
            0x5D => Machine::tstore,
            0x5E => Machine::mcopy,
            0x5F => Machine::push::<0>,
            0x60 => Machine::push::<1>,
            0x61 => Machine::push::<2>,
            0x62 => Machine::push::<3>,
            0x63 => Machine::push::<4>,
            0x64 => Machine::push::<5>,
            0x65 => Machine::push::<6>,
            0x66 => Machine::push::<7>,
            0x67 => Machine::push::<8>,
            0x68 => Machine::push::<9>,
            0x69 => Machine::push::<10>,
            0x6A => Machine::push::<11>,
            0x6B => Machine::push::<12>,
            0x6C => Machine::push::<13>,
            0x6D => Machine::push::<14>,
            0x6E => Machine::push::<15>,
            0x6F => Machine::push::<16>,
            0x70 => Machine::push::<17>,
            0x71 => Machine::push::<18>,
            0x72 => Machine::push::<19>,
            0x73 => Machine::push::<20>,
            0x74 => Machine::push::<21>,
            0x75 => Machine::push::<22>,
            0x76 => Machine::push::<23>,
            0x77 => Machine::push::<24>,
            0x78 => Machine::push::<25>,
            0x79 => Machine::push::<26>,
            0x7A => Machine::push::<27>,
            0x7B => Machine::push::<28>,
            0x7C => Machine::push::<29>,
            0x7D => Machine::push::<30>,
            0x7E => Machine::push::<31>,
            0x7F => Machine::push::<32>,
            0x80 => Machine::dup::<1>,
            0x81 => Machine::dup::<2>,
            0x82 => Machine::dup::<3>,
            0x83 => Machine::dup::<4>,
            0x84 => Machine::dup::<5>,
            0x85 => Machine::dup::<6>,
            0x86 => Machine::dup::<7>,
            0x87 => Machine::dup::<8>,
            0x88 => Machine::dup::<9>,
            0x89 => Machine::dup::<10>,
            0x8A => Machine::dup::<11>,
            0x8B => Machine::dup::<12>,
            0x8C => Machine::dup::<13>,
            0x8D => Machine::dup::<14>,
            0x8E => Machine::dup::<15>,
            0x8F => Machine::dup::<16>,
            0x90 => Machine::swap::<2>,
            0x91 => Machine::swap::<3>,
            0x92 => Machine::swap::<4>,
            0x93 => Machine::swap::<5>,
            0x94 => Machine::swap::<6>,
            0x95 => Machine::swap::<7>,
            0x96 => Machine::swap::<8>,
            0x97 => Machine::swap::<9>,
            0x98 => Machine::swap::<10>,
            0x99 => Machine::swap::<11>,
            0x9A => Machine::swap::<12>,
            0x9B => Machine::swap::<13>,
            0x9C => Machine::swap::<14>,
            0x9D => Machine::swap::<15>,
            0x9E => Machine::swap::<16>,
            0x9F => Machine::swap::<17>,
            0xA0 => Machine::log::<0>,
            0xA1 => Machine::log::<1>,
            0xA2 => Machine::log::<2>,
            0xA3 => Machine::log::<3>,
            0xA4 => Machine::log::<4>,
            0xF0 => Machine::create,
            0xF1 => Machine::call,
            0xF2 => Machine::callcode,
            0xF3 => Machine::r#return,
            0xF4 => Machine::delegatecall,
            0xF5 => Machine::create2,
            0xFA => Machine::staticcall,
            0xFD => Machine::revert,
            0xFE => Machine::invalid,
            0xFF => Machine::selfdestruct,
            _ => Machine::invalid,
        }
    }

    fn check_intrisic_cost(tctx: &TransactionContext, cctx: &mut CallContext) -> Result<(), Error> {
        let intrinsic_cost = tctx.tx.intrinsic_gas_cost();

        if cctx.contract.gas < intrinsic_cost { return Err(Error::IntrisicGasTooLow(intrinsic_cost)) };

        cctx.contract.gas -= intrinsic_cost;

        Ok(())
    }

    fn execute_next_opcode(s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> Result<(), Error> {
        let f = Machine::next_instruction(cctx);
        let output = f(s, tctx, cctx)?;

        if cctx.contract.gas < output.cost { cctx.contract.gas = 0; return Err(Error::OutOfGas); }

        cctx.contract.gas -= output.cost;
        cctx.pc += output.jump;

        Ok(())
    }

    pub fn execute_transaction(s: &mut WorldState, tctx: &TransactionContext) -> ExecutionResult {
        let cctx = &mut CallContext::from_transaction(s, &tctx.tx);

        Machine::check_intrisic_cost(tctx, cctx)?;

        let sender_balance = s.accounts.load(tctx.tx.from).value.balance;
        let actual_cost = (tctx.tx.gas * tctx.tx.gas_price).as_u256() + tctx.tx.value;

        if sender_balance < actual_cost { return Err(Error::InsufficientFunds(actual_cost)) };

        // TODO (fguerin - 22/12/2024) Handle sub-context creations
        while !cctx.stop {
            Machine::execute_next_opcode(s, tctx, cctx)?;
        }

        Ok(ExecutionOutput {
            data: cctx.r#return.clone(),
            remaining_gas: cctx.contract.gas,
            revert: cctx.revert,
        })
    }
}

#[cfg(test)]
mod tests {
    use ethnum::{uint, U256};
    use crate::storage::StorageValue;
    use super::*;

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
        assert_eq!(Machine::stop(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 0, jump: 0 }));
        assert!(cctx.stop);
    }

    #[test]
    fn add() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::add(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [16]);

        cctx.with_stack(vec![U256::MAX, U256::ONE]);
        assert_eq!(Machine::add(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn mul() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::mul(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [60]);

        cctx.with_stack(vec![U256::MAX, uint!("2")]);
        assert_eq!(Machine::mul(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [U256::MAX - 1]);
    }

    #[test]
    fn sub() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::sub(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [4]);

        cctx.with_stack(vec![0u8, 1]);
        assert_eq!(Machine::sub(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [U256::MAX]);
    }

    
    #[test]
    fn div() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::div(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![6u8, 0]);
        assert_eq!(Machine::div(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]); // dividing by zero returns zero by convention
    }

    #[test]
    fn sdiv() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::sdiv(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![U256::MAX - 1, U256::MAX]);
        assert_eq!(Machine::sdiv(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2]);

        cctx.with_stack(vec![U256::MAX - 1, U256::ONE]);
        assert_eq!(Machine::sdiv(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [U256::MAX - 1]);

        cctx.with_stack(vec![6u8, 0]);
        assert_eq!(Machine::sdiv(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]); // dividing by zero returns zero by convention
    }

    #[test]
    fn r#mod() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::r#mod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [4]);

        cctx.with_stack(vec![10u8, 3]);
        assert_eq!(Machine::r#mod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![6u8, 0]);
        assert_eq!(Machine::r#mod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]); // modulo zero returns zero by convention
    }

    #[test]
    fn smod() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 6]);
        assert_eq!(Machine::smod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [4]);

        cctx.with_stack(vec![3u8, 2]);
        assert_eq!(Machine::smod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![U256::MAX - 7, U256::MAX - 2]);
        assert_eq!(Machine::smod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [U256::MAX - 1]);

        cctx.with_stack(vec![U256::MAX - 2, uint!("2")]);
        assert_eq!(Machine::smod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [U256::MAX]);

        cctx.with_stack(vec![uint!("3"), U256::MAX - 1]);
        assert_eq!(Machine::smod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![6u8, 0]);
        assert_eq!(Machine::smod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]); // modulo zero returns zero by convention
    }
   
    #[test]
    fn addmod() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 10, 8]);
        assert_eq!(Machine::addmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [4]);

        cctx.with_stack(vec![U256::MAX, uint!("2"), uint!("2")]);
        assert_eq!(Machine::addmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![U256::MAX - 2, uint!("2"), uint!("3")]);
        assert_eq!(Machine::addmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![U256::MAX, uint!("1"), uint!("10")]);
        assert_eq!(Machine::addmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [6]);

        cctx.with_stack(vec![4u8, 6, 0]);
        assert_eq!(Machine::addmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]); // modulo zero returns zero by convention
    }
    
    #[test]
    fn mulmod() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 10, 8]);
        assert_eq!(Machine::mulmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [4]);

        cctx.with_stack(vec![U256::MAX, U256::MAX, uint!("12")]);
        assert_eq!(Machine::mulmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [9]);

        cctx.with_stack(vec![U256::MAX - 2, uint!("2"), uint!("3")]);
        assert_eq!(Machine::mulmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2]);

        cctx.with_stack(vec![4u8, 6, 0]);
        assert_eq!(Machine::mulmod(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);
    }


    #[test]
    fn exp() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 2]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 60, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [100]);

        cctx.with_stack(vec![2u8, 2]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 60, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [4]);

        cctx.with_stack(vec![5u8, 0]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 10, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![2u8, 10]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 60, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1024]);

        cctx.with_stack(vec![2u16, 260]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 110, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("0xFFFFFFFFFFFFFFFF"), uint!("3")]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 60, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0xFFFFFFFFFFFFFFFD0000000000000002FFFFFFFFFFFFFFFF")]);

        cctx.with_stack(vec![uint!("3"), uint!("0xFFFFFFFFFFFFFFF0")]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 410, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0xE9377A20E36295B65EA7F55D4A333F73CF25A1BE32FEBCF9702BDE500F57B8C1")]);

        cctx.with_stack(vec![uint!("5"), uint!("0xFFFFFFFFFFFFFFF0FFFFFF")]);
        assert_eq!(Machine::exp(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 560, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x49E63006C06484CE7E18DB842AD1771FC1C83AA03B09227A2EB3765958CCCCCD")]);
    }

    #[test]
    fn signextend() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![0u8, 0x41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0x41]);

        cctx.with_stack(vec![0u16, 0xEF41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0x41]);

        cctx.with_stack(vec![1u16, 0xEF41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEF41")]);

        cctx.with_stack(vec![2u16, 0xEF41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0xEF41]);

        cctx.with_stack(vec![30u16, 0xEF41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0xEF41]);

        cctx.with_stack(vec![31u16, 0xEF41]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0xEF41]);

        cctx.with_stack(vec![uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFF"), uint!("0xEF41")]);
        assert_eq!(Machine::signextend(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0xEF41]);
    }


    #[test]
    fn lt() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![9u8, 10]);
        assert_eq!(Machine::lt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![10u8, 10]);
        assert_eq!(Machine::lt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn gt() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 9]);
        assert_eq!(Machine::gt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![10u8, 10]);
        assert_eq!(Machine::gt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn eq() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![10u8, 10]);
        assert_eq!(Machine::eq(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![10u8, 3]);
        assert_eq!(Machine::eq(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn iszero() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![0u8]);
        assert_eq!(Machine::iszero(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![3u8]);
        assert_eq!(Machine::iszero(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn slt() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![U256::MAX, U256::ONE]);
        assert_eq!(Machine::slt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![U256::MAX, U256::MAX - 1]);
        assert_eq!(Machine::slt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![U256::ZERO, U256::MAX]);
        assert_eq!(Machine::slt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![1u8, 2]);
        assert_eq!(Machine::slt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![10u8, 10]);
        assert_eq!(Machine::slt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn sgt() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![U256::MAX, U256::ZERO]);
        assert_eq!(Machine::sgt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![U256::MAX, U256::MAX - 1]);
        assert_eq!(Machine::sgt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![U256::ZERO, U256::MAX]);
        assert_eq!(Machine::sgt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![1u8, 2]);
        assert_eq!(Machine::sgt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![10u8, 10]);
        assert_eq!(Machine::sgt(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn and() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![0xFFu8, 0xFF]);
        assert_eq!(Machine::and(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0xFF]);

        cctx.with_stack(vec![0u8, 0xFF]);
        assert_eq!(Machine::and(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![0xF0u8, 0xFF]);
        assert_eq!(Machine::and(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0xF0]);
    }

    #[test]
    fn or() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![0xFFu8, 0xFF]);
        assert_eq!(Machine::or(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0xFF]);

        cctx.with_stack(vec![0u8, 0xFF]);
        assert_eq!(Machine::or(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0xFF]);

        cctx.with_stack(vec![0xF0u8, 0xFF]);
        assert_eq!(Machine::or(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0xFF]);
    }

    #[test]
    fn xor() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![0xFFu8, 0xFF]);
        assert_eq!(Machine::xor(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![0u8, 0xFF]);
        assert_eq!(Machine::xor(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0xFF]);

        cctx.with_stack(vec![0xF0u8, 0xFF]);
        assert_eq!(Machine::xor(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0x0F]);
    }

    #[test]
    fn not() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![0u8]);
        assert_eq!(Machine::not(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [U256::MAX]);

        cctx.with_stack(vec![U256::MAX]);
        assert_eq!(Machine::not(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![0xF0u8]);
        assert_eq!(Machine::not(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0F")]);
    }

    #[test]
    fn byte() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![uint!("16"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Machine::byte(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![uint!("31"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Machine::byte(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0xF0]);

        cctx.with_stack(vec![uint!("15"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Machine::byte(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("32"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Machine::byte(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("28"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Machine::byte(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0xCD]);

        cctx.with_stack(vec![uint!("19"), uint!("0x0112233445566778899AABBCCDDEEFF0")]);
        assert_eq!(Machine::byte(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0x34]);
    }

    #[test]
    fn shl() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![1u8, 1]);
        assert_eq!(Machine::shl(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2]);

        cctx.with_stack(vec![uint!("4"), uint!("0xFF00000000000000000000000000000000000000000000000000000000000000")]);
        assert_eq!(Machine::shl(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0xF000000000000000000000000000000000000000000000000000000000000000")]);
    }

    #[test]
    fn shr() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![1u8, 2]);
        assert_eq!(Machine::shr(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![4u8, 0xFFu8]);
        assert_eq!(Machine::shr(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0x0F]);
    }

    #[test]
    fn sar() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![1u8, 2]);
        assert_eq!(Machine::sar(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_stack(vec![uint!("4"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);
        assert_eq!(Machine::sar(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [U256::MAX]);

        cctx.with_stack(vec![uint!("600"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);
        assert_eq!(Machine::sar(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [U256::MAX]);

        cctx.with_stack(vec![U256::MAX, uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);
        assert_eq!(Machine::sar(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [U256::MAX]);

        cctx.with_stack(vec![U256::MAX, uint!("0x0FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);
        assert_eq!(Machine::sar(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("0"), uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);
        assert_eq!(Machine::sar(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF0")]);

        cctx.with_stack(vec![uint!("4"), uint!("0xEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFAB00")]);
        assert_eq!(Machine::sar(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0xFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFAB0")]);
    }

    #[test]
    fn keccak256() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![0u8, 4]);
        cctx.with_memory("FFFFFFFF00000000000000000000000000000000000000000000000000000000");
        assert_eq!(Machine::keccak256(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 36, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x29045A592007D0C246EF02C2223570DA9522D0CF0F73282C79A1BC8F0BB2C238")]);

        cctx.with_stack(vec![4u8, 40]);
        cctx.with_memory("FFFFFFFF00000000000000000000000000000000000000000000000000000000");
        assert_eq!(Machine::keccak256(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 45, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0xDAA77426C30C02A43D9FBA4E841A6556C524D47030762EB14DC4AF897E605D9B")]);
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
        assert_eq!(Machine::address(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")]);
    }

    #[test]
    fn balance() {
        let state = &mut WorldState::default();
        let cctx = &mut CallContext::default();

        state.with_accounts(&[(Address(uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")), Account { balance: uint!("125985"), code: vec![] })]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]);
        assert_eq!(Machine::balance(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2600, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [125985]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]);
        assert_eq!(Machine::balance(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [125985]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D9")]);
        assert_eq!(Machine::balance(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2600, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D9")]);
        assert_eq!(Machine::balance(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("0x109BBFED6889322E016E0A02EE459D306FC19545D9")]);
        assert_eq!(Machine::balance(state, &TransactionContext::default(), cctx), Err(Error::InvalidAddress));
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

        assert_eq!(Machine::origin(&mut WorldState::default(), &tctx, cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]);
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
        assert_eq!(Machine::caller(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")]);
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
        assert_eq!(Machine::callvalue(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [42]);
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
        assert_eq!(Machine::calldataload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [U256::MAX]);

        cctx.with_stack(vec![31u8]);
        assert_eq!(Machine::calldataload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0xFF00000000000000000000000000000000000000000000000000000000000000")]);
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

        assert_eq!(Machine::calldatasize(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [30]);
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
        assert_eq!(Machine::calldatacopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 9, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap());

        cctx.with_stack(vec![0u8, 31, 8]);
        assert_eq!(Machine::calldatacopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
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

        assert_eq!(Machine::codesize(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [30]);
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
        assert_eq!(Machine::codecopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 9, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap());

        cctx.with_stack(vec![0u8, 31, 8]);
        assert_eq!(Machine::codecopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
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

        assert_eq!(Machine::gasprice(&mut WorldState::default(), &tctx, cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [15]);
    }

    #[test]
    fn extcodesize() {
        let state = &mut WorldState::default();
        let cctx = &mut CallContext::default();

        state.with_accounts(&[(Address(uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")), Account { balance: U256::ZERO, code: hex::decode("FF0F4C").unwrap() })]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]);
        assert_eq!(Machine::extcodesize(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2600, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [3]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")]);
        assert_eq!(Machine::extcodesize(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [3]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D9")]);
        assert_eq!(Machine::extcodesize(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2600, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D9")]);
        assert_eq!(Machine::extcodesize(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_stack(vec![uint!("0x109BBFED6889322E016E0A02EE459D306FC19545D9")]);
        assert_eq!(Machine::extcodesize(state, &TransactionContext::default(), cctx), Err(Error::InvalidAddress));
    }

    #[test]
    fn extcodecopy() {
        let state = &mut WorldState::default();
        let cctx = &mut CallContext::default();

        state.with_accounts(&[(Address(uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8")), Account { balance: U256::ZERO, code: hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap() })]);

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8"), U256::ZERO, U256::ZERO, uint!("32")]);
        assert_eq!(Machine::extcodecopy(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2606, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap());

        cctx.with_stack(vec![uint!("0x9BBFED6889322E016E0A02EE459D306FC19545D8"), U256::ZERO, uint!("31"), uint!("8")]);
        assert_eq!(Machine::extcodecopy(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 103, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("FF00000000000000FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap());
    }

    #[test]
    fn returndatasize() {
        let cctx = &mut CallContext::default();

        cctx.with_returndata("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF");

        assert_eq!(Machine::returndatasize(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [32]);
    }

    #[test]
    fn returndatacopy() {
        let cctx = &mut CallContext::default();

        cctx.with_returndata("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF");

        cctx.with_stack(vec![0u8, 0, 32]);
        assert_eq!(Machine::returndatacopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 9, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap());

        cctx.with_stack(vec![32u8, 31, 1]);
        assert_eq!(Machine::returndatacopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 9, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000000000000000000000000000000000000000000000000000000000").unwrap());
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

        assert_eq!(Machine::coinbase(&mut WorldState::default(), tctx, cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")]);
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

        assert_eq!(Machine::timestamp(&mut WorldState::default(), tctx, cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1734036363]);
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

        assert_eq!(Machine::number(&mut WorldState::default(), tctx, cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [50]);
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

        assert_eq!(Machine::prevrandao(&mut WorldState::default(), tctx, cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [50]);
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

        assert_eq!(Machine::gaslimit(&mut WorldState::default(), tctx, cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [50]);
    }

    #[test]
    fn chainid() {
        let s = &mut WorldState::default();
        let cctx = &mut CallContext::default();

        s.with_chain_id(5u8);

        assert_eq!(Machine::chainid(s, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [5]);
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

        assert_eq!(Machine::selfbalance(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [125985]);

        assert_eq!(Machine::selfbalance(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [125985]);
    }

    #[test]
    fn pop() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![30u8, 42]);
        assert_eq!(Machine::pop(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [42]);
    }

    #[test]
    fn mload() {
        let cctx = &mut CallContext::default();

        cctx.with_memory("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369");
        cctx.with_stack(vec![0u8]);
        assert_eq!(Machine::mload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369")]);
        assert_eq!(cctx.memory.0, hex::decode("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369").unwrap());

        cctx.with_memory("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369");
        cctx.with_stack(vec![2u8]);
        assert_eq!(Machine::mload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0xB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A023690000")]);
        assert_eq!(cctx.memory.0, hex::decode("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A023690000000000000000000000000000000000000000000000000000000000000000").unwrap());

        cctx.with_memory("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369");
        cctx.with_stack(vec![30u8]);
        assert_eq!(Machine::mload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x2369000000000000000000000000000000000000000000000000000000000000")]);
        assert_eq!(cctx.memory.0, hex::decode("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A023690000000000000000000000000000000000000000000000000000000000000000").unwrap());

        cctx.with_memory("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369");
        cctx.with_stack(vec![500u16]);
        assert_eq!(Machine::mload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 51, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);
        assert_eq!(cctx.memory.0, hex::decode("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A023690000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap());
    }

    #[test]
    fn mstore() {
        let cctx = &mut CallContext::default();

        cctx.with_memory("");
        cctx.with_stack(vec![0u8, 0xFF]);
        assert_eq!(Machine::mstore(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("00000000000000000000000000000000000000000000000000000000000000FF").unwrap());
        cctx.with_stack(vec![1u8, 0xFF]);
        assert_eq!(Machine::mstore(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("0000000000000000000000000000000000000000000000000000000000000000FF00000000000000000000000000000000000000000000000000000000000000").unwrap());

        cctx.with_memory("");
        cctx.with_stack(vec![3u8, 0xFF]);
        assert_eq!(Machine::mstore(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 9, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("00000000000000000000000000000000000000000000000000000000000000000000FF0000000000000000000000000000000000000000000000000000000000").unwrap());

        cctx.with_memory("");
        cctx.with_stack(vec![500u16, 0xABFF]);
        assert_eq!(Machine::mstore(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 54, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ABFF000000000000000000000000").unwrap());
    }

    #[test]
    fn mstore8() {
        let cctx = &mut CallContext::default();

        cctx.with_memory("");
        cctx.with_stack(vec![0u16, 0xFFAB]);
        assert_eq!(Machine::mstore8(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("AB00000000000000000000000000000000000000000000000000000000000000").unwrap());
        cctx.with_stack(vec![31u16, 0xFFAB]);
        assert_eq!(Machine::mstore8(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
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
        assert_eq!(Machine::sload(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2100, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0xAB]);
        cctx.with_stack(vec![42u8]);
        assert_eq!(Machine::sload(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0xAB]);

        cctx.with_stack(vec![40u8]);
        assert_eq!(Machine::sload(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2100, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);
        cctx.with_stack(vec![40u8]);
        assert_eq!(Machine::sload(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);
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
        assert_eq!(Machine::sstore(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 22100, jump: 1 })); // clean storage - no previous value - cold slot
        assert_eq!(state.storage.get(&Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91"))).unwrap().0.get(&uint!("0")), Some(&StorageValue {
            original_value: uint!("0"),
            value: uint!("0xFFFF"),
            warm: true,
        }));
        cctx.with_stack(vec![0u16, 0xFFFF]);
        assert_eq!(Machine::sstore(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 })); // dirty storage - same value - warn slot
        assert_eq!(state.storage.get(&Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91"))).unwrap().0.get(&uint!("0")), Some(&StorageValue {
            original_value: uint!("0"),
            value: uint!("0xFFFF"),
            warm: true,
        }));
        cctx.with_stack(vec![0u16, 0xFFF0]);
        assert_eq!(Machine::sstore(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 })); // dirty storage - different value - warn slot
        assert_eq!(state.storage.get(&Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91"))).unwrap().0.get(&uint!("0")), Some(&StorageValue {
            original_value: uint!("0"),
            value: uint!("0xFFF0"),
            warm: true,
        }));

        state.with_storage(&[(Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")), &[(1u8, 55)])]);

        cctx.with_stack(vec![1u16, 10]);
        assert_eq!(Machine::sstore(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 5000, jump: 1 })); // clean storage - different value - cold slot
        assert_eq!(state.storage.get(&Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91"))).unwrap().0.get(&uint!("1")), Some(&StorageValue {
            original_value: uint!("55"),
            value: uint!("10"),
            warm: true,
        }));

        state.with_storage(&[(Address(uint!("0xF778B86FA74E846C4F0A1FBD1335FE81C00A0C91")), &[(1u8, 55)])]);

        cctx.with_stack(vec![1u16, 55]);
        assert_eq!(Machine::sstore(state, &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2200, jump: 1 })); // clean storage - same value - cold slot
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
        assert_eq!(Machine::jump(&mut WorldState::default(), &TransactionContext::default(), cctx), Err(Error::InvalidJumpDest)); // not a usize
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![0xFFFFu16]);
        assert_eq!(Machine::jump(&mut WorldState::default(), &TransactionContext::default(), cctx), Err(Error::InvalidJumpDest)); // not in range
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![1u8]);
        assert_eq!(Machine::jump(&mut WorldState::default(), &TransactionContext::default(), cctx), Err(Error::InvalidJumpDest)); // not a valid destination
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![2u8]);
        assert_eq!(Machine::jump(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 8, jump: 0 }));
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
        assert_eq!(Machine::jumpi(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 10, jump: 1 }));
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![uint!("0xFFFFFFFFFFFFFFFFFFFF"), uint!("1")]);
        assert_eq!(Machine::jumpi(&mut WorldState::default(), &TransactionContext::default(), cctx), Err(Error::InvalidJumpDest)); // not a usize
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![0xFFFFu16, 1]);
        assert_eq!(Machine::jumpi(&mut WorldState::default(), &TransactionContext::default(), cctx), Err(Error::InvalidJumpDest)); // not in range
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![1u8, 1]);
        assert_eq!(Machine::jumpi(&mut WorldState::default(), &TransactionContext::default(), cctx), Err(Error::InvalidJumpDest)); // not a valid destination
        assert_eq!(cctx.pc, 0);

        cctx.with_stack(vec![2u8, 1]);
        assert_eq!(Machine::jumpi(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 10, jump: 0 }));
        assert_eq!(cctx.pc, 2);
    }

    #[test]
    fn pc() {
        let cctx = &mut CallContext::default();

        cctx.with_pc(30);

        assert_eq!(Machine::pc(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [30]);
    }

    #[test]
    fn msize() {
        let cctx = &mut CallContext::default();

        cctx.with_memory("4DBDB8BE3125A5DE53A0236934525103F67CF6E94DBDB8BE3125A5DE53A02369");

        assert_eq!(Machine::msize(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [32]);
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

        assert_eq!(Machine::gas(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [3]);


        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: vec![],
            gas: 3,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });

        assert_eq!(Machine::gas(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1]);

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: vec![],
            gas: 1,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });

        assert_eq!(Machine::gas(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);

        cctx.with_contract(CallContextContract {
            address: Address(U256::ZERO),
            caller: Address(U256::ZERO),
            code: vec![],
            gas: 0,
            input: vec![],
            logs: vec![],
            value: U256::ZERO,
        });

        assert_eq!(Machine::gas(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn jumpdest() {
        assert_eq!(Machine::jumpdest(&mut WorldState::default(), &TransactionContext::default(), &mut CallContext::default()), Ok(InstructionOutput { cost: 1, jump: 1 }));
    }

    #[test]
    fn tload() {
        let cctx = &mut CallContext::default();

        cctx.with_transient(&[(42u8, 0xAB)]);

        cctx.with_stack(vec![42u8]);
        assert_eq!(Machine::tload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0xAB]);

        cctx.with_stack(vec![45u8]);
        assert_eq!(Machine::tload(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [0]);
    }

    #[test]
    fn tstore() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![1u8, 55]);
        assert_eq!(Machine::tstore(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 100, jump: 1 }));
        assert_eq!(cctx.transient.0.get(&uint!("1")), Some(&uint!("55")));
    }

    #[test]
    fn mcopy() {
        let cctx = &mut CallContext::default();

        cctx.with_memory("00000000000000000000000000000000000000000000000000000000000000000001020304050607080910111213141516171819202122232425262728293031");

        cctx.with_stack(vec![0u8, 32, 32]);
        assert_eq!(Machine::mcopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("00010203040506070809101112131415161718192021222324252627282930310001020304050607080910111213141516171819202122232425262728293031").unwrap());

        cctx.with_stack(vec![4u8, 8, 16]);
        assert_eq!(Machine::mcopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 6, jump: 1 }));
        assert_eq!(cctx.memory.0, hex::decode("00010203080910111213141516171819202122232021222324252627282930310001020304050607080910111213141516171819202122232425262728293031").unwrap());

        cctx.with_memory("000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F");

        cctx.with_stack(vec![100u8, 4, 40]);
        assert_eq!(Machine::mcopy(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 21, jump: 1 }));
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

        assert_eq!(Machine::push::<0>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 2, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0")]);

        assert_eq!(Machine::push::<1>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 2 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x59")]);

        assert_eq!(Machine::push::<2>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 3 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936")]);

        assert_eq!(Machine::push::<3>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 4 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2")]);

        assert_eq!(Machine::push::<4>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 5 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1")]);

        assert_eq!(Machine::push::<5>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 6 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5")]);

        assert_eq!(Machine::push::<6>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 7 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3")]);

        assert_eq!(Machine::push::<7>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 8 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF")]);

        assert_eq!(Machine::push::<8>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 9 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2E")]);

        assert_eq!(Machine::push::<9>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 10 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB")]);

        assert_eq!(Machine::push::<10>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 11 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB31")]);

        assert_eq!(Machine::push::<11>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 12 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155")]);

        assert_eq!(Machine::push::<12>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 13 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B9")]);

        assert_eq!(Machine::push::<13>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 14 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B")]);

        assert_eq!(Machine::push::<14>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 15 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B30")]);

        assert_eq!(Machine::push::<15>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 16 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001")]);

        assert_eq!(Machine::push::<16>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 17 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A3")]);

        assert_eq!(Machine::push::<17>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 18 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347")]);

        assert_eq!(Machine::push::<18>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 19 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6")]);

        assert_eq!(Machine::push::<19>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 20 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE")]);

        assert_eq!(Machine::push::<20>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 21 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75")]);

        assert_eq!(Machine::push::<21>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 22 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E5")]);

        assert_eq!(Machine::push::<22>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 23 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E518")]);

        assert_eq!(Machine::push::<23>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 24 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859")]);

        assert_eq!(Machine::push::<24>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 25 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EB")]);

        assert_eq!(Machine::push::<25>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 26 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA")]);

        assert_eq!(Machine::push::<26>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 27 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA81")]);

        assert_eq!(Machine::push::<27>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 28 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155")]);

        assert_eq!(Machine::push::<28>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 29 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA815513")]);

        assert_eq!(Machine::push::<29>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 30 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155131A")]);

        assert_eq!(Machine::push::<30>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 31 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155131A8E")]);

        assert_eq!(Machine::push::<31>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 32 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155131A8E05")]);

        assert_eq!(Machine::push::<32>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 33 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [uint!("0x5936D2A1C5C3AF2EEB3155B96B3001A347D6FE75E51859EBBA8155131A8E0556")]);
    }

    #[test]
    fn dup() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![1u8]);
        assert_eq!(Machine::dup::<1>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 1]);
    
        cctx.with_stack(vec![0u8, 1]);
        assert_eq!(Machine::dup::<2>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 1]);
        assert_eq!(Machine::dup::<3>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 1]);
        assert_eq!(Machine::dup::<4>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 1]);
        assert_eq!(Machine::dup::<5>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 1]);
        assert_eq!(Machine::dup::<6>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Machine::dup::<7>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Machine::dup::<8>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Machine::dup::<9>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Machine::dup::<10>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Machine::dup::<11>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Machine::dup::<12>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Machine::dup::<13>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Machine::dup::<14>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Machine::dup::<15>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(Machine::dup::<16>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn swap() {
        let cctx = &mut CallContext::default();

        cctx.with_stack(vec![1u8, 2]);
        assert_eq!(Machine::swap::<2>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 1]);

        cctx.with_stack(vec![1u8, 0, 2]);
        assert_eq!(Machine::swap::<3>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 2]);
        assert_eq!(Machine::swap::<4>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 2]);
        assert_eq!(Machine::swap::<5>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 2]);
        assert_eq!(Machine::swap::<6>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Machine::swap::<7>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Machine::swap::<8>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Machine::swap::<9>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Machine::swap::<10>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Machine::swap::<11>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Machine::swap::<12>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Machine::swap::<13>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Machine::swap::<14>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Machine::swap::<15>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Machine::swap::<16>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

        cctx.with_stack(vec![1u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        assert_eq!(Machine::swap::<17>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 3, jump: 1 }));
        assert_eq!(Machine::pop_or_fail(cctx).unwrap(), [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    }

        #[test]
    fn log() {
        let cctx = &mut CallContext::default();

        cctx.with_memory("000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F");

        cctx.with_stack(vec![4u8, 16]);
        assert_eq!(Machine::log::<0>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 503, jump: 1 }));
        assert_eq!(cctx.contract.logs, vec![Log {
            data: hex::decode("0405060708090A0B0C0D0E0F10111213").unwrap(),
            topics: [None, None, None, None],
        }]);

        cctx.with_stack(vec![14u8, 8, 42]);
        assert_eq!(Machine::log::<1>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 814, jump: 1 }));
        assert_eq!(cctx.contract.logs, vec![Log {
            data: hex::decode("0405060708090A0B0C0D0E0F10111213").unwrap(),
            topics: [None, None, None, None],
        }, Log {
            data: hex::decode("0E0F101112131415").unwrap(),
            topics: [Some(uint!("42")), None, None, None],
        }]);

        cctx.with_stack(vec![18u8, 20, 50, 52]);
        assert_eq!(Machine::log::<2>(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 1288, jump: 1 }));
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
        assert_eq!(Machine::r#return(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 0, jump: 0 }));
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
        assert_eq!(Machine::revert(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 0, jump: 0 }));
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
        assert_eq!(Machine::invalid(&mut WorldState::default(), &TransactionContext::default(), cctx), Ok(InstructionOutput { cost: 25, jump: 0 }));
        assert!(cctx.stop);
        assert!(cctx.revert);
    }
}
