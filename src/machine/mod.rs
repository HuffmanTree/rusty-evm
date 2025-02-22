pub mod context;
pub mod instructions;
pub mod memory;
pub mod opcode;
pub mod stack;
pub mod transient;

use ethnum::AsU256;

use crate::blockchain::primitives::Account;
use crate::blockchain::WorldState;
use crate::blockchain::errors::Error;
use crate::machine::context::{CallContext, TransactionContext};
use crate::machine::opcode::OpCode;

#[derive(Default, Debug, Eq, PartialEq)]
pub struct ExecutionOutput {
    pub data: Vec<u8>,
    pub remaining_gas: usize,
    pub revert: bool,
}
pub type ExecutionResult = Result<ExecutionOutput, Error>;

pub struct Machine {}

impl Machine {
    fn pay_gas_cost(s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext, gas_cost: usize) -> Result<(), Error> {
        if cctx.contract.gas < gas_cost { cctx.contract.gas = 0; return Err(Error::OutOfGas); }

        cctx.contract.gas -= gas_cost;
        s.decrease_balance(tctx.tx.from, (gas_cost * tctx.tx.gas_price).as_u256())?;

        Ok(())
    }

    fn execute_next_opcode(s: &mut WorldState, tctx: &TransactionContext, cctx: &mut CallContext) -> Result<(), Error> {
        let opcode = OpCode(*cctx.contract.code.get(cctx.pc).unwrap_or(&0));
        let output = opcode.execute(s, tctx, cctx)?;

        Machine::pay_gas_cost(s, tctx, cctx, output.cost)?;
        cctx.pc += output.jump;

        Ok(())
    }

    pub fn execute_transaction(s: &mut WorldState, tctx: &TransactionContext) -> ExecutionResult {
        let cctx = &mut CallContext::from_transaction(s, &tctx.tx);

        let sender = s.accounts.load(tctx.tx.from).value;
        let max_cost = (tctx.tx.gas * tctx.tx.gas_price).as_u256() + tctx.tx.value;

        sender.check_enough_funds(max_cost)?;

        let intrisic_gas_cost = tctx.tx.intrinsic_gas_cost();
        Machine::pay_gas_cost(s, tctx, cctx, intrisic_gas_cost).map_err(|e| match e {
            Error::OutOfGas => Error::IntrisicGasTooLow(intrisic_gas_cost),
            _ => e,
        })?;

        // TODO (fguerin - 22/12/2024) Handle sub-context creations
        while !cctx.stop {
            Machine::execute_next_opcode(s, tctx, cctx)?;
        }

        if tctx.tx.is_contract_creation() {
            Machine::pay_gas_cost(s, tctx, cctx, 200 * cctx.r#return.clone().len())?; // code deposit cost
            s.accounts.store(cctx.contract.address, Account {
                balance: tctx.tx.value,
                code: cctx.r#return.clone(),
            });
            s.decrease_balance(tctx.tx.from, tctx.tx.value)?;
        }

        Ok(ExecutionOutput {
            data: cctx.r#return.clone(),
            remaining_gas: cctx.contract.gas,
            revert: cctx.revert,
        })
    }
}
