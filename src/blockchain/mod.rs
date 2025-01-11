pub mod errors;
pub mod primitives;
pub mod storage;

use ethnum::u256;
use crate::blockchain::primitives::{Account, Address};
use crate::blockchain::storage::Storage;
use std::collections::HashMap;

#[derive(Default)]
pub struct WorldState {
    pub accounts: Storage<Address, Account>,
    pub chain_id: u256,
    pub storage: HashMap<Address, Storage<u256, u256>>,
}
