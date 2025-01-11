pub mod context;
pub mod instructions;
pub mod memory;
pub mod stack;
pub mod transient;

use ethnum::AsU256;

use crate::blockchain::WorldState;
use crate::blockchain::errors::Error;
use crate::machine::context::{CallContext, TransactionContext};
use crate::machine::instructions::{InstructionFn, Instructions};

#[derive(Default, Debug, Eq, PartialEq)]
pub struct ExecutionOutput {
    pub data: Vec<u8>,
    pub remaining_gas: usize,
    pub revert: bool,
}
pub type ExecutionResult = Result<ExecutionOutput, Error>;

pub struct Machine {}

impl Machine {
    fn next_instruction(cctx: &CallContext) -> InstructionFn {
        match *cctx.contract.code.get(cctx.pc).unwrap_or(&0) {
            0x00 => Instructions::stop,
            0x01 => Instructions::add,
            0x02 => Instructions::mul,
            0x03 => Instructions::sub,
            0x04 => Instructions::div,
            0x05 => Instructions::sdiv,
            0x06 => Instructions::r#mod,
            0x07 => Instructions::smod,
            0x08 => Instructions::addmod,
            0x09 => Instructions::mulmod,
            0x0A => Instructions::exp,
            0x0B => Instructions::signextend,
            0x10 => Instructions::lt,
            0x11 => Instructions::gt,
            0x12 => Instructions::slt,
            0x13 => Instructions::sgt,
            0x14 => Instructions::eq,
            0x15 => Instructions::iszero,
            0x16 => Instructions::and,
            0x17 => Instructions::or,
            0x18 => Instructions::xor,
            0x19 => Instructions::not,
            0x1A => Instructions::byte,
            0x1B => Instructions::shl,
            0x1C => Instructions::shr,
            0x1D => Instructions::sar,
            0x20 => Instructions::keccak256,
            0x30 => Instructions::address,
            0x31 => Instructions::balance,
            0x32 => Instructions::origin,
            0x33 => Instructions::caller,
            0x34 => Instructions::callvalue,
            0x35 => Instructions::calldataload,
            0x36 => Instructions::calldatasize,
            0x37 => Instructions::calldatacopy,
            0x38 => Instructions::codesize,
            0x39 => Instructions::codecopy,
            0x3A => Instructions::gasprice,
            0x3B => Instructions::extcodesize,
            0x3C => Instructions::extcodecopy,
            0x3D => Instructions::returndatasize,
            0x3E => Instructions::returndatacopy,
            0x3F => Instructions::extcodehash,
            0x40 => Instructions::blockhash,
            0x41 => Instructions::coinbase,
            0x42 => Instructions::timestamp,
            0x43 => Instructions::number,
            0x44 => Instructions::prevrandao,
            0x45 => Instructions::gaslimit,
            0x46 => Instructions::chainid,
            0x47 => Instructions::selfbalance,
            0x48 => Instructions::basefee,
            0x49 => Instructions::blobhash,
            0x4A => Instructions::blobbasefee,
            0x50 => Instructions::pop,
            0x51 => Instructions::mload,
            0x52 => Instructions::mstore,
            0x53 => Instructions::mstore8,
            0x54 => Instructions::sload,
            0x55 => Instructions::sstore,
            0x56 => Instructions::jump,
            0x57 => Instructions::jumpi,
            0x58 => Instructions::pc,
            0x59 => Instructions::msize,
            0x5A => Instructions::gas,
            0x5B => Instructions::jumpdest,
            0x5C => Instructions::tload,
            0x5D => Instructions::tstore,
            0x5E => Instructions::mcopy,
            0x5F => Instructions::push::<0>,
            0x60 => Instructions::push::<1>,
            0x61 => Instructions::push::<2>,
            0x62 => Instructions::push::<3>,
            0x63 => Instructions::push::<4>,
            0x64 => Instructions::push::<5>,
            0x65 => Instructions::push::<6>,
            0x66 => Instructions::push::<7>,
            0x67 => Instructions::push::<8>,
            0x68 => Instructions::push::<9>,
            0x69 => Instructions::push::<10>,
            0x6A => Instructions::push::<11>,
            0x6B => Instructions::push::<12>,
            0x6C => Instructions::push::<13>,
            0x6D => Instructions::push::<14>,
            0x6E => Instructions::push::<15>,
            0x6F => Instructions::push::<16>,
            0x70 => Instructions::push::<17>,
            0x71 => Instructions::push::<18>,
            0x72 => Instructions::push::<19>,
            0x73 => Instructions::push::<20>,
            0x74 => Instructions::push::<21>,
            0x75 => Instructions::push::<22>,
            0x76 => Instructions::push::<23>,
            0x77 => Instructions::push::<24>,
            0x78 => Instructions::push::<25>,
            0x79 => Instructions::push::<26>,
            0x7A => Instructions::push::<27>,
            0x7B => Instructions::push::<28>,
            0x7C => Instructions::push::<29>,
            0x7D => Instructions::push::<30>,
            0x7E => Instructions::push::<31>,
            0x7F => Instructions::push::<32>,
            0x80 => Instructions::dup::<1>,
            0x81 => Instructions::dup::<2>,
            0x82 => Instructions::dup::<3>,
            0x83 => Instructions::dup::<4>,
            0x84 => Instructions::dup::<5>,
            0x85 => Instructions::dup::<6>,
            0x86 => Instructions::dup::<7>,
            0x87 => Instructions::dup::<8>,
            0x88 => Instructions::dup::<9>,
            0x89 => Instructions::dup::<10>,
            0x8A => Instructions::dup::<11>,
            0x8B => Instructions::dup::<12>,
            0x8C => Instructions::dup::<13>,
            0x8D => Instructions::dup::<14>,
            0x8E => Instructions::dup::<15>,
            0x8F => Instructions::dup::<16>,
            0x90 => Instructions::swap::<2>,
            0x91 => Instructions::swap::<3>,
            0x92 => Instructions::swap::<4>,
            0x93 => Instructions::swap::<5>,
            0x94 => Instructions::swap::<6>,
            0x95 => Instructions::swap::<7>,
            0x96 => Instructions::swap::<8>,
            0x97 => Instructions::swap::<9>,
            0x98 => Instructions::swap::<10>,
            0x99 => Instructions::swap::<11>,
            0x9A => Instructions::swap::<12>,
            0x9B => Instructions::swap::<13>,
            0x9C => Instructions::swap::<14>,
            0x9D => Instructions::swap::<15>,
            0x9E => Instructions::swap::<16>,
            0x9F => Instructions::swap::<17>,
            0xA0 => Instructions::log::<0>,
            0xA1 => Instructions::log::<1>,
            0xA2 => Instructions::log::<2>,
            0xA3 => Instructions::log::<3>,
            0xA4 => Instructions::log::<4>,
            0xF0 => Instructions::create,
            0xF1 => Instructions::call,
            0xF2 => Instructions::callcode,
            0xF3 => Instructions::r#return,
            0xF4 => Instructions::delegatecall,
            0xF5 => Instructions::create2,
            0xFA => Instructions::staticcall,
            0xFD => Instructions::revert,
            0xFE => Instructions::invalid,
            0xFF => Instructions::selfdestruct,
            _ => Instructions::invalid,
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
